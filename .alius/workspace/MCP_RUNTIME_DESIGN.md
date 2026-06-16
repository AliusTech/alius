# MCP Runtime 集成 - 技术设计文档

**版本**: 1.0  
**日期**: 2026-06-16  
**状态**: 设计阶段  

---

## 📋 背景

### 当前状态
- ✅ MCP 协议层完整实现（636 行）
- ✅ 工具桥接适配器（140 行）
- ✅ CLI 命令完全可用
- ❌ Runtime 自动加载未实现

### 问题
`CoreRuntimeManager::new_with_context()` 是同步函数，无法直接调用异步的 MCP 初始化。

---

## 🎯 目标

1. 在 Runtime 启动时自动加载 MCP 配置
2. 后台连接 MCP 服务器（不阻塞启动）
3. 动态注册 MCP 工具到 ToolRegistry
4. 提供健康检查和状态查询

---

## 💡 方案设计

### 方案选择: 延迟初始化（推荐）

#### 架构图
```
Runtime 启动
    ↓
CoreRuntimeManager::new_with_context() [同步]
    ↓
ToolRegistry 创建（空的 MCP 工具）
    ↓
Runtime 构建完成
    ↓
[后台任务] init_mcp_background() [异步]
    ├─ 加载配置
    ├─ 连接服务器
    ├─ 列出工具
    └─ 动态注册到 ToolRegistry
    ↓
MCP 工具可用
```

#### 优势
- ✅ 不阻塞 Runtime 启动
- ✅ 兼容现有同步架构
- ✅ 失败不影响核心功能
- ✅ 易于实现和测试

#### 劣势
- ⚠️ 启动时 MCP 工具暂时不可用
- ⚠️ 需要工具注册支持动态添加

---

## 🔧 详细设计

### 1. 创建 MCP 管理器

```rust
// runtime/core/src/mcp_manager.rs

use runtime_mcp::{McpRegistry, ClientCapabilities, ToolsCapability};
use runtime_tools::ToolRegistry;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct McpManager {
    registry: Arc<RwLock<Option<Arc<McpRegistry>>>>,
    status: Arc<RwLock<McpStatus>>,
}

#[derive(Debug, Clone)]
pub enum McpStatus {
    NotStarted,
    Initializing,
    Ready { connected: usize, tools: usize },
    Failed(String),
}

impl McpManager {
    pub fn new() -> Self {
        Self {
            registry: Arc::new(RwLock::new(None)),
            status: Arc::new(RwLock::new(McpStatus::NotStarted)),
        }
    }

    /// 启动后台初始化
    pub fn start_background_init(&self, tool_registry: Arc<RwLock<ToolRegistry>>) {
        let registry_clone = self.registry.clone();
        let status_clone = self.status.clone();

        tokio::spawn(async move {
            // 更新状态为初始化中
            *status_clone.write().await = McpStatus::Initializing;

            match Self::init_mcp().await {
                Ok(mcp_registry) => {
                    let connected = mcp_registry.list_connected().await.len();
                    
                    // 注册工具
                    match Self::register_tools(&tool_registry, mcp_registry.clone()).await {
                        Ok(tool_count) => {
                            *registry_clone.write().await = Some(mcp_registry);
                            *status_clone.write().await = McpStatus::Ready {
                                connected,
                                tools: tool_count,
                            };
                            tracing::info!("MCP initialized: {} servers, {} tools", connected, tool_count);
                        }
                        Err(e) => {
                            *status_clone.write().await = McpStatus::Failed(format!("Tool registration failed: {}", e));
                            tracing::warn!("MCP tool registration failed: {}", e);
                        }
                    }
                }
                Err(e) => {
                    *status_clone.write().await = McpStatus::Failed(format!("Initialization failed: {}", e));
                    tracing::debug!("MCP initialization skipped: {}", e);
                }
            }
        });
    }

    /// 初始化 MCP（异步）
    async fn init_mcp() -> Result<Arc<McpRegistry>, String> {
        let mcp_config_path = dirs::home_dir()
            .ok_or("Cannot determine home directory")?
            .join(".alius/mcp/servers.toml");

        if !mcp_config_path.exists() {
            return Err("MCP config not found".into());
        }

        let mut registry = McpRegistry::new();
        registry
            .load_config(&mcp_config_path)
            .map_err(|e| format!("Failed to load config: {}", e))?;

        // 连接所有服务器
        registry
            .connect_all()
            .await
            .map_err(|e| format!("Failed to connect: {}", e))?;

        Ok(Arc::new(registry))
    }

    /// 注册工具（异步）
    async fn register_tools(
        tool_registry: &Arc<RwLock<ToolRegistry>>,
        mcp_registry: Arc<McpRegistry>,
    ) -> Result<usize, String> {
        let mut registry = tool_registry.write().await;
        
        runtime_tools::mcp_bridge::register_mcp_tools(&mut *registry, mcp_registry)
            .await
            .map_err(|e| format!("Registration failed: {}", e))
    }

    /// 获取状态
    pub async fn status(&self) -> McpStatus {
        self.status.read().await.clone()
    }

    /// 获取注册表
    pub async fn registry(&self) -> Option<Arc<McpRegistry>> {
        self.registry.read().await.clone()
    }
}
```

