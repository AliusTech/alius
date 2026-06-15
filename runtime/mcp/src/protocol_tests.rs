#[cfg(test)]
mod tests {
    use crate::{ClientCapabilities, Content, McpTool, ToolsCapability};

    #[tokio::test]
    async fn test_mcp_protocol_structures() {
        // Test ClientCapabilities serialization
        let capabilities = ClientCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            ..Default::default()
        };

        let json = serde_json::to_string(&capabilities).unwrap();
        assert!(json.contains("tools"));
        assert!(json.contains("list_changed"));
    }

    #[test]
    fn test_mcp_tool_deserialization() {
        let json = r#"{
            "name": "test_tool",
            "description": "A test tool",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "param": {"type": "string"}
                }
            }
        }"#;

        let tool: McpTool = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "test_tool");
        assert_eq!(tool.description, Some("A test tool".to_string()));
    }

    #[test]
    fn test_content_types() {
        let text = Content::Text {
            text: "Hello".to_string(),
        };

        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains(r#""type":"text""#));
        assert!(json.contains("Hello"));
    }
}
