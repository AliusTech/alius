//! Protocol Bridge — CLI-friendly wrapper around the Protocol Interface Layer.
//!
//! Provides a simplified API for the CLI to interact with the Core Runtime
//! through the protocol boundary, handling envelope construction and
//! event parsing internally.

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Result;
use core_runtime::{CoreRuntime, CoreRuntimeBuilder};
use protocol_interface::core::*;
use protocol_interface::{
    ConfigSnapshot, HealthReport, ModelInfo, ProtocolContext, ProtocolInterface,
};
use runtime_config::Settings;
use runtime_model::LlmClient;
use runtime_tools::ToolRegistry;

/// CLI-friendly bridge to the Protocol Interface Layer.
///
/// Wraps ProtocolInterface and provides methods that handle envelope
/// construction, capability checks, and event parsing transparently.
pub struct ProtocolBridge {
    interface: ProtocolInterface<CoreRuntime>,
    workspace_root: PathBuf,
}

impl ProtocolBridge {
    /// Create a new ProtocolBridge.
    ///
    /// The runtime is built from the given settings and LLM client.
    pub fn new(
        workspace_root: PathBuf,
        settings: Settings,
        client: LlmClient,
        registry: Option<Arc<ToolRegistry>>,
    ) -> Result<Self> {
        let workspace_ref = WorkspaceRef::new(&workspace_root);
        let mut builder = CoreRuntimeBuilder::new()
            .workspace_ref(workspace_ref)
            .settings(settings)
            .client(client);
        if let Some(reg) = registry {
            builder = builder.tool_registry_arc(reg);
        }
        let runtime = builder
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build CoreRuntime: {}", e))?;

        Ok(Self {
            interface: ProtocolInterface::new(runtime),
            workspace_root,
        })
    }

    /// Create a new ProtocolBridge from pre-built runtime.
    #[allow(dead_code)]
    pub fn from_runtime(workspace_root: PathBuf, runtime: CoreRuntime) -> Self {
        Self {
            interface: ProtocolInterface::new(runtime),
            workspace_root,
        }
    }

    /// Access the underlying runtime for advanced usage (e.g., rebuilding client).
    #[allow(dead_code)]
    pub fn runtime(&self) -> Arc<CoreRuntime> {
        self.interface.runtime()
    }

    /// Send a Chat-mode message and return the full event list.
    #[allow(dead_code)]
    pub fn send_message(&self, text: &str) -> Result<Vec<ProtocolEnvelope<CoreEvent>>> {
        self.send_message_with_mode(text, RuntimeMode::Chat)
    }

    /// Send a message with an explicit mode (Chat or Plan) and return the full event list.
    pub fn send_message_with_mode(
        &self,
        text: &str,
        mode: RuntimeMode,
    ) -> Result<Vec<ProtocolEnvelope<CoreEvent>>> {
        let policy = match mode {
            RuntimeMode::Chat => LoopPolicy::chat(),
            RuntimeMode::Plan => LoopPolicy::plan(),
        };
        let request = CoreRequest::run_loop(text, mode, policy)
            .map_err(|e| anyhow::anyhow!("Invalid request: {}", e))?;

        let envelope =
            ProtocolEnvelope::new(Origin::LocalTui, CapabilityScope::local_tui(), request)
                .with_workspace_root(&self.workspace_root);

        let run_ref = self
            .interface
            .start(envelope)
            .map_err(|e| anyhow::anyhow!("Protocol start failed: {}", e))?;

        let events = self
            .interface
            .subscribe(&run_ref)
            .map_err(|e| anyhow::anyhow!("Protocol subscribe failed: {}", e))?;

        Ok(events)
    }

    /// Send a Chat-mode message with a streaming callback for each delta.
    ///
    /// The callback is called for each ModelDelta event with the text chunk.
    /// Returns the full response text after all events are processed.
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

    /// Read current configuration through the protocol layer.
    #[allow(dead_code)]
    pub fn config_read(&self) -> Result<ConfigSnapshot> {
        let ctx = self.make_context();
        self.interface
            .config_read(&ctx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Update a configuration key through the protocol layer.
    #[allow(dead_code)]
    pub fn config_update(&self, key: &str, value: serde_json::Value) -> Result<()> {
        let ctx = self.make_context();
        self.interface
            .config_update(&ctx, key, value)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// List available models through the protocol layer.
    #[allow(dead_code)]
    pub fn model_list(&self) -> Result<Vec<ModelInfo>> {
        let ctx = self.make_context();
        self.interface
            .model_list(&ctx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Run a health check through the protocol layer.
    #[allow(dead_code)]
    pub fn health_check(&self) -> Result<HealthReport> {
        let ctx = self.make_context();
        self.interface
            .health_check(&ctx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Close a session through the protocol layer.
    #[allow(dead_code)]
    pub fn close_session(&self, session_ref: &SessionRef) -> Result<()> {
        let ctx = self.make_context();
        self.interface
            .close_session(&ctx, session_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Clear conversation history through the protocol layer.
    pub fn clear_conversation(&self, session_ref: &SessionRef) -> Result<()> {
        let ctx = self.make_context();
        self.interface
            .clear_conversation(&ctx, session_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Save a memory entry through the protocol layer.
    pub fn memory_save(&self, text: &str, tags: Vec<String>) -> Result<()> {
        let ctx = self.make_context();
        self.interface
            .memory_save(&ctx, text, tags)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// List all memory entries through the protocol layer.
    pub fn memory_list(&self) -> Result<Vec<MemoryEntry>> {
        let ctx = self.make_context();
        self.interface
            .memory_list(&ctx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Clear all memory entries through the protocol layer.
    pub fn memory_clear(&self) -> Result<()> {
        let ctx = self.make_context();
        self.interface
            .memory_clear(&ctx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// List available tools through the protocol layer.
    pub fn tool_list(&self) -> Result<Vec<ToolInfo>> {
        let ctx = self.make_context();
        self.interface
            .tool_list(&ctx)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Start a code review through the protocol layer.
    pub fn review_start(&self, session_ref: &SessionRef) -> Result<RunRef> {
        let ctx = self.make_context();
        self.interface
            .review_start(&ctx, session_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Subscribe to run events through the protocol layer.
    pub fn subscribe(&self, run_ref: &RunRef) -> Result<Vec<ProtocolEnvelope<CoreEvent>>> {
        self.interface
            .subscribe(run_ref)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    /// Query log records through the protocol layer.
    #[allow(dead_code)]
    pub fn query_logs(&self, query: LogQuery) -> Result<Vec<LogRecord>> {
        let ctx = self.make_context();
        self.interface
            .query_logs(&ctx, query)
            .map_err(|e| anyhow::anyhow!("{}", e))
    }

    fn make_context(&self) -> ProtocolContext {
        ProtocolContext {
            origin: Origin::LocalTui,
            capability_scope: CapabilityScope::local_tui(),
            workspace_root: Some(self.workspace_root.clone()),
        }
    }
}
