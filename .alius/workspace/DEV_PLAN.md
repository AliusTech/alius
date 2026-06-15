# Alius CLI 开发计划
> 制定日期: 2026-06-15
> 状态: 待审批

## 一、当前代码库状态

### 1.1 未提交变更
- **规模**: 58个文件，+6,638/-526行变更
- **范围**: 协议层、运行时核心、模型、工具、TUI工作区、配置、文档
- **风险**: 大量未提交代码存在冲突和回退风险

### 1.2 部分完成的功能

#### Stage B 工具确认流程
**已完成 (B1-B3, B6)**:
- ✅ 协议类型定义 (`CoreCommandKind::RespondToolConfirmation`, `CoreEventPayload::ToolConfirmation`)
- ✅ `tool_step.rs` 在高风险操作时暂停并发出 `ToolConfirmationRequired` 事件
- ✅ `SessionManager` 实现了 oneshot 确认通道管理

**待完成 (B4-B5)**:
- ❌ `CoreRuntimeManager` 缺少 `respond_confirmation` 公开方法
- ❌ TUI 未处理 `ToolConfirmationRequired` 事件
- ❌ TUI 未实现确认交互界面

#### MCP 协议集成
**已完成**:
- ✅ MCP v2024-11-05 协议客户端 (`runtime/mcp`)
- ✅ `McpManager` 后台初始化框架 (`runtime/core/mcp_manager.rs`)
- ✅ CLI 命令脚手架 (`alius mcp`)

**待完成**:
- ❌ MCP 工具未注册到默认工具注册表
- ❌ CLI/TUI 命令未连接到 `McpManager`
- ❌ 缺少 MCP 服务器配置管理

---

## 二、开发计划

### 阶段 0: 代码库稳定化 (优先级: P0 - 必须先完成)

**目标**: 保护已完成的工作，建立清晰的开发基线

**任务**:
1. **提交当前工作** (0.1)
   - 审查所有未提交变更
   - 创建有意义的提交消息，描述已完成的功能
   - 提交: "feat: 实现工具确认协议基础 (Stage B B1-B3+B6)"
   - 提交: "feat: 添加TUI对话块折叠和MCP协议实现"

**验收标准**:
- `git status` 显示干净的工作树
- 所有变更已提交并有清晰的提交历史

**预计工作量**: 30分钟

---

### 阶段 1: 完成 Stage B 工具确认流程 (优先级: P1)

**目标**: 完成工具确认的端到端实现，使高风险操作在 Plan 模式下需要用户批准

#### 任务 1.1: CoreRuntimeManager 桥接层 (B4)
**文件**: `runtime/core/src/manager.rs`

**实现**:
```rust
impl CoreRuntimeManager {
    /// 响应工具确认请求 (Stage B B4)
    pub fn respond_confirmation(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        approved: bool,
    ) -> Result<(), ProtocolError> {
        // 需要访问 session_manager
        // 选项 A: CoreRuntime 暴露 session_manager() 方法
        // 选项 B: CoreRuntimeManager 直接持有 Arc<SessionManager>
        self.runtime()
            .session_manager()
            .deliver_confirmation(run_ref, tool_call_id, approved)
    }
    
    // 在 cancel() 方法中添加
    pub fn cancel(&self, run_ref: &RunRef, reason: Option<String>) -> Result<(), ProtocolError> {
        // 取消待确认的工具调用
        self.runtime()
            .session_manager()
            .cancel_pending_confirmations(run_ref);
        
        self.interface.cancel(run_ref, reason)
    }
}
```

**前置条件**:
- `CoreRuntime` 需要暴露 `session_manager()` 方法

**验收标准**:
- `CoreRuntimeManager::respond_confirmation` 方法存在并编译通过
- `CoreRuntimeManager::cancel` 调用 `cancel_pending_confirmations`
- 单元测试验证确认传递流程

#### 任务 1.2: TUI 事件处理 (B5a)
**文件**: `entrypoints/cli/src/tui/workspace/mod.rs`, `events.rs`

**实现**:
1. 扩展 `WorkspaceAction` 枚举:
```rust
pub enum WorkspaceAction {
    // ... 现有变体
    RespondToolConfirmation { tool_call_id: String, approved: bool },
}
```

2. 在事件处理循环中处理 `ToolConfirmationRequired`:
```rust
CoreEventKind::ToolConfirmationRequired => {
    if let CoreEventPayload::ToolConfirmation { tool_call_id, tool_name, details } = &event.payload {
        state.show_tool_confirmation(tool_call_id, tool_name, details);
    }
}
```

**验收标准**:
- `ToolConfirmationRequired` 事件触发确认界面
- 事件日志显示工具名称和详情

#### 任务 1.3: TUI 确认界面 (B5b)
**文件**: `entrypoints/cli/src/tui/workspace/interaction.rs`

**实现**:
1. 添加确认状态到 `InteractionState`:
```rust
pub struct ToolConfirmationState {
    pub tool_call_id: String,
    pub tool_name: String,
    pub details: String,
}
```

2. 渲染确认界面 (类似 Plan 审批界面):
   - 显示工具名称、操作详情
   - 提供选项: `[Y] 允许` / `[N] 拒绝` / `[Esc] 取消`
   - 支持键盘快捷键

3. 调用 `CoreRuntimeManager::respond_confirmation`

**验收标准**:
- 确认界面在 `ToolConfirmationRequired` 时显示
- Y/N 键触发批准/拒绝
- 响应正确发送到运行时
- 界面在响应后关闭

