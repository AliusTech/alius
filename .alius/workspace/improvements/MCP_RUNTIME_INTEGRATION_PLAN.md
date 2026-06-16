## 🎯 MCP Runtime 集成实施计划

### 目标
将 MCP 注册表集成到 Alius Runtime，使得在启动时自动加载和连接 MCP 服务器。

### 实施步骤

#### 步骤 1: 添加依赖
在 `runtime/core/Cargo.toml` 中添加：
```toml
runtime-mcp = { workspace = true, optional = true }

[features]
default = ["mcp"]
mcp = ["dep:runtime-mcp"]
```

#### 步骤 2: 修改 CoreRuntimeManager
在 `runtime/core/src/manager.rs` 中：

1. **添加 MCP 注册表字段**
```rust
pub struct CoreRuntimeManager {
    // ... 现有字段
    #[cfg(feature = "mcp")]
    mcp_registry: Option<Arc<runtime_mcp::McpRegistry>>,
}
```

2. **在初始化时加载 MCP**
```rust
pub async fn new(settings: Settings) -> Result<Self> {
    // ... 现有初始化代码
    
    // 初始化 MCP
    #[cfg(feature = "mcp")]
    let mcp_registry = Self::init_mcp_registry().await.ok();
    
    // ... 注册 MCP 工具到 ToolRegistry
    #[cfg(feature = "mcp")]
    if let Some(ref mcp_reg) = mcp_registry {
        runtime_tools::mcp_bridge::register_mcp_tools(&mut tool_registry, mcp_reg.clone())
            .await
            .ok();
    }
    
    Ok(Self {
        // ... 现有字段
        #[cfg(feature = "mcp")]
        mcp_registry,
    })
}
```

3. **添加 MCP 初始化辅助方法**
```rust
#[cfg(feature = "mcp")]
async fn init_mcp_registry() -> Result<Arc<runtime_mcp::McpRegistry>> {
    use runtime_mcp::{McpRegistry, ClientCapabilities, ToolsCapability};
    
    let mcp_config_path = dirs::home_dir()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine home directory"))?
        .join(".alius/mcp/servers.toml");
    
    if !mcp_config_path.exists() {
        tracing::info!("No MCP configuration found, skipping MCP initialization");
        return Err(anyhow::anyhow!("MCP config not found"));
    }
    
    let mut registry = McpRegistry::new();
    registry.load_config(&mcp_config_path)?;
    
    // 自动连接所有启用的服务器
    match registry.connect_all().await {
        Ok(_) => {
            let connected = registry.list_connected().await;
            tracing::info!("Connected to {} MCP server(s)", connected.len());
        }
        Err(e) => {
            tracing::warn!("Failed to connect to some MCP servers: {}", e);
        }
    }
    
    Ok(Arc::new(registry))
}
```

4. **添加访问器方法**
```rust
#[cfg(feature = "mcp")]
pub fn mcp_registry(&self) -> Option<Arc<runtime_mcp::McpRegistry>> {
    self.mcp_registry.clone()
}
```

#### 步骤 3: 在 TUI 中使用 MCP

在 `entrypoints/cli/src/tui/workspace/mod.rs` 中：

1. **添加 MCP 支持到 WorkspaceState**
```rust
pub struct WorkspaceState {
    // ... 现有字段
    #[cfg(feature = "mcp")]
    mcp_enabled: bool,
}
```

2. **在 `/tools` 命令中显示 MCP 工具**
```rust
async fn cmd_tools(&self) -> Result<String> {
    let mut output = String::new();
    
    // 显示内置工具
    output.push_str("Built-in Tools:\n");
    // ... 现有代码
    
    // 显示 MCP 工具
    #[cfg(feature = "mcp")]
    if let Some(mcp_registry) = self.runtime_manager.mcp_registry() {
        match mcp_registry.list_all_tools().await {
            Ok(all_tools) if !all_tools.is_empty() => {
                output.push_str("\n\nMCP Tools:\n");
                for (server, tools) in all_tools {
                    output.push_str(&format!("\n📦 {}\n", server));
                    for tool in tools {
                        let desc = tool.description.as_deref().unwrap_or("");
                        output.push_str(&format!("  🔧 {} - {}\n", tool.name, desc));
                    }
                }
            }
            _ => {}
        }
    }
    
    Ok(output)
}
```

### 优势

1. **自动化**: 启动时自动加载，用户无需手动操作
2. **可选**: 通过 feature flag 可选，不强制依赖
3. **容错**: 配置缺失或连接失败不影响主流程
4. **日志**: 详细的日志记录便于调试
5. **集成**: 无缝集成到现有工具系统

### 风险和缓解

**风险 1**: MCP 服务器启动慢影响 Alius 启动时间
**缓解**: 异步连接，超时机制，失败不阻塞

**风险 2**: MCP 配置错误导致启动失败
**缓解**: 配置验证，错误恢复，降级运行

**风险 3**: 内存占用增加
**缓解**: 按需连接，连接池管理

### 测试计划

1. **无配置场景**: Alius 正常启动，跳过 MCP
2. **有配置场景**: MCP 服务器自动连接
3. **配置错误场景**: 日志警告，但不影响启动
4. **工具调用**: `/tools` 命令显示 MCP 工具

---

**状态**: 设计完成，待实施  
**预计时间**: 4-6 小时  
**下一步**: 修改 runtime/core 代码
