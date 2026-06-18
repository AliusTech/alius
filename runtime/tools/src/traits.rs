//! Tool trait definition

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::PermissionLevel;
use protocol_interface::core::ToolSource;
use protocol_interface::{AliusError, PermissionStrategy, RuntimeMode};

/// Context for tool execution
pub struct ToolContext {
    pub workspace: std::path::PathBuf,
    pub session_id: String,
    pub working_directory: std::path::PathBuf,
    /// Runtime mode requested by the caller.
    pub mode: RuntimeMode,
    /// Bypass Alius confirmation and permission gates for this execution.
    pub permission_strategy: PermissionStrategy,
}

impl ToolContext {
    pub fn new(workspace: std::path::PathBuf, session_id: String, mode: RuntimeMode) -> Self {
        let permission_strategy = match mode {
            RuntimeMode::Bypass => PermissionStrategy::BypassPermissions,
            RuntimeMode::Chat | RuntimeMode::Plan => PermissionStrategy::AcceptEdits,
        };
        Self::new_with_permission_strategy(workspace, session_id, mode, permission_strategy)
    }

    pub fn new_with_permission_strategy(
        workspace: std::path::PathBuf,
        session_id: String,
        mode: RuntimeMode,
        permission_strategy: PermissionStrategy,
    ) -> Self {
        Self {
            working_directory: workspace.clone(),
            workspace,
            session_id,
            mode,
            permission_strategy,
        }
    }

    pub fn bypass_permissions(&self) -> bool {
        self.permission_strategy == PermissionStrategy::BypassPermissions
    }
}

/// Tool execution result
pub struct ToolResult {
    pub output: String,
    pub success: bool,
    pub metadata: Option<JsonValue>,
}

impl ToolResult {
    pub fn success(output: String) -> Self {
        Self {
            output,
            success: true,
            metadata: None,
        }
    }

    pub fn error(message: String) -> Self {
        Self {
            output: message,
            success: false,
            metadata: None,
        }
    }

    pub fn with_metadata(self, metadata: JsonValue) -> Self {
        Self {
            metadata: Some(metadata),
            ..self
        }
    }
}

/// Confirmation request for destructive operations
pub struct ConfirmationRequest {
    pub tool_name: String,
    pub operation: String,
    pub details: String,
}

/// Alius tool trait
#[async_trait]
pub trait AliusTool: Send + Sync {
    /// Tool name (used in function calling)
    fn name(&self) -> &'static str;

    /// Tool description (shown to LLM)
    fn description(&self) -> &'static str;

    /// JSON schema for input parameters
    fn input_schema(&self) -> JsonValue;

    /// Tool source (default: RustWasm for native/WASM tools).
    /// MCP adapters should override to return `ToolSource::Mcp`.
    fn source(&self) -> ToolSource {
        ToolSource::RustWasm
    }

    /// Required permission level (default: Read)
    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    /// Whether this operation requires user confirmation
    fn requires_confirmation(&self, _args: &JsonValue) -> bool {
        false
    }

    /// Whether this invocation needs the user's approval before executing,
    /// given the current runtime mode. Default: never. Tools like `shell`
    /// (high-risk per Shell Gate) and `write_file` override this to return
    /// true when `mode == RuntimeMode::Plan`.
    fn preview_confirmation(&self, _args: &JsonValue, _mode: RuntimeMode) -> bool {
        false
    }

    /// Get confirmation request details
    fn confirmation_request(&self, args: &JsonValue) -> Option<ConfirmationRequest> {
        if self.requires_confirmation(args) {
            Some(ConfirmationRequest {
                tool_name: self.name().to_string(),
                operation: self.name().to_string(),
                details: serde_json::to_string_pretty(args).unwrap_or_default(),
            })
        } else {
            None
        }
    }

    /// Execute the tool
    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError>;
}