#### 任务 1.4: 端到端测试
**测试场景**:
1. Plan 模式下执行高风险 shell 命令 (如 `rm -rf`)
2. 验证 TUI 显示确认对话框
3. 测试批准路径: 工具执行
4. 测试拒绝路径: 工具跳过，返回 "denied by user"
5. 测试取消路径: 运行取消，确认通道关闭

**预计工作量**: 4-6小时

---

### 阶段 2: MCP 工具集成 (优先级: P2)

**目标**: 将 MCP 协议客户端连接到运行时，使外部 MCP 工具可用

#### 任务 2.1: MCP 工具注册表桥接
**文件**: `runtime/core/src/mcp_manager.rs`, `runtime/tools/src/registry.rs`

**实现**:
1. 创建 MCP 工具适配器，将 MCP 工具转换为 `AliusTool` trait
2. 在 `McpManager::start_background_init` 中:
   - 加载 MCP 服务器配置
   - 启动 MCP 客户端
   - 将 MCP 工具注册到 `ToolRegistry`

**设计选项**:
- **选项 A**: MCP 工具作为动态工具注册 (推荐)
  - 无需 WASM 编译
  - 通过 `ToolRegistry::register_dynamic()` 注册
  
- **选项 B**: MCP 工具包装为 WASM 模块
  - 符合 "所有工具都是 WASM" 的架构原则
  - 但增加复杂性

**验收标准**:
- MCP 服务器启动时，其工具出现在 `alius tool list`
- MCP 工具可在 Plan 模式下调用
- MCP 工具执行成功返回结果

#### 任务 2.2: CLI 命令连接
**文件**: `entrypoints/cli/src/cli.rs`, `entrypoints/cli/src/main.rs`

**实现**:
1. 连接 `alius mcp list` 到 `McpManager`
2. 连接 `alius mcp add <server>` 到配置管理
3. 连接 `alius mcp remove <server>` 到配置管理
4. 显示 MCP 初始化状态

**验收标准**:
- `alius mcp list` 显示已配置的 MCP 服务器
- `alius mcp add` 添加新服务器并重启
- `alius mcp status` 显示初始化状态

**预计工作量**: 6-8小时

---

### 阶段 3: 文档和测试完善 (优先级: P3)

**任务**:
1. 更新 `implementation-gaps.md`
   - 标记 Stage B 为已完成
   - 更新 MCP 集成状态

2. 更新 TUI 文档
   - 记录工具确认界面交互
   - 添加 MCP 工具使用示例

3. 添加集成测试
   - 工具确认流程测试
   - MCP 工具执行测试

**预计工作量**: 2-3小时

---

## 三、风险和依赖

### 3.1 技术风险
- **R1**: `CoreRuntime` 可能不暴露 `session_manager`
  - **缓解**: 修改 `CoreRuntime` 添加访问器方法
  
- **R2**: MCP 工具与 WASM 架构不匹配
  - **缓解**: 使用动态工具注册机制

### 3.2 依赖关系
```
阶段 0 (代码提交)
  └─> 阶段 1 (工具确认)
       ├─> 任务 1.1 (Manager)
       │    └─> 任务 1.2 (TUI 事件)
       │         └─> 任务 1.3 (TUI UI)
       │              └─> 任务 1.4 (测试)
       └─> 阶段 2 (MCP 集成)
            └─> 阶段 3 (文档)
```

---

## 四、总体时间估算

| 阶段 | 任务 | 预计时间 |
|------|------|----------|
| 阶段 0 | 代码提交 | 0.5小时 |
| 阶段 1 | 工具确认完成 | 4-6小时 |
| 阶段 2 | MCP 集成 | 6-8小时 |
| 阶段 3 | 文档和测试 | 2-3小时 |
| **总计** | | **12.5-17.5小时** |

---

## 五、建议的执行顺序

### 立即执行 (本次会话):
1. **阶段 0**: 提交当前代码 (保护工作成果)
2. **阶段 1.1-1.3**: 完成工具确认流程 (高价值功能)

### 后续会话:
3. **阶段 1.4**: 端到端测试
4. **阶段 2**: MCP 集成 (独立功能)
5. **阶段 3**: 文档完善

---

## 六、验收标准

### 阶段 0 完成标准:
- ✅ 所有变更已提交
- ✅ Git 历史清晰

### 阶段 1 完成标准:
- ✅ Plan 模式下高风险工具操作触发确认界面
- ✅ 用户可批准/拒绝/取消工具执行
- ✅ 拒绝的工具不执行，返回明确错误

### 阶段 2 完成标准:
- ✅ MCP 服务器可配置和启动
- ✅ MCP 工具在工具列表中可见
- ✅ MCP 工具可在对话中调用

### 阶段 3 完成标准:
- ✅ 文档更新反映新功能
- ✅ 集成测试通过

---

## 七、用户决策点

### Q1: 阶段优先级
是否同意以下优先级排序？
1. 阶段 0 (必须) → 阶段 1 (高) → 阶段 2 (中)

### Q2: MCP 工具架构
MCP 工具注册方式选择:
- **选项 A**: 动态工具注册 (快速实现)
- **选项 B**: WASM 包装 (符合架构原则)

推荐: **选项 A**，因为 MCP 工具需要运行时通信，WASM 沙箱会增加复杂性。

### Q3: 本次会话范围
建议本次会话完成:
- ✅ 阶段 0 (代码提交)
- ✅ 阶段 1.1-1.3 (工具确认核心实现)

是否同意？或需要调整范围？
