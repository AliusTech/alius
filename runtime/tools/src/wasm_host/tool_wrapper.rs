//! WasmPluginTool — wraps a Rust WASM module tool as an AliusTool.

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use super::host::{call_plugin_tool, PluginToolDef};
use crate::permission::PermissionLevel;
use crate::traits::{AliusTool, ToolContext, ToolResult};
use protocol_interface::AliusError;

/// A Rust WASM module tool adapter that implements AliusTool.
///
/// Each WasmPluginTool represents one tool from one Rust WASM module. The
/// adapter delegates execution to the module's `alius_plugin_call_tool` export.
#[derive(Clone)]
pub struct WasmPluginTool {
    wasm_bytes: Vec<u8>,
    tool_def: PluginToolDef,
    // Leaked &'static str to satisfy AliusTool::name() signature
    name_static: &'static str,
    desc_static: &'static str,
}

impl WasmPluginTool {
    /// Create a new WasmPluginTool from WASM bytes and a tool definition.
    pub fn new(wasm_bytes: Vec<u8>, tool_def: PluginToolDef) -> Self {
        let name_static = Box::leak(tool_def.name.clone().into_boxed_str());
        let desc_static = Box::leak(tool_def.description.clone().into_boxed_str());
        Self {
            wasm_bytes,
            tool_def,
            name_static,
            desc_static,
        }
    }

    /// Discover all tools in a Rust WASM module and return WasmPluginTool instances.
    pub fn from_wasm_bytes(wasm_bytes: &[u8]) -> Result<Vec<Self>, anyhow::Error> {
        let tools = super::host::list_plugin_tools(wasm_bytes)?;
        Ok(tools
            .into_iter()
            .map(|td| Self::new(wasm_bytes.to_vec(), td))
            .collect())
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

    async fn execute(&self, args: JsonValue, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let result = call_plugin_tool(&self.wasm_bytes, &self.tool_def.name, &args)
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
        let result = WasmPluginTool::from_wasm_bytes(&wasm);
        assert!(result.is_err());
    }
}
