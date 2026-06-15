//! MCP server registry and management.

use crate::client::McpClient;
use crate::protocol::{ClientCapabilities, McpTool, McpToolResult, ToolsCapability};
use crate::transport::StdioTransport;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

/// MCP server configuration
#[derive(Debug, Clone, Deserialize)]
pub struct McpServerConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub disabled: bool,
}

/// Registry for managing multiple MCP servers
pub struct McpRegistry {
    servers: Arc<RwLock<HashMap<String, Arc<McpClient>>>>,
    configs: HashMap<String, McpServerConfig>,
}

impl McpRegistry {
    /// Create a new empty registry
    pub fn new() -> Self {
        Self {
            servers: Arc::new(RwLock::new(HashMap::new())),
            configs: HashMap::new(),
        }
    }

    /// Load server configurations from a TOML file
    pub fn load_config(&mut self, config_path: &Path) -> anyhow::Result<()> {
        let content = std::fs::read_to_string(config_path)?;

        #[derive(Deserialize)]
        struct ConfigFile {
            servers: HashMap<String, McpServerConfig>,
        }

        let config_file: ConfigFile = toml::from_str(&content)?;
        self.configs = config_file.servers;

        tracing::info!("Loaded {} MCP server configs", self.configs.len());
        Ok(())
    }

    /// Connect to a specific MCP server
    pub async fn connect_server(&self, name: &str) -> anyhow::Result<()> {
        let config = self
            .configs
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Server not found: {}", name))?;

        if config.disabled {
            anyhow::bail!("Server is disabled: {}", name);
        }

        tracing::info!("Connecting to MCP server: {}", name);

        let transport = StdioTransport::new(&config.command, &config.args).await?;
        let mut client = McpClient::new("alius", env!("CARGO_PKG_VERSION"), Box::new(transport));

        let capabilities = ClientCapabilities {
            tools: Some(ToolsCapability {
                list_changed: Some(true),
            }),
            ..Default::default()
        };

        client.initialize(capabilities).await?;

        self.servers
            .write()
            .await
            .insert(name.to_string(), Arc::new(client));

        Ok(())
    }

    /// Connect to all configured servers
    pub async fn connect_all(&self) -> anyhow::Result<()> {
        for (name, config) in &self.configs {
            if config.disabled {
                continue;
            }

            match self.connect_server(name).await {
                Ok(_) => tracing::info!("Connected MCP server: {}", name),
                Err(e) => tracing::warn!("Failed to connect MCP server {}: {}", name, e),
            }
        }
        Ok(())
    }

    /// Get all tools from all connected servers
    pub async fn list_all_tools(&self) -> anyhow::Result<HashMap<String, Vec<McpTool>>> {
        let servers = self.servers.read().await;
        let mut all_tools = HashMap::new();

        for (server_name, client) in servers.iter() {
            match client.list_tools().await {
                Ok(tools) => {
                    all_tools.insert(server_name.clone(), tools);
                }
                Err(e) => {
                    tracing::warn!("Failed to list tools from {}: {}", server_name, e);
                }
            }
        }

        Ok(all_tools)
    }

    /// Call a tool on a specific server
    pub async fn call_tool(
        &self,
        server: &str,
        tool: &str,
        arguments: serde_json::Value,
    ) -> anyhow::Result<McpToolResult> {
        let servers = self.servers.read().await;
        let client = servers
            .get(server)
            .ok_or_else(|| anyhow::anyhow!("Server not connected: {}", server))?;

        client.call_tool(tool, arguments).await
    }

    /// Get a connected server client
    pub async fn get_server(&self, name: &str) -> Option<Arc<McpClient>> {
        self.servers.read().await.get(name).cloned()
    }

    /// List all configured server names
    pub fn list_configs(&self) -> Vec<String> {
        self.configs.keys().cloned().collect()
    }

    /// List all connected server names
    pub async fn list_connected(&self) -> Vec<String> {
        self.servers.read().await.keys().cloned().collect()
    }
}

impl Default for McpRegistry {
    fn default() -> Self {
        Self::new()
    }
}
