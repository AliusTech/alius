//! Tool registry
//!
//! Uses interior `RwLock` so tools can be registered after the registry
//! is wrapped in `Arc` — required for async MCP tool registration.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::AliusTool;
use protocol_interface::core::ToolInfo;
use protocol_interface::ToolDef;

/// Tool registry for managing available tools.
///
/// Thread-safe via interior `RwLock`. `register()` takes `&self` so MCP
/// background tasks can add tools after the registry is `Arc`-wrapped.
pub struct ToolRegistry {
    tools: RwLock<HashMap<String, Arc<dyn AliusTool>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: RwLock::new(HashMap::new()),
        }
    }

    /// Register a tool implementation. Returns `Err` if a tool with the same
    /// name is already registered — callers must not silently shadow built-in
    /// tools (e.g. native `shell`) with WASM or external implementations.
    pub fn register<T>(&self, tool: T) -> Result<(), String>
    where
        T: AliusTool + 'static,
    {
        let mut tools = self.tools.write().unwrap();
        let name = tool.name().to_string();
        if tools.contains_key(&name) {
            return Err(format!("tool '{}' is already registered", name));
        }
        tools.insert(name, Arc::new(tool));
        Ok(())
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn AliusTool>> {
        let tools = self.tools.read().unwrap();
        tools.get(name).cloned()
    }

    /// Check if a tool exists
    pub fn has(&self, name: &str) -> bool {
        let tools = self.tools.read().unwrap();
        tools.contains_key(name)
    }

    /// List all tool names
    pub fn list_names(&self) -> Vec<String> {
        let tools = self.tools.read().unwrap();
        tools.keys().cloned().collect()
    }

    /// Get all tools as OpenAI function definitions
    pub fn to_openai_tools(&self) -> Vec<serde_json::Value> {
        let tools = self.tools.read().unwrap();
        tools
            .values()
            .map(|tool| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": tool.name(),
                        "description": tool.description(),
                        "parameters": tool.input_schema(),
                    }
                })
            })
            .collect()
    }

    /// Get all tools as provider-agnostic ToolDef list
    pub fn to_tool_defs(&self) -> Vec<ToolDef> {
        let tools = self.tools.read().unwrap();
        tools
            .values()
            .map(|tool| ToolDef {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.input_schema(),
            })
            .collect()
    }

    /// Get all tools as `ToolInfo` list with source metadata.
    /// Each tool's `source()` method determines the `ToolSource` value.
    pub fn to_tool_infos(&self) -> Vec<ToolInfo> {
        let tools = self.tools.read().unwrap();
        tools
            .values()
            .map(|tool| ToolInfo {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                source: tool.source(),
            })
            .collect()
    }
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::native;
    use crate::testing::FakeTool;
    use protocol_interface::core::ToolSource;

    #[test]
    fn test_native_tools_registered() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        assert!(registry.has("shell"));
        assert!(registry.has("read_file"));
        assert!(registry.has("write_file"));
        assert!(registry.has("list_dir"));
        assert!(registry.has("edit_file"));
    }

    #[test]
    fn test_get_native_tools() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        let shell = registry.get("shell");
        assert!(shell.is_some());
        assert_eq!(shell.unwrap().name(), "shell");

        let read_file = registry.get("read_file");
        assert!(read_file.is_some());
        assert_eq!(read_file.unwrap().name(), "read_file");
    }

    #[test]
    fn test_to_tool_defs_includes_native() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        let tool_defs = registry.to_tool_defs();
        let names: Vec<String> = tool_defs.iter().map(|t| t.name.clone()).collect();

        assert!(names.contains(&"shell".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"write_file".to_string()));
        assert!(names.contains(&"list_dir".to_string()));
        assert!(names.contains(&"edit_file".to_string()));
    }

    #[test]
    fn test_duplicate_name_rejected() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        // Attempting to register a tool with the same name as a native tool
        // must fail — this prevents WASM plugins from shadowing built-in tools.
        let result = registry.register(FakeTool::new("shell"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already registered"));

        // The original native tool must still be present.
        let shell = registry.get("shell").expect("native shell must survive");
        assert_eq!(shell.name(), "shell");
    }

    #[test]
    fn test_all_native_names_rejected_on_duplicate() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        for name in &["shell", "read_file", "write_file", "list_dir", "edit_file"] {
            let result = registry.register(FakeTool::new(name));
            assert!(result.is_err(), "duplicate '{}' must be rejected", name);
        }
    }

    #[test]
    fn test_register_after_arc_wrap() {
        // Verify that register works after the registry is wrapped in Arc
        // (the key requirement for MCP background registration).
        let registry = Arc::new(ToolRegistry::new());
        native::register_native_tools(&registry);

        // Register an extra tool via Arc reference.
        let result = registry.register(FakeTool::new("extra_tool"));
        assert!(result.is_ok());
        assert!(registry.has("extra_tool"));
        assert!(registry.has("shell")); // native tools still present
    }

    #[test]
    fn test_native_tools_have_rustwasm_source() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        let infos = registry.to_tool_infos();
        let shell_info = infos.iter().find(|i| i.name == "shell").unwrap();
        assert_eq!(shell_info.source, ToolSource::RustWasm);
    }

    #[test]
    fn test_to_tool_infos_includes_source() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        let infos = registry.to_tool_infos();
        assert!(!infos.is_empty());
        // All native tools should have RustWasm source.
        for info in &infos {
            assert_eq!(info.source, ToolSource::RustWasm);
        }
    }

    #[test]
    fn test_mcp_tool_has_mcp_source() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);
        registry
            .register(FakeTool::new("mcp_search").as_mcp())
            .unwrap();

        let infos = registry.to_tool_infos();
        let native_count = infos
            .iter()
            .filter(|i| i.source == ToolSource::RustWasm)
            .count();
        let mcp_count = infos.iter().filter(|i| i.source == ToolSource::Mcp).count();

        assert_eq!(native_count, 5); // shell, read_file, write_file, list_dir, edit_file
        assert_eq!(mcp_count, 1); // mcp_search
        assert_eq!(infos.len(), 6);
    }

    #[test]
    fn test_mcp_and_native_sources_coexist() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);
        registry
            .register(FakeTool::new("mcp_tool_1").as_mcp())
            .unwrap();
        registry
            .register(FakeTool::new("mcp_tool_2").as_mcp())
            .unwrap();

        let infos = registry.to_tool_infos();
        let mcp_tools: Vec<_> = infos
            .iter()
            .filter(|i| i.source == ToolSource::Mcp)
            .collect();
        assert_eq!(mcp_tools.len(), 2);
        assert!(mcp_tools.iter().any(|t| t.name == "mcp_tool_1"));
        assert!(mcp_tools.iter().any(|t| t.name == "mcp_tool_2"));
    }

    #[test]
    fn test_mcp_duplicate_name_rejected() {
        let registry = ToolRegistry::new();
        native::register_native_tools(&registry);

        // MCP tool with same name as native tool should be rejected.
        let result = registry.register(FakeTool::new("shell").as_mcp());
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already registered"));

        // Native tool should still be there with RustWasm source.
        let infos = registry.to_tool_infos();
        let shell = infos.iter().find(|i| i.name == "shell").unwrap();
        assert_eq!(shell.source, ToolSource::RustWasm);
    }
}
