//! Agent loop implementation

use std::sync::Arc;
use futures::StreamExt;

use crate::{LlmClient, Conversation, AgentEvent, ChatEvent, ToolCall};
use alius_config::Settings;
use alius_tools::{ToolRegistry, ToolContext, ToolResult};

/// Agent for handling user messages with tool calling
pub struct AliusAgent {
    client: LlmClient,
    registry: Arc<ToolRegistry>,
    settings: Settings,
    max_tool_calls: usize,
}

impl AliusAgent {
    /// Create a new agent
    pub fn new(client: LlmClient, registry: Arc<ToolRegistry>, settings: Settings) -> Self {
        Self {
            client,
            registry,
            settings,
            max_tool_calls: 10, // Limit tool calls per turn
        }
    }

    /// Handle a user message with tool calling support
    pub async fn handle_message(
        &self,
        conversation: &mut Conversation,
        user_input: String,
        _workspace: std::path::PathBuf,
        _session_id: String,
    ) -> Vec<AgentEvent> {
        let mut events = Vec::new();

        events.push(AgentEvent::TurnStarted);

        // Add user message to conversation
        conversation.add_user_message(user_input);

        // Loop: call model, execute tools if needed, repeat
        loop {
            // Get model response
            events.push(AgentEvent::ModelStarted);

            let stream_result = self.client.chat_stream(conversation).await;

            match stream_result {
                Ok(stream) => {
                    let mut stream = Box::pin(stream);
                    let mut full_response = String::new();

                    while let Some(event) = stream.next().await {
                        match event {
                            Ok(ChatEvent::Delta { text }) => {
                                events.push(AgentEvent::ModelDelta { text: text.clone() });
                                full_response.push_str(&text);
                            }
                            Ok(ChatEvent::Done { .. }) => break,
                            Ok(ChatEvent::Error { message }) => {
                                events.push(AgentEvent::Error { message });
                                return events;
                            }
                            Err(e) => {
                                events.push(AgentEvent::Error { message: e.to_string() });
                                return events;
                            }
                        }
                    }

                    events.push(AgentEvent::ModelFinished { full_response: full_response.clone() });

                    // Add assistant message to conversation
                    conversation.add_assistant_message(full_response);

                    // If no tool calls, turn is finished
                    events.push(AgentEvent::TurnFinished);
                    break;
                }
                Err(e) => {
                    events.push(AgentEvent::Error { message: e.to_string() });
                    break;
                }
            }
        }

        events
    }

    /// Execute a tool call
    pub async fn execute_tool(
        &self,
        tool_call: &ToolCall,
        workspace: std::path::PathBuf,
        session_id: String,
    ) -> ToolResult {
        let ctx = ToolContext::new(workspace, session_id);

        if let Some(tool) = self.registry.get(&tool_call.name) {
            tool.execute(tool_call.args.clone(), ctx)
                .await
                .unwrap_or_else(|e| ToolResult::error(e.to_string()))
        } else {
            ToolResult::error(format!("Unknown tool: {}", tool_call.name))
        }
    }
}