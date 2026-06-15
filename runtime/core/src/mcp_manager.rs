//! MCP Manager for background initialization and lifecycle management.
//!
//! This module manages the Model Context Protocol (MCP) integration lifecycle:
//! - Background initialization of MCP servers
//! - Connection management and status tracking
//! - Tool registration to the runtime ToolRegistry
//!
//! MCP tools are registered directly into the shared `ToolRegistry` so they
//! appear in `tool_list`, `to_tool_defs`, and can be executed by LoopEngine.

use runtime_mcp::McpRegistry;
use std::sync::Arc;
use tokio::sync::RwLock;

/// MCP initialization status
#[derive(Debug, Clone)]
pub enum McpStatus {
    /// Not yet started
    NotStarted,
    /// Currently initializing
    Initializing,
    /// Ready with connected servers and registered tools
    Ready { connected: usize, tools: usize },
    /// Failed with error message
    Failed(String),
}

impl std::fmt::Display for McpStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            McpStatus::NotStarted => write!(f, "Not started"),
            McpStatus::Initializing => write!(f, "Initializing..."),
            McpStatus::Ready { connected, tools } => {
                write!(f, "Ready ({} servers, {} tools)", connected, tools)
            }
            McpStatus::Failed(err) => write!(f, "Failed: {}", err),
        }
    }
}

/// Manager for MCP background initialization and lifecycle.
///
/// MCP tools are registered directly into the shared `ToolRegistry`
/// (which uses interior `RwLock`), so they are visible through
/// `tool_list`, `to_tool_defs`, and executable by LoopEngine.
pub struct McpManager {
    mcp_registry: Arc<RwLock<Option<Arc<McpRegistry>>>>,
    status: Arc<RwLock<McpStatus>>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self {
            mcp_registry: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(McpStatus::NotStarted)),
        }
    }

    /// Start background initialization (non-blocking).
    /// MCP tools are registered directly into the shared `tool_registry`.
    /// Duplicate names are skipped — native/WASM tools take priority.
    pub fn start_background_init(&self, tool_registry: Arc<runtime_tools::ToolRegistry>) {
        let registry_clone = self.mcp_registry.clone();
        let status_clone = self.status.clone();

        tokio::spawn(async move {
            *status_clone.write().await = McpStatus::Initializing;
            tracing::debug!("MCP background initialization started");

            match Self::init_mcp().await {
                Ok(mcp_registry) => {
                    let connected = mcp_registry.list_connected().await.len();

                    let tool_count =
                        Self::register_tools(&tool_registry, mcp_registry.clone()).await;

                    *registry_clone.write().await = Some(mcp_registry);
                    *status_clone.write().await = McpStatus::Ready {
                        connected,
                        tools: tool_count,
                    };
                    tracing::info!(
                        "MCP initialized successfully: {} servers, {} tools",
                        connected,
                        tool_count
                    );
                }
                Err(e) => {
                    let msg = format!("Initialization failed: {}", e);
                    *status_clone.write().await = McpStatus::Failed(msg.clone());
                    tracing::debug!("MCP initialization skipped: {}", e);
                }
            }
        });
    }

    /// Initialize MCP registry (async)
    async fn init_mcp() -> Result<Arc<McpRegistry>, String> {
        let home_dir =
            dirs::home_dir().ok_or_else(|| "Cannot determine home directory".to_string())?;

        let mcp_config_path = home_dir.join(".alius/mcp/servers.toml");

        if !mcp_config_path.exists() {
            return Err(format!(
                "MCP config not found at {}. Create it to enable MCP servers.",
                mcp_config_path.display()
            ));
        }

        tracing::debug!("Loading MCP config from: {}", mcp_config_path.display());

        let mut registry = McpRegistry::new();
        registry
            .load_config(&mcp_config_path)
            .map_err(|e| format!("Failed to load MCP config: {}", e))?;

        // Connect to all enabled servers
        registry
            .connect_all()
            .await
            .map_err(|e| format!("Failed to connect to MCP servers: {}", e))?;

        tracing::info!("MCP registry initialized successfully");
        Ok(Arc::new(registry))
    }

    /// Register MCP tools directly into the ToolRegistry.
    /// Creates `McpToolAdapter` wrappers and calls `registry.register()`.
    /// Duplicate names are rejected by the registry (native/WASM take priority).
    async fn register_tools(
        tool_registry: &runtime_tools::ToolRegistry,
        mcp_registry: Arc<McpRegistry>,
    ) -> usize {
        let tools_map = match mcp_registry.list_all_tools().await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to list MCP tools: {e}");
                return 0;
            }
        };

        let mut registered = 0usize;

        for (server_name, tools) in &tools_map {
            let client = match mcp_registry.get_server(server_name).await {
                Some(c) => c,
                None => {
                    tracing::warn!("MCP server '{}' has no client, skipping tools", server_name);
                    continue;
                }
            };

            for tool in tools {
                let adapter =
                    runtime_tools::mcp_bridge::McpToolAdapter::from_mcp_tool(tool, client.clone());
                match tool_registry.register(adapter) {
                    Ok(()) => {
                        registered += 1;
                        tracing::debug!(
                            "Registered MCP tool '{}' from server '{}'",
                            tool.name,
                            server_name
                        );
                    }
                    Err(conflict) => {
                        // Duplicate name — native/WASM tool takes priority.
                        tracing::info!(
                            "MCP tool '{}' from '{}' skipped: {}",
                            tool.name,
                            server_name,
                            conflict
                        );
                    }
                }
            }
        }

        registered
    }

    /// Get current MCP status
    pub async fn status(&self) -> McpStatus {
        self.status.read().await.clone()
    }

    /// Get MCP registry if ready
    pub async fn registry(&self) -> Option<Arc<McpRegistry>> {
        self.mcp_registry.read().await.clone()
    }
}

