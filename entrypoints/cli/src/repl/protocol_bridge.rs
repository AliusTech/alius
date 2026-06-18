//! CLI-friendly compatibility wrapper around `CoreRuntimeManager`.
//!
//! The manager owns runtime assembly; this bridge keeps the existing REPL/TUI
//! call sites small while preserving the product -> protocol -> core boundary.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use core_runtime::{CoreRuntime, CoreRuntimeManager, RuntimeManagerContext};
use protocol_interface::core::*;
use protocol_interface::{
    ConfigSnapshot, HealthReport, LogQuery, LogRecord, MemoryEntry, ModelInfo, ProtocolEnvelope,
    ToolInfo,
};
use runtime_config::Settings;

/// CLI-friendly bridge to the local Runtime Manager.
pub struct ProtocolBridge {
    manager: CoreRuntimeManager,
}

impl ProtocolBridge {
    /// Create a new TUI bridge.
    pub fn new(workspace_root: PathBuf, settings: Settings) -> Result<Self> {
        let manager = CoreRuntimeManager::new_local_tui(workspace_root, settings)
            .map_err(|e| anyhow::anyhow!("Failed to build CoreRuntimeManager: {}", e))?;
        Ok(Self { manager })
    }

    /// Create a new ProtocolBridge from a pre-built runtime.
    #[allow(dead_code)]
    pub fn from_runtime(workspace_root: PathBuf, runtime: CoreRuntime) -> Self {
        Self {
            manager: CoreRuntimeManager::from_runtime_with_context(
                workspace_root,
                runtime,
                RuntimeManagerContext::local_tui(),
            ),
        }
    }

    /// Access the underlying runtime for advanced usage.
    #[allow(dead_code)]
    pub fn runtime(&self) -> Arc<CoreRuntime> {
        self.manager.runtime()
    }

    /// Send a Chat-mode message and return the full event list.
    #[allow(dead_code)]
    pub fn send_message(&self, text: &str) -> Result<Vec<ProtocolEnvelope<CoreEvent>>> {
        self.send_message_with_mode(text, RuntimeMode::Chat)
    }

