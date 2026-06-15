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

/// Manager for MCP background initialization and lifecycle.
///
/// MCP tools are stored separately from the native/WASM `ToolRegistry`
/// because `ToolRegistry::register` requires `&mut self` and the registry
/// is `Arc`-wrapped. MCP tools are accessible via `mcp_tools()` and are
/// merged into `tool_list` queries at the runtime level.
pub struct McpManager {
    mcp_registry: Arc<RwLock<Option<Arc<McpRegistry>>>>,
    status: Arc<RwLock<McpStatus>>,
    mcp_tools: Arc<RwLock<Vec<Arc<dyn runtime_tools::AliusTool>>>>,
}

impl McpManager {
    /// Create a new MCP manager
    pub fn new() -> Self {
        Self {
            mcp_registry: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(McpStatus::NotStarted)),
            mcp_tools: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Start background initialization (non-blocking).
    /// `existing_tool_names` is the list of native/WASM tool names already registered;
    /// MCP tools with duplicate names will be skipped.
    pub fn start_background_init(&self, existing_tool_names: Vec<String>) {
        let registry_clone = self.mcp_registry.clone();
        let status_clone = self.status.clone();
        let tools_clone = self.mcp_tools.clone();

        tokio::spawn(async move {
            *status_clone.write().await = McpStatus::Initializing;
            tracing::debug!("MCP background initialization started");

            match Self::init_mcp().await {
                Ok(mcp_registry) => {
                    let connected = mcp_registry.list_connected().await.len();

                    let (tool_count, mcp_tool_list) =
                        Self::register_tools(mcp_registry.clone(), &existing_tool_names).await;

                    *registry_clone.write().await = Some(mcp_registry);
                    *tools_clone.write().await = mcp_tool_list;
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

    /// Register MCP tools. Creates McpToolAdapter instances and stores them
    /// in a local registry. Returns the count of successfully registered tools.
    ///
    /// Native/WASM tools take priority — MCP tools with duplicate names are skipped.
    async fn register_tools(
        mcp_registry: Arc<McpRegistry>,
        existing_tool_names: &[String],
    ) -> (usize, Vec<Arc<dyn runtime_tools::AliusTool>>) {
        let tools_map = match mcp_registry.list_all_tools().await {
            Ok(m) => m,
            Err(e) => {
                tracing::warn!("Failed to list MCP tools: {e}");
                return (0, Vec::new());
            }
        };

        let mut registered = 0usize;
        let mut mcp_tools: Vec<Arc<dyn runtime_tools::AliusTool>> = Vec::new();

        for (server_name, tools) in &tools_map {
            let client = match mcp_registry.get_server(server_name).await {
                Some(c) => c,
                None => {
                    tracing::warn!("MCP server '{}' has no client, skipping tools", server_name);
                    continue;
                }
            };

            for tool in tools {
                // Skip if a native/WASM tool with the same name already exists.
                if existing_tool_names.iter().any(|n| n == &tool.name) {
                    tracing::info!(
                        "MCP tool '{}' from '{}' skipped: native/WASM tool takes priority",
                        tool.name,
                        server_name
                    );
                    continue;
                }

                let adapter =
                    runtime_tools::mcp_bridge::McpToolAdapter::from_mcp_tool(tool, client.clone());
                mcp_tools.push(Arc::new(adapter));
                registered += 1;
                tracing::debug!(
                    "Registered MCP tool '{}' from server '{}'",
                    tool.name,
                    server_name
                );
            }
        }

        (registered, mcp_tools)
    }

    /// Get current MCP status
    pub async fn status(&self) -> McpStatus {
        self.status.read().await.clone()
    }

    /// Get MCP registry if ready
    pub async fn registry(&self) -> Option<Arc<McpRegistry>> {
        self.mcp_registry.read().await.clone()
    }

    /// Get MCP tool list (for runtime tool_list queries).
    pub async fn mcp_tools(&self) -> Vec<Arc<dyn runtime_tools::AliusTool>> {
        self.mcp_tools.read().await.clone()
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

    #[tokio::test]
    async fn test_mcp_tools_empty_before_init() {
        let manager = McpManager::new();
        let tools = manager.mcp_tools().await;
        assert!(
            tools.is_empty(),
            "MCP tools should be empty before initialization"
        );
    }

    #[tokio::test]
    async fn test_mcp_no_config_no_panic() {
        // Start background init without MCP config — should not panic.
        let manager = McpManager::new();
        manager.start_background_init(vec!["shell".to_string(), "read_file".to_string()]);

        // Wait a moment for the background task.
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;

        let status = manager.status().await;
        // Should be Failed (no config) — not panicked.
        assert!(
            matches!(status, McpStatus::Failed(_)),
            "Expected Failed status when no MCP config, got {:?}",
            status
        );

        // MCP tools should still be empty.
        let tools = manager.mcp_tools().await;
        assert!(tools.is_empty());
    }
}