### 2. 集成到 CoreRuntimeManager

```rust
// runtime/core/src/manager.rs

pub struct CoreRuntimeManager {
    // ... 现有字段
    #[cfg(feature = "mcp")]
    mcp_manager: Option<Arc<McpManager>>,
}

impl CoreRuntimeManager {
    pub fn new_with_context(
        workspace_root: impl Into<PathBuf>,
        settings: Settings,
        context: RuntimeManagerContext,
    ) -> Result<Self, ProtocolError> {
        let workspace_root = workspace_root.into();
        let client = LlmClient::new(settings.llm.clone())
            .map_err(|e| ProtocolError::Internal(format!("model client: {e}")))?;

        let registry = ToolPackageResolver::new(workspace_root.clone()).build_registry_lossy();
        let registry_arc = Arc::new(RwLock::new(registry));

        // 创建 MCP 管理器
        #[cfg(feature = "mcp")]
        let mcp_manager = {
            let manager = Arc::new(McpManager::new());
            manager.start_background_init(registry_arc.clone());
            Some(manager)
        };

        let runtime = CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new(&workspace_root))
            .settings(settings)
            .client(client)
            .tool_registry_arc(registry_arc)
            .build()?;

        Ok(Self {
            interface: ProtocolInterface::new(runtime),
            workspace_root,
            context,
            #[cfg(feature = "mcp")]
            mcp_manager,
        })
    }

    /// 获取 MCP 状态
    #[cfg(feature = "mcp")]
    pub async fn mcp_status(&self) -> Option<McpStatus> {
        self.mcp_manager.as_ref().map(|m| m.status()).flatten()
    }
}
```

### 3. 使 ToolRegistry 支持动态添加

```rust
// runtime/tools/src/registry.rs

impl ToolRegistry {
    /// 动态注册工具（线程安全）
    pub fn register_dynamic(&mut self, tool: Arc<dyn AliusTool>) -> Result<()> {
        let name = tool.name().to_string();
        
        if self.tools.contains_key(&name) {
            // 允许覆盖（用于 MCP 工具热加载）
            tracing::debug!("Replacing existing tool: {}", name);
        }
        
        self.tools.insert(name.clone(), tool);
        Ok(())
    }

    /// 批量动态注册
    pub fn register_batch(&mut self, tools: Vec<Arc<dyn AliusTool>>) -> Result<usize> {
        let mut count = 0;
        for tool in tools {
            self.register_dynamic(tool)?;
            count += 1;
        }
        Ok(count)
    }
}
```

---

## 📝 实施步骤

### Phase 1: 基础框架（2-3 小时）
1. 创建 `mcp_manager.rs`
2. 实现 `McpManager` 结构
3. 实现后台初始化逻辑
4. 单元测试

### Phase 2: Runtime 集成（2-3 小时）
1. 更新 `CoreRuntimeManager`
2. 添加 MCP 管理器字段
3. 启动后台初始化
4. 集成测试

### Phase 3: 动态工具注册（1-2 小时）
1. 更新 `ToolRegistry`
2. 支持动态添加工具
3. 线程安全保证
4. 测试验证

### Phase 4: 测试和优化（2-3 小时）
1. 完整功能测试
2. 性能测试
3. 错误处理优化
4. 文档更新

**总预计时间**: 8-12 小时

---

## ✅ 验收标准

- [ ] Runtime 启动不阻塞（< 500ms）
- [ ] MCP 后台初始化成功
- [ ] 工具动态注册成功
- [ ] 所有测试通过
- [ ] 文档完整
- [ ] 性能达标

---

## 🚀 下一步

开始实施 Phase 1...
