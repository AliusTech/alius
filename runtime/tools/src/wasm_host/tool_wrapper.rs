//! WasmPluginTool — wraps a Rust WASM module tool as an AliusTool.

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use super::host::{call_plugin_tool_with_state, PluginToolDef, ResolvedPluginPermissions};
use super::imports::WasmHostState;
use crate::permission::PermissionLevel;
use crate::traits::{AliusTool, ToolContext, ToolResult};
use protocol_interface::AliusError;

/// A Rust WASM module tool adapter that implements AliusTool.
///
/// Each WasmPluginTool represents one tool from one Rust WASM module. The
/// adapter delegates execution to the module's `alius_plugin_call_tool` export
/// through host imports that enforce permissions, Shell Gate, and audit logging.
#[derive(Clone)]
pub struct WasmPluginTool {
    wasm_bytes: Vec<u8>,
    tool_def: PluginToolDef,
    /// Resolved permissions from the plugin manifest.
    permissions: ResolvedPluginPermissions,
    /// Plugin identifier for audit trail.
    plugin_id: String,
    // Leaked &'static str to satisfy AliusTool::name() signature
    name_static: &'static str,
    desc_static: &'static str,
}

impl WasmPluginTool {
    /// Create a new WasmPluginTool from WASM bytes, tool definition, and resolved permissions.
    pub fn new(
        wasm_bytes: Vec<u8>,
        tool_def: PluginToolDef,
        permissions: ResolvedPluginPermissions,
        plugin_id: String,
    ) -> Self {
        let name_static = Box::leak(tool_def.name.clone().into_boxed_str());
        let desc_static = Box::leak(tool_def.description.clone().into_boxed_str());
        Self {
            wasm_bytes,
            tool_def,
            permissions,
            plugin_id,
            name_static,
            desc_static,
        }
    }

    /// Discover all tools in a Rust WASM module and return WasmPluginTool instances.
    ///
    /// `permissions` and `plugin_id` are attached to every tool so that execution
    /// goes through host imports with permission enforcement and audit logging.
    pub fn from_wasm_bytes(
        wasm_bytes: &[u8],
        permissions: ResolvedPluginPermissions,
        plugin_id: String,
    ) -> Result<Vec<Self>, anyhow::Error> {
        let tools = super::host::list_plugin_tools(wasm_bytes)?;
        Ok(tools
            .into_iter()
            .map(|td| {
                Self::new(
                    wasm_bytes.to_vec(),
                    td,
                    permissions.clone(),
                    plugin_id.clone(),
                )
            })
            .collect())
    }

    /// Backward-compatible discovery without permissions (legacy path).
    ///
    /// Tools discovered this way will execute without host imports — no
    /// permission checks, no audit, no Shell Gate. Prefer `from_wasm_bytes`
    /// with explicit permissions for production use.
    pub fn from_wasm_bytes_legacy(wasm_bytes: &[u8]) -> Result<Vec<Self>, anyhow::Error> {
        Self::from_wasm_bytes(
            wasm_bytes,
            ResolvedPluginPermissions::default(),
            "unknown".to_string(),
        )
    }
}

#[async_trait]
impl AliusTool for WasmPluginTool {
    fn name(&self) -> &'static str {
        self.name_static
    }

    fn description(&self) -> &'static str {
        self.desc_static
    }

    fn input_schema(&self) -> JsonValue {
        self.tool_def.input_schema.clone()
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn requires_confirmation(&self, _args: &JsonValue) -> bool {
        self.tool_def.requires_confirmation
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        // Build WasmHostState for permission-enforced execution via host imports.
        // Uses workspace from ToolContext (runtime-provided) and permissions from
        // the plugin manifest (stored at construction time).
        // Even when permissions are empty, we still use host state so that
        // audit trail is recorded and all capability calls are properly denied.
        let host_state = WasmHostState::new(
            self.permissions.clone(),
            self.plugin_id.clone(),
            ctx.workspace.clone(),
            ctx.session_id.clone(),
        );

        let result = call_plugin_tool_with_state(
            &self.wasm_bytes,
            &self.tool_def.name,
            &args,
            host_state,
        )
        .map_err(|e| AliusError::Agent(e.to_string()))?;

        let output = result
            .get("output")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let success = result
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        Ok(ToolResult {
            output,
            success,
            metadata: Some(result),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_wasm_bytes_missing_exports() {
        let wasm = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap();
        let result = WasmPluginTool::from_wasm_bytes_legacy(&wasm);
        assert!(result.is_err());
    }
}
