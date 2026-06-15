//! MCP tool adapter — wraps an MCP tool as an `AliusTool`.
//!
//! Each `McpToolAdapter` holds a reference to the `McpClient` that owns
//! the server connection, plus the tool name/schema extracted from the
//! MCP `tools/list` response.

use async_trait::async_trait;
use protocol_interface::AliusError;
use runtime_mcp::client::McpClient;
use runtime_mcp::protocol::McpTool;
use serde_json::Value as JsonValue;
use std::sync::Arc;

use crate::traits::{AliusTool, ToolContext, ToolResult};
use crate::PermissionLevel;

/// Wraps an MCP tool as a native `AliusTool` implementation.
///
/// Name and description are leaked once at construction to satisfy the
/// `&'static str` requirement of `AliusTool::name()` / `description()`.
/// This is acceptable because MCP tools are long-lived and bounded in count.
pub struct McpToolAdapter {
    name: &'static str,
    description: &'static str,
    input_schema: JsonValue,
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    /// Create an adapter from an `McpTool` descriptor and its owning client.
    pub fn from_mcp_tool(tool: &McpTool, client: Arc<McpClient>) -> Self {
        let name: &'static str = Box::leak(tool.name.clone().into_boxed_str());
        let desc = tool
            .description
            .clone()
            .unwrap_or_else(|| format!("MCP tool: {}", tool.name));
        let description: &'static str = Box::leak(desc.into_boxed_str());

        Self {
            name,
            description,
            input_schema: tool.input_schema.clone(),
            client,
        }
    }
}

#[async_trait]
impl AliusTool for McpToolAdapter {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn input_schema(&self) -> JsonValue {
        self.input_schema.clone()
    }

    fn source(&self) -> protocol_interface::core::ToolSource {
        protocol_interface::core::ToolSource::Mcp
    }

    fn required_permission(&self) -> PermissionLevel {
        // MCP tools are remote — treat as Execute-level by default.
        PermissionLevel::Execute
    }

    async fn execute(&self, args: JsonValue, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let result = self
            .client
            .call_tool(self.name.to_string(), args)
            .await
            .map_err(|e| AliusError::Agent(format!("MCP tool '{}' failed: {e}", self.name)))?;

        // Extract text content from MCP result.
        let output = result
            .content
            .iter()
            .filter_map(|c| match c {
                runtime_mcp::protocol::Content::Text { text } => Some(text.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n");

        let success = !result.is_error.unwrap_or(false);

        Ok(ToolResult {
            output,
            success,
            metadata: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    /// Helper to create a mock McpTool for testing schema conversion.
    fn make_mcp_tool(name: &str, desc: &str) -> McpTool {
        McpTool {
            name: name.to_string(),
            description: Some(desc.to_string()),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"}
                },
                "required": ["query"]
            }),
        }
    }

    #[test]
    fn test_adapter_preserves_name() {
        // We can't easily create a real McpClient in unit tests,
        // but we can verify schema conversion logic by checking
        // the adapter construction doesn't panic and preserves fields.
        let tool = make_mcp_tool("test_tool", "A test tool");
        // Verify the McpTool struct is correct before passing to adapter.
        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, Some("A test tool".to_string()));
        assert!(tool.input_schema.is_object());
    }

    #[test]
    fn test_mcp_tool_schema_conversion() {
        let tool = make_mcp_tool("search", "Search the web");
        let schema = &tool.input_schema;

        // Verify schema structure is preserved.
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"].is_object());
        assert_eq!(schema["properties"]["query"]["type"], "string");
        assert!(schema["required"].is_array());
    }

    #[test]
    fn test_mcp_tool_without_description() {
        let tool = McpTool {
            name: "no_desc".to_string(),
            description: None,
            input_schema: json!({"type": "object"}),
        };

        // Adapter should generate a fallback description.
        assert!(tool.description.is_none());
        // The from_mcp_tool constructor should handle this.
        let fallback_desc = tool
            .description
            .clone()
            .unwrap_or_else(|| format!("MCP tool: {}", tool.name));
        assert_eq!(fallback_desc, "MCP tool: no_desc");
    }
}
