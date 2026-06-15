//! MCP tool bridge for integrating MCP tools into Alius tool system.

use async_trait::async_trait;
use runtime_mcp::{Content, McpRegistry, McpTool, McpToolResult};
use serde_json::Value;
use std::sync::Arc;

use crate::{AliusTool, ToolContext, ToolResult};

/// Bridge that wraps an MCP tool as an Alius tool
pub struct McpToolBridge {
    registry: Arc<McpRegistry>,
    server_name: String,
    tool_name: String,
    tool_def: McpTool,
}

impl McpToolBridge {
    /// Create a new MCP tool bridge
    pub fn new(
        registry: Arc<McpRegistry>,
        server_name: String,
        tool_name: String,
        tool_def: McpTool,
    ) -> Self {
        Self {
            registry,
            server_name,
            tool_name,
            tool_def,
        }
    }
}

#[async_trait]
impl AliusTool for McpToolBridge {
    fn name(&self) -> &'static str {
        // This is a workaround - we leak the string to get 'static lifetime
        Box::leak(self.tool_name.clone().into_boxed_str())
    }

    fn description(&self) -> &'static str {
        // This is a workaround - we leak the string to get 'static lifetime
        self.tool_def
            .description
            .as_deref()
            .map(|s| Box::leak(s.to_string().into_boxed_str()) as &'static str)
            .unwrap_or("MCP tool (no description)")
    }

    fn input_schema(&self) -> Value {
        self.tool_def.input_schema.clone()
    }

    async fn execute(
        &self,
        args: Value,
        _ctx: ToolContext,
    ) -> Result<ToolResult, protocol_interface::AliusError> {
        tracing::debug!(
            "Calling MCP tool: server={}, tool={}, args={:?}",
            self.server_name,
            self.tool_name,
            args
        );

        let result = self
            .registry
            .call_tool(&self.server_name, &self.tool_name, args)
            .await
            .map_err(|e| protocol_interface::AliusError::Agent(format!("MCP tool error: {}", e)))?;

        // Convert MCP result to Alius ToolResult
        let output = convert_mcp_result(&result);
        let is_error = result.is_error.unwrap_or(false);

        Ok(ToolResult {
            success: !is_error,
            output,
            metadata: None,
        })
    }
}

/// Convert MCP tool result content to string
fn convert_mcp_result(result: &McpToolResult) -> String {
    result
        .content
        .iter()
        .map(|content| match content {
            Content::Text { text } => text.clone(),
            Content::Image { mime_type, .. } => format!("[Image: {}]", mime_type),
            Content::Resource { uri, mime_type } => {
                if let Some(mime) = mime_type {
                    format!("[Resource: {} ({})]", uri, mime)
                } else {
                    format!("[Resource: {}]", uri)
                }
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Register all MCP tools from connected servers into the tool registry
pub async fn register_mcp_tools(
    tool_registry: &mut crate::ToolRegistry,
    mcp_registry: Arc<McpRegistry>,
) -> anyhow::Result<usize> {
    let all_tools = mcp_registry.list_all_tools().await?;
    let mut count = 0;

    for (server_name, tools) in all_tools {
        for tool in tools {
            let bridge = McpToolBridge::new(
                mcp_registry.clone(),
                server_name.clone(),
                tool.name.clone(),
                tool,
            );

            // Register with qualified name: server_name.tool_name
            let qualified_name = format!("{}.{}", server_name, bridge.name());
            tool_registry.register_with_name(Arc::new(bridge), qualified_name);
            count += 1;
        }
    }

    tracing::info!("Registered {} MCP tools", count);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use runtime_mcp::{Content, McpToolResult};

    #[test]
    fn test_convert_text_content() {
        let result = McpToolResult {
            content: vec![Content::Text {
                text: "Hello, world!".to_string(),
            }],
            is_error: None,
        };

        let output = convert_mcp_result(&result);
        assert_eq!(output, "Hello, world!");
    }

    #[test]
    fn test_convert_mixed_content() {
        let result = McpToolResult {
            content: vec![
                Content::Text {
                    text: "Result:".to_string(),
                },
                Content::Image {
                    data: "base64data".to_string(),
                    mime_type: "image/png".to_string(),
                },
                Content::Resource {
                    uri: "file:///path/to/file".to_string(),
                    mime_type: Some("text/plain".to_string()),
                },
            ],
            is_error: Some(false),
        };

        let output = convert_mcp_result(&result);
        assert!(output.contains("Result:"));
        assert!(output.contains("[Image: image/png]"));
        assert!(output.contains("[Resource: file:///path/to/file (text/plain)]"));
    }
}
