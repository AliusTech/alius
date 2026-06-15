//! MCP client implementation.

use crate::protocol::*;
use crate::transport::Transport;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;

/// MCP client for communicating with MCP servers
pub struct McpClient {
    name: String,
    version: String,
    transport: Arc<Mutex<Box<dyn Transport>>>,
    request_id: AtomicU64,
    server_info: Option<ServerInfo>,
    server_capabilities: Option<ServerCapabilities>,
}

impl McpClient {
    /// Create a new MCP client
    pub fn new(
        name: impl Into<String>,
        version: impl Into<String>,
        transport: Box<dyn Transport>,
    ) -> Self {
        Self {
            name: name.into(),
            version: version.into(),
            transport: Arc::new(Mutex::new(transport)),
            request_id: AtomicU64::new(1),
            server_info: None,
            server_capabilities: None,
        }
    }

    /// Initialize the MCP connection
    pub async fn initialize(&mut self, capabilities: ClientCapabilities) -> anyhow::Result<()> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "initialize",
            "params": {
                "protocolVersion": MCP_VERSION,
                "capabilities": capabilities,
                "clientInfo": {
                    "name": self.name,
                    "version": self.version,
                }
            },
            "id": id,
        });

        let mut transport = self.transport.lock().await;
        transport.send(request).await?;

        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Initialize failed: {}", error);
        }

        let result = response["result"].clone();
        self.server_info = serde_json::from_value(result["serverInfo"].clone()).ok();
        self.server_capabilities = serde_json::from_value(result["capabilities"].clone()).ok();

        // Send initialized notification
        let notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized",
        });
        transport.send(notification).await?;

        tracing::info!(
            "MCP client initialized: server={:?}",
            self.server_info.as_ref().map(|s| s.name.as_str())
        );

        Ok(())
    }

    /// List available tools
    pub async fn list_tools(&self) -> anyhow::Result<Vec<McpTool>> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/list",
            "id": id,
        });

        let mut transport = self.transport.lock().await;
        transport.send(request).await?;

        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("List tools failed: {}", error);
        }

        let tools: Vec<McpTool> = serde_json::from_value(response["result"]["tools"].clone())?;
        Ok(tools)
    }

    /// Call a tool
    pub async fn call_tool(
        &self,
        name: impl Into<String>,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "tools/call",
            "params": {
                "name": name.into(),
                "arguments": arguments,
            },
            "id": id,
        });

        let mut transport = self.transport.lock().await;
        transport.send(request).await?;

        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Tool call failed: {}", error);
        }

        let result: McpToolResult = serde_json::from_value(response["result"].clone())?;
        Ok(result)
    }

    /// List available resources
    pub async fn list_resources(&self) -> anyhow::Result<Vec<Resource>> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "resources/list",
            "id": id,
        });

        let mut transport = self.transport.lock().await;
        transport.send(request).await?;

        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("List resources failed: {}", error);
        }

        let resources: Vec<Resource> =
            serde_json::from_value(response["result"]["resources"].clone())?;
        Ok(resources)
    }

    /// Read a resource
    pub async fn read_resource(&self, uri: impl Into<String>) -> anyhow::Result<Vec<Content>> {
        let id = self.next_id();
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "resources/read",
            "params": {
                "uri": uri.into(),
            },
            "id": id,
        });

        let mut transport = self.transport.lock().await;
        transport.send(request).await?;

        let response = transport.receive().await?;
        if let Some(error) = response.get("error") {
            anyhow::bail!("Read resource failed: {}", error);
        }

        let contents: Vec<Content> =
            serde_json::from_value(response["result"]["contents"].clone())?;
        Ok(contents)
    }

    /// Get server info
    pub fn server_info(&self) -> Option<&ServerInfo> {
        self.server_info.as_ref()
    }

    /// Get server capabilities
    pub fn server_capabilities(&self) -> Option<&ServerCapabilities> {
        self.server_capabilities.as_ref()
    }

    fn next_id(&self) -> u64 {
        self.request_id.fetch_add(1, Ordering::SeqCst)
    }
}
