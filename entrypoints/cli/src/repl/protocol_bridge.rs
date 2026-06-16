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
}
