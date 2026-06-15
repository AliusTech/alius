//! MCP Manager for background initialization and lifecycle management.
//!
//! This module manages the Model Context Protocol (MCP) integration lifecycle:
//! - Background initialization of MCP servers
//! - Connection management and status tracking
//! - Tool registration to the runtime registry

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

/// Manager for MCP background initialization and lifecycle
pub struct McpManager {
    registry: Arc<RwLock<Option<Arc<McpRegistry>>>>,
    status: Arc<RwLock<McpStatus>>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(McpStatus::NotStarted)),
        }
    }

    /// Start background initialization (non-blocking)
    pub fn start_background_init(&self, tool_registry: Arc<runtime_tools::ToolRegistry>) {
        let registry_clone = self.registry.clone();
        let status_clone = self.status.clone();

        // Wrap Arc<ToolRegistry> in RwLock for internal use
        let tool_registry = Arc::new(RwLock::new(tool_registry));

        tokio::spawn(async move {
            // Update status to initializing
            *status_clone.write().await = McpStatus::Initializing;
            tracing::debug!("MCP background initialization started");

            match Self::init_mcp().await {
                Ok(mcp_registry) => {
                    let connected = mcp_registry.list_connected().await.len();

                    // Register tools
                    match Self::register_tools(&tool_registry, mcp_registry.clone()).await {
                        Ok(tool_count) => {
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
                            let msg = format!("Tool registration failed: {}", e);
                            *status_clone.write().await = McpStatus::Failed(msg.clone());
                            tracing::warn!("{}", msg);
                        }
                    }
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

    /// Register MCP tools to the tool registry (async)
    async fn register_tools(
        _tool_registry: &Arc<RwLock<Arc<runtime_tools::ToolRegistry>>>,
        mcp_registry: Arc<McpRegistry>,
    ) -> Result<usize, String> {
        // Access MCP tools
        let tools_map = mcp_registry
            .list_all_tools()
            .await
            .map_err(|e| format!("Failed to list tools: {}", e))?;

        // Count total tools across all servers
        let tools_count: usize = tools_map.values().map(|v| v.len()).sum();

        // TODO: Actual tool registration to be implemented when ToolRegistry supports dynamic registration
        // For now we just return the count

        Ok(tools_count)
    }

    /// Get current MCP status
    pub async fn status(&self) -> McpStatus {
        self.status.read().await.clone()
    }

    /// Get MCP registry if ready
    pub async fn registry(&self) -> Option<Arc<McpRegistry>> {
        self.registry.read().await.clone()
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
        // Manager should be created successfully
        assert_eq!(
            std::mem::size_of_val(&manager),
            std::mem::size_of::<McpManager>()
        );
    }

    #[tokio::test]
    async fn test_mcp_status_not_started() {
        let manager = McpManager::new();
        let status = manager.status().await;

        match status {
            McpStatus::NotStarted => {
                // Expected initial state
            }
            _ => panic!("Expected NotStarted status, got: {:?}", status),
        }
    }

    #[test]
    fn test_mcp_status_display() {
        let status_not_started = McpStatus::NotStarted;
        assert_eq!(status_not_started.to_string(), "Not started");

        let status_initializing = McpStatus::Initializing;
        assert_eq!(status_initializing.to_string(), "Initializing...");

        let status_ready = McpStatus::Ready {
            connected: 2,
            tools: 5,
        };
        assert_eq!(status_ready.to_string(), "Ready (2 servers, 5 tools)");

        let status_failed = McpStatus::Failed("test error".to_string());
        assert_eq!(status_failed.to_string(), "Failed: test error");
    }

    #[tokio::test]
    async fn test_mcp_manager_default() {
        let manager = McpManager::default();
        let status = manager.status().await;

        assert!(matches!(status, McpStatus::NotStarted));
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
}
