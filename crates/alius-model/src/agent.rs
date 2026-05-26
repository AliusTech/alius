//! Agent loop implementation

use std::sync::Arc;
use futures::StreamExt;

use crate::{LlmClient, Conversation, AgentEvent, ChatEvent, ToolCall};
use alius_config::Settings;
use alius_tools::{ToolRegistry, ToolContext, ToolResult, ConfirmationRequest};

/// Confirmation callback type
pub type ConfirmationCallback = Box<dyn Fn(ConfirmationRequest) -> bool + Send + Sync>;

/// Model response with optional tool calls
pub struct ModelResponse {
    pub text: String,
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Agent for handling user messages with tool calling
pub struct AliusAgent {
    client: Arc<LlmClient>,
    registry: Arc<ToolRegistry>,
    #[allow(dead_code)]
    settings: Settings,
    max_tool_calls: usize,
    auto_confirm: bool,
}

impl AliusAgent {
    /// Create a new agent
    pub fn new(client: Arc<LlmClient>, registry: Arc<ToolRegistry>, settings: Settings) -> Self {
        Self {
            client,
            registry,
            settings,
            max_tool_calls: 10, // Limit tool calls per turn
            auto_confirm: false,
        }
    }

    /// Enable auto-confirm for all tool operations
    pub fn with_auto_confirm(mut self, value: bool) -> Self {
        self.auto_confirm = value;
        self
    }

    /// Handle a user message with tool calling support
    pub async fn handle_message(
        &self,
        conversation: &mut Conversation,
        user_input: String,
        workspace: std::path::PathBuf,
        session_id: String,
    ) -> Vec<AgentEvent> {
        let mut events = Vec::new();
        let mut tool_call_count = 0;
        let mut pending_tool_results: Vec<(String, String, String)> = Vec::new();

        events.push(AgentEvent::TurnStarted);

        // Add user message to conversation
        conversation.add_user_message(user_input);

        // Get tools from registry
        let tools = self.registry.to_openai_tools();

        // Loop: call model, execute tools if needed, repeat
        loop {
            if tool_call_count >= self.max_tool_calls {
                events.push(AgentEvent::Error {
                    message: "Maximum tool calls exceeded".to_string()
                });
                break;
            }

            // Get model response with tools
            events.push(AgentEvent::ModelStarted);

            let result = if pending_tool_results.is_empty() {
                self.get_model_response(conversation, tools.clone()).await
            } else {
                self.get_model_response_with_results(
                    conversation,
                    pending_tool_results.clone(),
                    tools.clone()
                ).await
            };

            match result {
                Ok(response) => {
                    events.push(AgentEvent::ModelFinished {
                        full_response: response.text.clone()
                    });

                    // Add assistant message to conversation
                    if !response.text.is_empty() {
                        conversation.add_assistant_message(response.text.clone());
                    }

                    // Clear pending results after processing
                    pending_tool_results.clear();

                    // Check for tool calls
                    if let Some(calls) = response.tool_calls {
                        for call in calls {
                            events.push(AgentEvent::ToolCallStarted {
                                id: call.id.clone(),
                                name: call.name.clone(),
                                args: call.args.clone(),
                            });

                            // Check if tool requires confirmation
                            if let Some(tool) = self.registry.get(&call.name) {
                                if tool.requires_confirmation(&call.args) {
                                    if let Some(req) = tool.confirmation_request(&call.args) {
                                        events.push(AgentEvent::ToolConfirmationRequested {
                                            id: call.id.clone(),
                                            name: call.name.clone(),
                                            operation: req.operation,
                                            details: req.details,
                                        });

                                        // Auto-confirm if enabled
                                        if self.auto_confirm {
                                            events.push(AgentEvent::ToolConfirmed { id: call.id.clone() });
                                        } else {
                                            // In auto mode, we deny - REPL should handle this
                                            events.push(AgentEvent::ToolDenied {
                                                id: call.id.clone(),
                                                reason: "User confirmation required".to_string(),
                                            });
                                            pending_tool_results.push((
                                                call.id.clone(),
                                                call.name.clone(),
                                                "Tool execution denied - user confirmation required".to_string()
                                            ));
                                            tool_call_count += 1;
                                            continue;
                                        }
                                    }
                                }
                            }

                            // Execute tool
                            let result = self.execute_tool(&call, workspace.clone(), session_id.clone());
                            let tool_result = result.await;

                            events.push(AgentEvent::ToolCallFinished {
                                id: call.id.clone(),
                                name: call.name.clone(),
                                result: tool_result.output.clone(),
                                success: tool_result.success,
                            });

                            pending_tool_results.push((call.id, call.name, tool_result.output));
                            tool_call_count += 1;
                        }

                        // Continue loop - model will process tool results
                        continue;
                    }

                    // No tool calls, turn is finished
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

    /// Get model response (initial call)
    async fn get_model_response(
        &self,
        conversation: &Conversation,
        tools: Vec<serde_json::Value>,
    ) -> anyhow::Result<ModelResponse> {
        let (stream, tool_calls) = self.client.chat_stream_with_tools(conversation, tools).await?;

        let mut text = String::new();
        let mut stream = Box::pin(stream);

        while let Some(event) = stream.next().await {
            match event {
                Ok(ChatEvent::Delta { text: t }) => text.push_str(&t),
                Ok(ChatEvent::Done { full_response }) => {
                    if full_response.len() > text.len() {
                        text = full_response;
                    }
                    break;
                }
                Ok(ChatEvent::Error { message }) => {
                    return Err(anyhow::anyhow!("{}", message));
                }
                Err(e) => return Err(e),
            }
        }

        Ok(ModelResponse { text, tool_calls })
    }

    /// Get model response with tool results
    async fn get_model_response_with_results(
        &self,
        conversation: &Conversation,
        tool_results: Vec<(String, String, String)>,
        tools: Vec<serde_json::Value>,
    ) -> anyhow::Result<ModelResponse> {
        let (stream, tool_calls) = self.client
            .continue_with_tool_results(conversation, tool_results, tools)
            .await?;

        let mut text = String::new();
        let mut stream = Box::pin(stream);

        while let Some(event) = stream.next().await {
            match event {
                Ok(ChatEvent::Delta { text: t }) => text.push_str(&t),
                Ok(ChatEvent::Done { full_response }) => {
                    if full_response.len() > text.len() {
                        text = full_response;
                    }
                    break;
                }
                Ok(ChatEvent::Error { message }) => {
                    return Err(anyhow::anyhow!("{}", message));
                }
                Err(e) => return Err(e),
            }
        }

        Ok(ModelResponse { text, tool_calls })
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