    /// Send a message with an explicit mode and return the full event list.
    pub fn send_message_with_mode(
        &self,
        text: &str,
        mode: RuntimeMode,
    ) -> Result<Vec<ProtocolEnvelope<CoreEvent>>> {
        self.manager
            .run_text(text, mode)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Start a streaming execution, returning a channel of CoreEvents.
    pub fn start_streaming(
        &self,
        text: &str,
        mode: RuntimeMode,
    ) -> Result<(RunRef, tokio::sync::mpsc::UnboundedReceiver<CoreEvent>)> {
        self.manager
            .start_streaming(text, mode)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Start a streaming execution with an explicit loop policy.
    pub fn start_streaming_with_policy(
        &self,
        text: &str,
        mode: RuntimeMode,
        policy: LoopPolicy,
    ) -> Result<(RunRef, tokio::sync::mpsc::UnboundedReceiver<CoreEvent>)> {
        self.manager
            .start_streaming_with_policy(text, mode, policy)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Send a Chat-mode message with a streaming callback for each delta.
    #[allow(dead_code)]
    pub fn send_message_streaming<F>(&self, text: &str, on_delta: F) -> Result<String>
    where
        F: FnMut(&str),
    {
        self.send_message_streaming_with_mode(text, RuntimeMode::Chat, on_delta)
    }

    /// Send a message with an explicit mode and streaming callback for each delta.
    pub fn send_message_streaming_with_mode<F>(
        &self,
        text: &str,
        mode: RuntimeMode,
        mut on_delta: F,
    ) -> Result<String>
    where
        F: FnMut(&str),
    {
        let events = self.send_message_with_mode(text, mode)?;
        let mut full_response = String::new();

        for envelope in &events {
            match (&envelope.payload.kind, &envelope.payload.payload) {
                (CoreEventKind::ModelDelta, CoreEventPayload::Text { text }) => {
                    full_response.push_str(text);
                    on_delta(text);
                }
                (CoreEventKind::ErrorRaised, CoreEventPayload::Error { message, .. }) => {
                    return Err(anyhow::anyhow!("Model error: {}", message));
                }
                (CoreEventKind::FinalResult, CoreEventPayload::Final { success: false, .. }) => {
                    return Err(anyhow::anyhow!("Run failed"));
                }
                _ => {}
            }
        }

        Ok(full_response)
    }

    /// Read current configuration through the Runtime Manager.
    #[allow(dead_code)]
    pub fn config_read(&self) -> Result<ConfigSnapshot> {
        self.manager
            .config_read()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Update a configuration key through the Runtime Manager.
    #[allow(dead_code)]
    pub fn config_update(&self, key: &str, value: serde_json::Value) -> Result<()> {
        self.manager
            .config_update(key, value)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// List available models through the Runtime Manager.
    pub fn model_list(&self) -> Result<Vec<ModelInfo>> {
        self.manager
            .model_list()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Run a health check through the Runtime Manager.
    #[allow(dead_code)]
    pub fn health_check(&self) -> Result<HealthReport> {
        self.manager
            .health_check()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Close a session through the Runtime Manager.
    #[allow(dead_code)]
    pub fn close_session(&self, session_ref: &SessionRef) -> Result<()> {
        self.manager
            .close_session(session_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Clear conversation history through the Runtime Manager.
    pub fn clear_conversation(&self, session_ref: &SessionRef) -> Result<()> {
        self.manager
            .clear_conversation(session_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Save a memory entry through the Runtime Manager.
    pub fn memory_save(&self, text: &str, tags: Vec<String>) -> Result<()> {
        self.manager
            .memory_save(text, tags)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// List all memory entries through the Runtime Manager.
    pub fn memory_list(&self) -> Result<Vec<MemoryEntry>> {
        self.manager
            .memory_list()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Clear all memory entries through the Runtime Manager.
    pub fn memory_clear(&self) -> Result<()> {
        self.manager
            .memory_clear()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// List available tools through the Runtime Manager.
    pub fn tool_list(&self) -> Result<Vec<ToolInfo>> {
        self.manager
            .tool_list()
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Start a code review through the Runtime Manager.
    pub fn review_start(&self, session_ref: &SessionRef) -> Result<RunRef> {
        self.manager
            .review_start(session_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Subscribe to run events through the Runtime Manager.
    pub fn subscribe(&self, run_ref: &RunRef) -> Result<Vec<ProtocolEnvelope<CoreEvent>>> {
        self.manager
            .subscribe(run_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Cancel a running execution through the Runtime Manager.
    pub fn cancel(&self, run_ref: &RunRef, reason: Option<String>) -> Result<()> {
        self.manager
            .cancel(run_ref, reason)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Respond to a tool confirmation request.
    /// This delivers the user's approval/rejection to the waiting tool execution.
    pub fn respond_confirmation(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        approved: bool,
    ) -> Result<()> {
        self.manager
            .respond_confirmation(run_ref, tool_call_id, approved)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Query log records through the Runtime Manager.
    #[allow(dead_code)]
    pub fn query_logs(&self, query: LogQuery) -> Result<Vec<LogRecord>> {
        self.manager
            .query_logs(query)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::sync::Arc;

    #[test]
    fn test_respond_confirmation_returns_error_for_nonexistent_run() {
        let workspace = PathBuf::from("/tmp/test-bridge");
        let settings = Settings::default();
        let bridge = match ProtocolBridge::new(workspace, settings) {
            Ok(b) => b,
            Err(_) => return, // Skip if no API key
        };

        // respond_confirmation with a nonexistent run_ref should return an error
        let fake_run = RunRef::new();
        let result = bridge.respond_confirmation(&fake_run, "fake-tool-id", true);
        assert!(result.is_err(), "Should fail for nonexistent run_ref");
    }

    #[test]
    fn test_respond_confirmation_returns_error_for_nonexistent_tool() {
        let workspace = PathBuf::from("/tmp/test-bridge");
        let settings = Settings::default();
        let bridge = match ProtocolBridge::new(workspace, settings) {
            Ok(b) => b,
            Err(_) => return,
        };

        // Create a run
        let run_ref = match bridge.send_message_with_mode("test", RuntimeMode::Plan) {
            Ok(events) => events
                .iter()
                .find(|e| e.payload.kind == CoreEventKind::RunStarted)
                .and_then(|e| e.run_ref.clone())
                .unwrap_or_default(),
            Err(_) => return, // Skip if runtime not available
        };

        // respond_confirmation with invalid tool_call_id should fail
        let result = bridge.respond_confirmation(&run_ref, "nonexistent-tool-id", true);
        assert!(result.is_err(), "Should fail for nonexistent tool_call_id");
    }

    /// Streaming acceptance test: approve path.
    /// Verifies the full chain: start streaming → ToolConfirmationRequired →
    /// respond_confirmation(approved) → runtime resumes → tool executes →
    /// ToolCallCompleted(success=true).
    #[tokio::test]
    async fn streaming_confirmation_approve_path() {
        use core_runtime::CoreRuntimeBuilder;
        use runtime_model::LlmClient;
        use runtime_tools::ToolRegistry;

        // Create a tool that requires confirmation.
        struct ConfirmTool;
        #[async_trait::async_trait]
        impl runtime_tools::AliusTool for ConfirmTool {
            fn name(&self) -> &'static str {
                "confirm_tool"
            }
            fn description(&self) -> &'static str {
                "test tool"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object", "properties": {}})
            }
            fn preview_confirmation(
                &self,
                _args: &serde_json::Value,
                _mode: protocol_interface::core::RuntimeMode,
            ) -> bool {
                true // Always requires confirmation
            }
            async fn execute(
                &self,
                _args: serde_json::Value,
                _ctx: runtime_tools::ToolContext,
            ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
                Ok(runtime_tools::ToolResult {
                    output: "tool executed successfully".to_string(),
                    success: true,
                    metadata: None,
                })
            }
        }

        // Create tool registry with confirm_tool + native tools.
        let registry = Arc::new(ToolRegistry::new());
        runtime_tools::native::register_native_tools(&registry);
        registry.register(ConfirmTool).unwrap();

        // Create LLM client with a fake provider that returns tool calls.
        let client = LlmClient::new_with_provider_for_test(
            Box::new(FakeToolCallProvider {
                tool_call_id: "tc-approve".to_string(),
                tool_name: "confirm_tool".to_string(),
            }),
            "test-model",
            protocol_interface::ProviderType::Openai,
        );

        // Build runtime and bridge.
        let tmp = tempfile::TempDir::new().unwrap();
        let runtime = CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new(tmp.path()))
            .settings(runtime_config::Settings::default())
            .client(client)
            .tool_registry_arc(registry)
            .build()
            .unwrap();

        let bridge = ProtocolBridge::from_runtime(tmp.path().to_path_buf(), runtime);

        // Start streaming Plan run.
        let (run_ref, mut rx) = bridge
            .start_streaming_with_policy(
                "execute confirm_tool",
                RuntimeMode::Plan,
                LoopPolicy::plan_accept_edits(),
            )
            .unwrap();

        // Wait for ToolConfirmationRequired event.
        let mut confirmation_event = None;
        while let Ok(event) =
            tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await
        {
            let event = event.expect("channel should not close");
            if event.kind == CoreEventKind::ToolConfirmationRequired {
                confirmation_event = Some(event);
                break;
            }
        }

        let confirm_event = confirmation_event.expect("Should receive ToolConfirmationRequired");
        let tool_call_id = match &confirm_event.payload {
            CoreEventPayload::ToolConfirmation { tool_call_id, .. } => tool_call_id.clone(),
            _ => panic!("Expected ToolConfirmation payload"),
        };

        // Approve the confirmation through bridge.
        let result = bridge.respond_confirmation(&run_ref, &tool_call_id, true);
        assert!(result.is_ok(), "respond_confirmation should succeed");

        // Continue listening for final result.
        let mut final_success = false;
        while let Ok(event) =
            tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await
        {
            let event = event.expect("channel should not close");
            match (&event.kind, &event.payload) {
                (CoreEventKind::ToolCallCompleted, CoreEventPayload::Json { value }) => {
                    assert_eq!(value["success"], true);
                    assert!(value["output"].as_str().unwrap().contains("tool executed"));
                }
                (CoreEventKind::FinalResult, CoreEventPayload::Final { success, .. }) => {
                    final_success = *success;
                    break;
                }
                _ => {}
            }
        }

        assert!(
            final_success,
            "Run should complete successfully after approval"
        );
    }

    /// Streaming acceptance test: deny path.
    /// Verifies: start streaming → ToolConfirmationRequired →
    /// respond_confirmation(denied) → runtime fails → no tool execution.
    #[tokio::test]
    async fn streaming_confirmation_deny_path() {
        use core_runtime::CoreRuntimeBuilder;
        use runtime_model::LlmClient;
        use runtime_tools::ToolRegistry;

        // Create a tool that requires confirmation.
        struct ConfirmTool;
        #[async_trait::async_trait]
        impl runtime_tools::AliusTool for ConfirmTool {
            fn name(&self) -> &'static str {
                "confirm_tool"
            }
            fn description(&self) -> &'static str {
                "test tool"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object", "properties": {}})
            }
            fn preview_confirmation(
                &self,
                _args: &serde_json::Value,
                _mode: protocol_interface::core::RuntimeMode,
            ) -> bool {
                true
            }
            async fn execute(
                &self,
                _args: serde_json::Value,
                _ctx: runtime_tools::ToolContext,
            ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
                // Should NOT be called if denied.
                panic!("Tool should not execute after denial!");
            }
        }

        let registry = Arc::new(ToolRegistry::new());
        runtime_tools::native::register_native_tools(&registry);
        registry.register(ConfirmTool).unwrap();

        let client = LlmClient::new_with_provider_for_test(
            Box::new(FakeToolCallProvider {
                tool_call_id: "tc-deny".to_string(),
                tool_name: "confirm_tool".to_string(),
            }),
            "test-model",
            protocol_interface::ProviderType::Openai,
        );

        let tmp = tempfile::TempDir::new().unwrap();
        let runtime = CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new(tmp.path()))
            .settings(runtime_config::Settings::default())
            .client(client)
            .tool_registry_arc(registry)
            .build()
            .unwrap();

        let bridge = ProtocolBridge::from_runtime(tmp.path().to_path_buf(), runtime);

        let (run_ref, mut rx) = bridge
            .start_streaming_with_policy(
                "execute confirm_tool",
                RuntimeMode::Plan,
                LoopPolicy::plan_accept_edits(),
            )
            .unwrap();

        // Wait for ToolConfirmationRequired.
        let mut confirmation_event = None;
        while let Ok(event) =
            tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await
        {
            let event = event.expect("channel should not close");
            if event.kind == CoreEventKind::ToolConfirmationRequired {
                confirmation_event = Some(event);
                break;
            }
        }

        let confirm_event = confirmation_event.expect("Should receive ToolConfirmationRequired");
        let tool_call_id = match &confirm_event.payload {
            CoreEventPayload::ToolConfirmation { tool_call_id, .. } => tool_call_id.clone(),
            _ => panic!("Expected ToolConfirmation payload"),
        };

        // Deny the confirmation.
        let result = bridge.respond_confirmation(&run_ref, &tool_call_id, false);
        assert!(result.is_ok(), "respond_confirmation should succeed");

        // Verify ToolCallCompleted(success=false) and FinalResult(success=false).
        let mut tool_denied = false;
        let mut final_failed = false;
        while let Ok(event) =
            tokio::time::timeout(std::time::Duration::from_secs(5), rx.recv()).await
        {
            let event = event.expect("channel should not close");
            match (&event.kind, &event.payload) {
                (CoreEventKind::ToolCallCompleted, CoreEventPayload::Json { value }) => {
                    assert_eq!(value["success"], false);
                    assert!(value["denied"].as_bool().unwrap_or(false));
                    tool_denied = true;
                }
                (CoreEventKind::FinalResult, CoreEventPayload::Final { success, .. }) => {
                    final_failed = !success;
                    break;
                }
                _ => {}
            }
        }

        assert!(tool_denied, "Tool should be marked as denied");
        assert!(final_failed, "Run should fail after denial");
    }

    /// Fake LLM provider that returns a tool call for a named tool.
    struct FakeToolCallProvider {
        tool_call_id: String,
        tool_name: String,
    }

    impl runtime_model::LlmProvider for FakeToolCallProvider {
        fn chat_stream<'a>(
            &'a self,
            _conversation: &'a runtime_model::Conversation,
        ) -> std::pin::Pin<
            Box<
                dyn std::future::Future<Output = anyhow::Result<runtime_model::ChatStream>>
                    + Send
                    + 'a,
            >,
        > {
            let stream: runtime_model::ChatStream = Box::pin(futures::stream::iter(vec![
                Ok(runtime_model::ChatEvent::Delta {
                    text: "calling tool".to_string(),
                }),
                Ok(runtime_model::ChatEvent::Done {
                    full_response: "calling tool".to_string(),
                }),
            ]));
            Box::pin(async move { Ok(stream) })
        }

        fn chat_once<'a>(
            &'a self,
            _prompt: &'a str,
            _system: Option<&'a str>,
        ) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<String>> + Send + 'a>>
        {
            Box::pin(async { Ok(String::new()) })
        }

        fn list_models<'a>(
            &'a self,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = anyhow::Result<Vec<String>>> + Send + 'a>,
        > {
            Box::pin(async { Ok(Vec::new()) })
        }

        fn chat_stream_with_tools<'a>(
            &'a self,
            _conversation: &'a runtime_model::Conversation,
            _tools: &'a [protocol_interface::ToolDef],
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = runtime_model::ToolResponse> + Send + 'a>,
        > {
            let tool_calls = vec![runtime_model::ToolCall::new(
                self.tool_call_id.clone(),
                self.tool_name.clone(),
                serde_json::json!({}),
            )];
            let stream: runtime_model::ChatStream = Box::pin(futures::stream::iter(vec![
                Ok(runtime_model::ChatEvent::Delta {
                    text: "requesting tool".to_string(),
                }),
                Ok(runtime_model::ChatEvent::Done {
                    full_response: "requesting tool".to_string(),
                }),
            ]));
            Box::pin(async move { Ok((stream, Some(tool_calls))) })
        }

        fn continue_with_tool_results<'a>(
            &'a self,
            _conversation: &'a runtime_model::Conversation,
            _tool_results: &'a [(String, String, String)],
            _assistant_tool_calls: &'a [runtime_model::ToolCall],
            _tools: &'a [protocol_interface::ToolDef],
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = runtime_model::ToolResponse> + Send + 'a>,
        > {
            let stream: runtime_model::ChatStream = Box::pin(futures::stream::iter(vec![Ok(
                runtime_model::ChatEvent::Done {
                    full_response: String::new(),
                },
            )]));
            Box::pin(async move { Ok((stream, None)) })
        }
    }
}
