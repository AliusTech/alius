//! Local Runtime Manager facade.
//!
//! This module is the product-facing local entrypoint for the Core Runtime.
//! It owns runtime assembly for local callers, then routes all execution
//! through `ProtocolInterface<CoreRuntime>`.

use std::path::PathBuf;
use std::sync::Arc;

use protocol_interface::core::*;
use protocol_interface::{
    ConfigSnapshot, HealthReport, LogQuery, LogRecord, MemoryEntry, ModelInfo, ProtocolContext,
    ProtocolEnvelope, ProtocolInterface, ToolInfo,
};
use runtime_config::Settings;
use runtime_model::LlmClient;
use runtime_tools::ToolPackageResolver;

use crate::{CoreRuntime, CoreRuntimeBuilder};

#[cfg(feature = "mcp")]
use crate::mcp_manager::McpManager;

/// Caller context used by the local manager when creating protocol envelopes.
#[derive(Debug, Clone)]
pub struct RuntimeManagerContext {
    pub origin: Origin,
    pub capability_scope: CapabilityScope,
}

impl RuntimeManagerContext {
    /// Local CLI context.
    pub fn local_cli() -> Self {
        Self {
            origin: Origin::LocalCli,
            capability_scope: CapabilityScope::local_cli(),
        }
    }

    /// Local TUI context.
    pub fn local_tui() -> Self {
        Self {
            origin: Origin::LocalTui,
            capability_scope: CapabilityScope::local_tui(),
        }
    }

    /// JSON-RPC adapter context.
    pub fn json_rpc() -> Self {
        Self {
            origin: Origin::JsonRpc,
            capability_scope: CapabilityScope::local_cli(),
        }
    }
}

/// Unified local Runtime Manager for product entrypoints.
pub struct CoreRuntimeManager {
    interface: ProtocolInterface<CoreRuntime>,
    workspace_root: PathBuf,
    context: RuntimeManagerContext,
    #[cfg(feature = "mcp")]
    mcp_manager: Option<Arc<McpManager>>,
}

impl CoreRuntimeManager {
    /// Build a local CLI manager from workspace root and settings.
    pub fn new_local(
        workspace_root: impl Into<PathBuf>,
        settings: Settings,
    ) -> Result<Self, ProtocolError> {
        Self::new_with_context(workspace_root, settings, RuntimeManagerContext::local_cli())
    }

    /// Build a local TUI manager from workspace root and settings.
    pub fn new_local_tui(
        workspace_root: impl Into<PathBuf>,
        settings: Settings,
    ) -> Result<Self, ProtocolError> {
        Self::new_with_context(workspace_root, settings, RuntimeManagerContext::local_tui())
    }

    /// Build a manager with an explicit caller context.
    pub fn new_with_context(
        workspace_root: impl Into<PathBuf>,
        settings: Settings,
        context: RuntimeManagerContext,
    ) -> Result<Self, ProtocolError> {
        let workspace_root = workspace_root.into();
        let client = LlmClient::new(settings.llm.clone())
            .map_err(|e| ProtocolError::Internal(format!("model client: {e}")))?;

        let registry = ToolPackageResolver::new(workspace_root.clone()).build_registry_lossy();

        let runtime = CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new(&workspace_root))
            .settings(settings)
            .client(client)
            .tool_registry_arc(Arc::new(registry))
            .build()?;

        let manager = Self::from_runtime_with_context(workspace_root, runtime, context);

        // Start MCP background initialization if feature is enabled
        #[cfg(feature = "mcp")]
        manager.start_mcp_init();

