//! Tool registry

use std::collections::HashMap;
use std::sync::Arc;

use crate::AliusTool;
use alius_protocol::ToolDef;

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

    /// Register a tool
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