//! End-to-end integration tests for MCP protocol over stdio.
//!
//! These tests verify the complete MCP protocol flow:
//! - client sends `initialize`
//! - receives server capabilities
//! - sends `notifications/initialized`
//! - calls `tools/list`
//! - calls `tools/call`
//!
//! Uses a real Python MCP server fixture running as a child process.

use runtime_mcp::client::McpClient;
use runtime_mcp::protocol::{ClientCapabilities, Content, ToolsCapability};
use runtime_mcp::transport::StdioTransport;
use serde_json::json;
use std::path::PathBuf;

/// Path to the echo MCP server fixture.
fn echo_server_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.push("tests/fixtures/echo_mcp_server.py");
    path
}

/// Create a client connected to the echo MCP server.
async fn create_echo_client() -> anyhow::Result<McpClient> {
    let server_path = echo_server_path();
    assert!(
        server_path.exists(),
        "Echo MCP server fixture not found at: {}",
        server_path.display()
    );

    let transport = StdioTransport::new("python3", &[server_path.to_str().unwrap().to_string()])
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start MCP server: {e}"))?;

    let mut client = McpClient::new("test-client", "1.0.0", Box::new(transport));

    let capabilities = ClientCapabilities {
        tools: Some(ToolsCapability {
            list_changed: Some(true),
        }),
        ..Default::default()
    };

    client
        .initialize(capabilities)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to initialize MCP client: {e}"))?;

    Ok(client)
}

#[tokio::test]
async fn test_mcp_initialize_returns_server_info() {
    let client = create_echo_client().await.expect("Failed to create client");

    let server_info = client.server_info();
    assert!(server_info.is_some(), "Server info should be populated");
    let info = server_info.unwrap();
    assert_eq!(info.name, "echo-mcp-server");
    assert_eq!(info.version, "1.0.0");

    let capabilities = client.server_capabilities();
    assert!(
        capabilities.is_some(),
        "Server capabilities should be populated"
    );
}

#[tokio::test]
async fn test_mcp_list_tools_returns_echo_tool() {
    let client = create_echo_client().await.expect("Failed to create client");

    let tools = client.list_tools().await.expect("Failed to list tools");

    assert_eq!(tools.len(), 1, "Should have exactly one tool");

    let echo_tool = &tools[0];
    assert_eq!(echo_tool.name, "echo");
    assert!(echo_tool.description.is_some());
    assert_eq!(
        echo_tool.description.as_ref().unwrap(),
        "Echoes back the input message"
    );
    assert!(echo_tool.input_schema.is_object());

    // Verify schema structure
    let schema = &echo_tool.input_schema;
    assert_eq!(schema["type"], "object");
    assert!(schema["properties"].is_object());
    assert!(schema["properties"]["message"].is_object());
}

#[tokio::test]
async fn test_mcp_call_tool_echo() {
    let client = create_echo_client().await.expect("Failed to create client");

    let result = client
        .call_tool("echo", json!({"message": "hello world"}))
        .await
        .expect("Failed to call echo tool");

    assert!(
        !result.is_error.unwrap_or(true),
        "Tool call should not be an error"
    );
    assert!(!result.content.is_empty(), "Result should have content");

    // Extract text content
    let text = result
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    assert!(
        text.contains("Echo: hello world"),
        "Output should contain echoed message, got: {}",
        text
    );
}

#[tokio::test]
async fn test_mcp_call_tool_unknown_returns_error() {
    let client = create_echo_client().await.expect("Failed to create client");

    let result = client.call_tool("nonexistent_tool", json!({})).await;

    assert!(result.is_err(), "Unknown tool should return error");
}

#[tokio::test]
async fn test_mcp_protocol_sequence() {
    // Test that the protocol sequence works correctly:
    // 1. Initialize
    // 2. List tools (verify tool exists)
    // 3. Call tool (verify response)
    // All through the same client connection.
    let client = create_echo_client().await.expect("Failed to create client");

    // Step 1: Verify initialization
    let server_info = client.server_info().expect("Server info missing");
    assert_eq!(server_info.name, "echo-mcp-server");

    // Step 2: List tools
    let tools = client.list_tools().await.expect("Failed to list tools");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name, "echo");

    // Step 3: Call tool with different inputs
    let result1 = client
        .call_tool("echo", json!({"message": "first"}))
        .await
        .expect("Failed to call tool");
    let text1 = extract_text(&result1);
    assert!(
        text1.contains("Echo: first"),
        "First call should echo 'first'"
    );

    let result2 = client
        .call_tool("echo", json!({"message": "second"}))
        .await
        .expect("Failed to call tool");
    let text2 = extract_text(&result2);
    assert!(
        text2.contains("Echo: second"),
        "Second call should echo 'second'"
    );
}

#[tokio::test]
async fn test_mcp_tool_source_is_mcp() {
    // This test verifies that when McpToolAdapter wraps an MCP tool,
    // the source() method returns ToolSource::Mcp.
    use runtime_tools::AliusTool;

    let client = create_echo_client().await.expect("Failed to create client");
    let tools = client.list_tools().await.expect("Failed to list tools");
    let echo_tool = &tools[0];

    // Create an adapter
    let adapter = runtime_tools::mcp_bridge::McpToolAdapter::from_mcp_tool(
        echo_tool,
        std::sync::Arc::new(client),
    );

    // Verify source
    assert_eq!(
        adapter.source(),
        protocol_interface::core::ToolSource::Mcp,
        "MCP tool source should be ToolSource::Mcp"
    );
    assert_eq!(adapter.name(), "echo");
}

#[tokio::test]
async fn test_mcp_tool_adapter_execute() {
    // Test that McpToolAdapter::execute() works correctly
    use runtime_tools::AliusTool;

    let client = create_echo_client().await.expect("Failed to create client");
    let tools = client.list_tools().await.expect("Failed to list tools");
    let echo_tool = &tools[0];

    let adapter = runtime_tools::mcp_bridge::McpToolAdapter::from_mcp_tool(
        echo_tool,
        std::sync::Arc::new(client),
    );

    // Execute the tool through the adapter
    let ctx = runtime_tools::ToolContext::new(
        std::path::PathBuf::from("/tmp"),
        "test-session".to_string(),
        protocol_interface::RuntimeMode::Plan,
    );

    let result = adapter
        .execute(json!({"message": "test via adapter"}), ctx)
        .await
        .expect("Tool execution failed");

    assert!(result.success, "Tool execution should succeed");
    assert!(
        result.output.contains("Echo: test via adapter"),
        "Output should contain echoed message, got: {}",
        result.output
    );
}

fn extract_text(result: &runtime_mcp::protocol::McpToolResult) -> String {
    result
        .content
        .iter()
        .filter_map(|c| match c {
            Content::Text { text } => Some(text.as_str()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
