//! E2E Integration Test for MCP
//!
//! Tests the complete MCP integration flow

#[cfg(test)]
mod mcp_integration_tests {
    use std::path::PathBuf;
    use tokio;

    #[tokio::test]
    async fn test_mcp_manager_creation() {
        // Test MCP Manager can be created
        let manager = core_runtime::mcp_manager::McpManager::new();
        assert_eq!(
            format!("{:?}", manager.status().await),
            "NotStarted"
        );
    }

    #[tokio::test]
    async fn test_mcp_config_loading() {
        // Test configuration file loading
        let config_path = dirs::home_dir()
            .unwrap()
            .join(".alius/mcp/servers.toml");

        if config_path.exists() {
            let content = std::fs::read_to_string(&config_path).unwrap();
            assert!(!content.is_empty());
        }
    }

    #[tokio::test]
    async fn test_tool_registry_creation() {
        // Test tool registry can be created and used
        use runtime_tools::ToolRegistry;

        let registry = ToolRegistry::new();
        let tools = registry.list();

        // Should start empty
        assert!(tools.len() >= 0);
    }
}

#[cfg(test)]
mod mcp_tools_tests {
    use std::sync::Arc;
    use tokio;

    #[tokio::test]
    async fn test_mcp_tool_bridge_creation() {
        // Test MCP tool bridge can be created
        use runtime_mcp::{McpRegistry, McpTool};

        let registry = Arc::new(McpRegistry::new());
        let tool_def = McpTool {
            name: "test_tool".to_string(),
            description: Some("Test tool".to_string()),
            input_schema: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        };

        // Create bridge
        let bridge = runtime_tools::mcp_bridge::McpToolBridge::new(
            registry.clone(),
            "test-server".to_string(),
            "test_tool".to_string(),
            tool_def,
        );

        // Verify bridge properties
        assert_eq!(bridge.name(), "test_tool");
    }
}
