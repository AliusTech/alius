#[cfg(test)]
mod tests {
    use crate::{
        ClientCapabilities, Content, McpTool, ServerCapabilities, ServerInfo, ToolsCapability,
    };

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

    #[test]
    fn test_mcp_tool_with_no_description() {
        let json = r#"{
            "name": "minimal_tool",
            "inputSchema": {"type": "object"}
        }"#;

        let tool: McpTool = serde_json::from_str(json).unwrap();
        assert_eq!(tool.name, "minimal_tool");
        assert_eq!(tool.description, None);
    }

    #[test]
    fn test_server_info_serialization() {
        let info = ServerInfo {
            name: "test-server".to_string(),
            version: "1.0.0".to_string(),
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("test-server"));
        assert!(json.contains("1.0.0"));
    }

    #[test]
    fn test_server_capabilities_serialization() {
        let caps = ServerCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(false),
            }),
            ..Default::default()
        };
        let json = serde_json::to_string(&caps).unwrap();
        assert!(json.contains("tools"));
    }

    #[test]
    fn test_client_capabilities_default() {
        let caps = ClientCapabilities::default();
        assert!(caps.tools.is_none());
        assert!(caps.resources.is_none());
        assert!(caps.prompts.is_none());
        assert!(caps.sampling.is_none());
    }

    #[test]
    fn test_mcp_version_constant() {
        assert_eq!(crate::MCP_VERSION, "2024-11-05");
    }

    #[test]
    fn test_content_image_type() {
        let img = Content::Image {
            data: "base64data".to_string(),
            mime_type: "image/png".to_string(),
        };
        let json = serde_json::to_string(&img).unwrap();
        assert!(json.contains(r#""type":"image""#));
        assert!(json.contains("base64data"));
        assert!(json.contains("image/png"));
    }
}
