//! Tool registry

use std::collections::HashMap;
use std::sync::Arc;

use crate::AliusTool;
use protocol_interface::ToolDef;

/// Tool registry for managing available tools
pub struct ToolRegistry {
    tools: HashMap<String, Arc<dyn AliusTool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self {
            tools: HashMap::new(),
        }
    }

    /// Register a tool implementation.
    pub fn register<T>(&mut self, tool: T)
    where
        T: AliusTool + 'static,
    {
        let name = tool.name();
        self.tools.insert(name.to_string(), Arc::new(tool));
    }

    /// Get a tool by name
    pub fn get(&self, name: &str) -> Option<Arc<dyn AliusTool>> {
        self.tools.get(name).cloned()
    }

    /// Check if a tool exists
    pub fn has(&self, name: &str) -> bool {
        self.tools.contains_key(name)
    }

    /// List all tool names
    pub fn list_names(&self) -> Vec<String> {
        self.tools.keys().cloned().collect()
    }

    /// Get all tools as OpenAI function definitions
    pub fn to_openai_tools(&self) -> Vec<serde_json::Value> {
        self.tools
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
        self.tools
            .values()
            .map(|tool| ToolDef {
                name: tool.name().to_string(),
                description: tool.description().to_string(),
                parameters: tool.input_schema(),
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

    #[test]
    fn test_native_tools_registered() {
        let mut registry = ToolRegistry::new();
        native::register_native_tools(&mut registry);

        // Check that all native tools are registered
        assert!(registry.has("shell"));
        assert!(registry.has("read_file"));
        assert!(registry.has("write_file"));
        assert!(registry.has("list_dir"));
        assert!(registry.has("edit_file"));
    }

    #[test]
    fn test_get_native_tools() {
        let mut registry = ToolRegistry::new();
        native::register_native_tools(&mut registry);

        // Test get() for native tools
        let shell = registry.get("shell");
        assert!(shell.is_some());
        assert_eq!(shell.unwrap().name(), "shell");

        let read_file = registry.get("read_file");
        assert!(read_file.is_some());
        assert_eq!(read_file.unwrap().name(), "read_file");
    }

    #[test]
    fn test_to_tool_defs_includes_native() {
        let mut registry = ToolRegistry::new();
        native::register_native_tools(&mut registry);

        let tool_defs = registry.to_tool_defs();
        let names: Vec<String> = tool_defs.iter().map(|t| t.name.clone()).collect();

        assert!(names.contains(&"shell".to_string()));
        assert!(names.contains(&"read_file".to_string()));
        assert!(names.contains(&"write_file".to_string()));
        assert!(names.contains(&"list_dir".to_string()));
        assert!(names.contains(&"edit_file".to_string()));
    }
}