impl Default for McpManager {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_manager_creation() {
        let manager = McpManager::new();
        assert_eq!(
            std::mem::size_of_val(&manager),
            std::mem::size_of::<McpManager>()
        );
    }

    #[tokio::test]
    async fn test_mcp_status_not_started() {
        let manager = McpManager::new();
        let status = manager.status().await;
        assert!(matches!(status, McpStatus::NotStarted));
    }

    #[test]
    fn test_mcp_status_display() {
        assert_eq!(McpStatus::NotStarted.to_string(), "Not started");
        assert_eq!(McpStatus::Initializing.to_string(), "Initializing...");
        assert_eq!(
            McpStatus::Ready {
                connected: 2,
                tools: 5
            }
            .to_string(),
            "Ready (2 servers, 5 tools)"
        );
        assert_eq!(
            McpStatus::Failed("test error".to_string()).to_string(),
            "Failed: test error"
        );
    }

    #[test]
    fn test_mcp_status_clone() {
        let status = McpStatus::Ready {
            connected: 2,
            tools: 5,
        };
        let cloned = status.clone();
        assert!(matches!(
            cloned,
            McpStatus::Ready {
                connected: 2,
                tools: 5
            }
        ));
    }

    #[tokio::test]
    async fn test_mcp_manager_default() {
        let manager = McpManager::default();
        let status = manager.status().await;
        assert!(matches!(status, McpStatus::NotStarted));
    }

    #[tokio::test]
    async fn test_mcp_no_config_no_panic() {
        let manager = McpManager::new();
        let registry = Arc::new(runtime_tools::ToolRegistry::new());
        runtime_tools::native::register_native_tools(&registry);

        manager.start_background_init(registry.clone());
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let status = manager.status().await;
        assert!(
            matches!(status, McpStatus::Failed(_)),
            "Expected Failed status when no MCP config, got {:?}",
            status
        );

        // Native tools must still be present.
        assert!(registry.has("shell"));
        assert!(registry.has("read_file"));
    }
}