        Ok(manager)
    }

    /// Wrap an existing runtime with the default local CLI context.
    pub fn from_runtime(workspace_root: impl Into<PathBuf>, runtime: CoreRuntime) -> Self {
        Self::from_runtime_with_context(workspace_root, runtime, RuntimeManagerContext::local_cli())
    }

    /// Wrap an existing runtime with an explicit caller context.
    pub fn from_runtime_with_context(
        workspace_root: impl Into<PathBuf>,
        runtime: CoreRuntime,
        context: RuntimeManagerContext,
    ) -> Self {
        #[cfg(feature = "mcp")]
        let mcp_manager = {
            let manager = Arc::new(McpManager::new());
            // Note: Background initialization will be triggered separately
            Some(manager)
        };

        #[cfg(not(feature = "mcp"))]
        let mcp_manager = ();
        Self {
            interface: ProtocolInterface::new(runtime),
            workspace_root: workspace_root.into(),
            context,
            #[cfg(feature = "mcp")]
            mcp_manager,
        }
    }

    /// Access the wrapped runtime for integration code and tests.
    pub fn runtime(&self) -> Arc<CoreRuntime> {
        self.interface.runtime()
    }

    /// Run text input and return the collected event envelopes.
    pub fn run_text(
        &self,
        text: &str,
        mode: RuntimeMode,
    ) -> Result<Vec<ProtocolEnvelope<CoreEvent>>, ProtocolError> {
        let request = self.run_loop_request(text, mode)?;
        let envelope = self.request_envelope(request);
        let run_ref = self.interface.start(envelope)?;
        self.interface.subscribe(&run_ref)
    }

    /// Start a streaming execution and return a run ref plus event receiver.
    pub fn start_streaming(
        &self,
        text: &str,
        mode: RuntimeMode,
    ) -> Result<(RunRef, tokio::sync::mpsc::UnboundedReceiver<CoreEvent>), ProtocolError> {
        let request = self.run_loop_request(text, mode)?;
        self.interface
            .start_streaming(self.request_envelope(request))
    }

    /// Subscribe to run events.
    pub fn subscribe(
        &self,
        run_ref: &RunRef,
    ) -> Result<Vec<ProtocolEnvelope<CoreEvent>>, ProtocolError> {
        self.interface.subscribe(run_ref)
    }

    /// Cancel a run through the protocol interface.
    pub fn cancel(&self, run_ref: &RunRef, reason: Option<String>) -> Result<(), ProtocolError> {
        // NOTE: Stage B cancel_pending_confirmations will be wired here once
        // CoreRuntimeManager gains a session_manager handle (B4 follow-up).
        self.interface.cancel(run_ref, reason)
    }

    /// Read current runtime configuration.
    pub fn config_read(&self) -> Result<ConfigSnapshot, ProtocolError> {
        self.interface.config_read(&self.protocol_context())
    }

    /// Update a runtime configuration key.
    pub fn config_update(&self, key: &str, value: serde_json::Value) -> Result<(), ProtocolError> {
        self.interface
            .config_update(&self.protocol_context(), key, value)
    }

    /// List available models through the runtime.
    pub fn model_list(&self) -> Result<Vec<ModelInfo>, ProtocolError> {
        self.interface.model_list(&self.protocol_context())
    }

    /// Close a session.
    pub fn close_session(&self, session_ref: &SessionRef) -> Result<(), ProtocolError> {
        self.interface
            .close_session(&self.protocol_context(), session_ref)
    }

    /// Clear a session conversation.
    pub fn clear_conversation(&self, session_ref: &SessionRef) -> Result<(), ProtocolError> {
        self.interface
            .clear_conversation(&self.protocol_context(), session_ref)
    }

    /// Save a memory entry.
    pub fn memory_save(&self, text: &str, tags: Vec<String>) -> Result<(), ProtocolError> {
        self.interface
            .memory_save(&self.protocol_context(), text, tags)
    }

    /// List memory entries.
    pub fn memory_list(&self) -> Result<Vec<MemoryEntry>, ProtocolError> {
        self.interface.memory_list(&self.protocol_context())
    }

    /// Clear memory entries.
    pub fn memory_clear(&self) -> Result<(), ProtocolError> {
        self.interface.memory_clear(&self.protocol_context())
    }

    /// List registered tools.
    pub fn tool_list(&self) -> Result<Vec<ToolInfo>, ProtocolError> {
        self.interface.tool_list(&self.protocol_context())
    }

    /// Start a review run.
    pub fn review_start(&self, session_ref: &SessionRef) -> Result<RunRef, ProtocolError> {
        self.interface
            .review_start(&self.protocol_context(), session_ref)
    }

    /// Query log records.
    pub fn query_logs(&self, query: LogQuery) -> Result<Vec<LogRecord>, ProtocolError> {
        self.interface.query_logs(&self.protocol_context(), query)
    }

    /// Run a health check.
    pub fn health_check(&self) -> Result<HealthReport, ProtocolError> {
        self.interface.health_check(&self.protocol_context())
    }

    /// Get MCP initialization status (only available with mcp feature)
    #[cfg(feature = "mcp")]
    pub async fn mcp_status(&self) -> crate::mcp_manager::McpStatus {
        if let Some(manager) = &self.mcp_manager {
            manager.status().await
        } else {
            crate::mcp_manager::McpStatus::NotStarted
        }
    }

    /// Start MCP background initialization (only available with mcp feature).
    /// Passes the shared ToolRegistry so MCP tools are registered directly
    /// into the runtime tool chain.
    #[cfg(feature = "mcp")]
    pub fn start_mcp_init(&self) {
        if let Some(manager) = &self.mcp_manager {
            if let Some(registry) = self.runtime().tool_registry() {
                manager.start_background_init(registry);
                tracing::info!("MCP background initialization started");
            } else {
                tracing::warn!("MCP initialization skipped: no tool registry available");
            }
        }
    }

    fn run_loop_request(
        &self,
        text: &str,
        mode: RuntimeMode,
    ) -> Result<CoreRequest, ProtocolError> {
        let policy = match mode {
            RuntimeMode::Chat => LoopPolicy::chat(),
            RuntimeMode::Plan => LoopPolicy::plan(),
        };
        CoreRequest::run_loop(text, mode, policy)
    }

    fn request_envelope(&self, request: CoreRequest) -> ProtocolEnvelope<CoreRequest> {
        ProtocolEnvelope::new(
            self.context.origin.clone(),
            self.context.capability_scope.clone(),
            request,
        )
        .with_workspace_root(&self.workspace_root)
    }

    fn protocol_context(&self) -> ProtocolContext {
        ProtocolContext {
            origin: self.context.origin.clone(),
            capability_scope: self.context.capability_scope.clone(),
            workspace_root: Some(self.workspace_root.clone()),
        }
    }
}
