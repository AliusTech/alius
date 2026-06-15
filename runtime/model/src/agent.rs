//! Agent loop implementation with tool calling support.
//!
//! The agent handles user messages by:
//! 1. Sending the message to the LLM
//! 2. If the model requests tool calls, executing them
//! 3. Sending tool results back to the model
//! 4. Repeating until the model produces a final text response
//!
//! This implements the "agent loop" pattern common in LLM applications.

use futures::StreamExt;
use std::sync::Arc;

use crate::{AgentEvent, ChatEvent, Conversation, LlmClient, ToolCall};
use runtime_config::Settings;
use runtime_tools::{ConfirmationRequest, ToolContext, ToolRegistry, ToolResult};

/// Callback type for tool execution confirmation.
///
/// Called when a tool requires user confirmation before execution.
/// Returns `true` to confirm, `false` to deny.
pub type ConfirmationCallback = Box<dyn Fn(ConfirmationRequest) -> bool + Send + Sync>;

/// Model response containing optional text and tool calls.
///
/// When the model decides to use tools, it returns tool calls instead of
/// (or in addition to) text. The agent loop processes these tool calls
/// and continues the conversation.
pub struct ModelResponse {
    /// The text response from the model (may be empty if only tool calls).
    pub text: String,
    /// Optional tool calls requested by the model.
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Agent for handling user messages with tool calling support.
///
/// The agent manages the conversation loop between the user, the LLM,
/// and the tool system. It handles:
/// - Sending messages to the LLM
/// - Processing tool call requests
/// - Executing tools and sending results back
/// - Confirmation workflows for dangerous operations
pub struct AliusAgent {
    /// The LLM client for API communication.
    client: Arc<LlmClient>,
    /// The tool registry containing available tools.
    registry: Arc<ToolRegistry>,
    /// Application settings.
    #[allow(dead_code)]
    settings: Settings,
    /// Maximum number of tool calls per turn to prevent infinite loops.
    max_tool_calls: usize,
    /// Whether to auto-confirm all tool operations (for testing/automation).
    auto_confirm: bool,
}

impl AliusAgent {
    /// Create a new agent with the given LLM client, tool registry, and settings.
    ///
    /// Defaults to a maximum of 10 tool calls per turn and auto-confirm disabled.
    pub fn new(client: Arc<LlmClient>, registry: Arc<ToolRegistry>, settings: Settings) -> Self {
        Self {
            client,
            registry,
            settings,
            max_tool_calls: 10, // Limit tool calls per turn to prevent infinite loops
            auto_confirm: false,
        }
    }

    /// Enable or disable auto-confirm for all tool operations.
    ///
    /// When enabled, tools that require confirmation will be executed
    /// without prompting the user. Useful for testing and automation.
    pub fn with_auto_confirm(mut self, value: bool) -> Self {
        self.auto_confirm = value;
        self
    }

    /// Handle a user message with tool calling support.
    ///
    /// This is the main entry point for the agent loop. It:
    /// 1. Adds the user message to the conversation
    /// 2. Calls the LLM with tool definitions
    /// 3. If the model requests tools, executes them (with confirmation if needed)
    /// 4. Sends tool results back to the model
    /// 5. Repeats until the model produces a final text response
    ///
    /// # Arguments
    /// * `conversation` - The conversation history to append to
    /// * `user_input` - The user's message text
    /// * `workspace` - The working directory for file operations
    /// * `session_id` - The current session identifier
    ///
    /// # Returns
    /// A list of agent events describing what happened during the turn.
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

        // Get tool definitions from registry
        let tools = self.registry.to_tool_defs();

        // Agent loop: call model, execute tools if needed, repeat
        loop {
            // Check tool call limit to prevent infinite loops
            if tool_call_count >= self.max_tool_calls {
                events.push(AgentEvent::Error {
                    message: "Maximum tool calls exceeded".to_string(),
                });
                break;
            }

            // Get model response (with or without pending tool results)
            events.push(AgentEvent::ModelStarted);

            let result = if pending_tool_results.is_empty() {
                self.get_model_response(conversation, tools.clone()).await
            } else {
                self.get_model_response_with_results(
                    conversation,
                    pending_tool_results.clone(),
                    // TODO: thread the previous turn's tool_calls here so the
                    // assistant tool_calls message precedes tool results (the
                    // loop_engine path already does this; agent path pending).
                    Vec::new(),
                    tools.clone(),
                )
                .await
            };

            match result {
                Ok(response) => {
                    events.push(AgentEvent::ModelFinished {
                        full_response: response.text.clone(),
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
                                            events.push(AgentEvent::ToolConfirmed {
                                                id: call.id.clone(),
                                            });
                                        } else {
                                            // In auto mode, deny - REPL should handle confirmation
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

                            // Execute the tool
                            let result =
                                self.execute_tool(&call, workspace.clone(), session_id.clone());
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
                    events.push(AgentEvent::Error {
                        message: e.to_string(),
                    });
                    break;
                }
            }
        }

        events
    }

    /// Get model response (initial call without tool results).
    ///
    /// Sends the conversation to the LLM with tool definitions and
    /// returns the response text and any tool calls.
    async fn get_model_response(
        &self,
        conversation: &Conversation,
        tools: Vec<protocol_interface::ToolDef>,
    ) -> anyhow::Result<ModelResponse> {
        let (stream, tool_calls) = self
            .client
            .chat_stream_with_tools(conversation, tools)
            .await?;

        let mut text = String::new();
        let mut stream = Box::pin(stream);

        // Collect streaming response
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

    /// Get model response with tool execution results.
    ///
    /// Sends the conversation plus tool results back to the LLM and
    /// returns the response text and any additional tool calls.
    async fn get_model_response_with_results(
        &self,
        conversation: &Conversation,
        tool_results: Vec<(String, String, String)>,
        assistant_tool_calls: Vec<ToolCall>,
        tools: Vec<protocol_interface::ToolDef>,
    ) -> anyhow::Result<ModelResponse> {
        let (stream, tool_calls) = self
            .client
            .continue_with_tool_results(conversation, tool_results, assistant_tool_calls, tools)
            .await?;

        let mut text = String::new();
        let mut stream = Box::pin(stream);

        // Collect streaming response
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

    /// Execute a tool call and return the result.
    ///
    /// Looks up the tool in the registry and executes it with the given
    /// arguments and context. Returns an error result if the tool is not found.
    pub async fn execute_tool(
        &self,
        tool_call: &ToolCall,
        workspace: std::path::PathBuf,
        session_id: String,
    ) -> ToolResult {
        let ctx = ToolContext::new(workspace, session_id, protocol_interface::RuntimeMode::Chat);

        if let Some(tool) = self.registry.get(&tool_call.name) {
            tool.execute(tool_call.args.clone(), ctx)
                .await
                .unwrap_or_else(|e| ToolResult::error(e.to_string()))
        } else {
            ToolResult::error(format!("Unknown tool: {}", tool_call.name))
        }
    }
}
