# Protocol Interface Contract

更新时间: 2026-06-05 20:00

## 职责

协议接口层统一所有产品入口的请求、命令、事件和错误语义。它只做传输适配、边界归一化、origin/capability 转换和协议校验，不执行业务逻辑，不保存业务状态。

## 代码来源

Phase 0 冻结的最小 Rust 契约位于:

```text
protocol/src/core.rs
```

Direct Rust API 的最小 Protocol Interface 实现位于:

```text
protocol/src/interface.rs
```

公共导出路径:

```rust
use protocol_interface::{
    ProtocolEnvelope, CoreRequest, CoreCommand, CoreEvent, ProtocolError,
    Origin, CapabilityScope, CoreRuntimeApi,
};
use protocol_interface::ProtocolInterface;
```

## 核心结构

### ProtocolEnvelope

```rust
pub struct ProtocolEnvelope<T> {
    pub protocol_version: String,
    pub origin: Origin,
    pub capability_scope: CapabilityScope,
    pub workspace_root: Option<PathBuf>,
    pub session_ref: Option<SessionRef>,
    pub run_ref: Option<RunRef>,
    pub trace_id: TraceId,
    pub payload: T,
}
```

约束:

- `protocol_version` 当前固定为 `1.0`。
- `origin` 表示产品或协议适配器来源。
- `capability_scope` 是该来源的能力上限，不是最终授权结果。
- `workspace_root` 是 workspace 边界，Shell Gate、Storage 和 Memory 都必须使用该边界。
- `trace_id` 必须贯穿 request、command、event、log。
- `payload` 只允许是 `CoreRequest`、`CoreCommand`、`CoreEvent` 或未来经过正式评审的协议载荷。

### Origin

| Origin | 来源 | 默认含义 |
| --- | --- | --- |
| `LocalCli` | CLI 非交互命令 | 本地用户命令行 |
| `LocalTui` | CLI 内 TUI workspace | 本地用户全屏工作区 |
| `EmbeddedSdk` | 嵌入式第三方 SDK | 裁剪能力，默认无 shell |
| `IdeExtension` | IDE 插件 | 规划接口，JSON-RPC / LSP-like |
| `Desktop` | Desktop 规划产品 | 规划接口，JSON-RPC |
| `RemoteA2A` | 远端 A2A Agent | 默认不得继承本地文件和 shell 权限 |
| `PluginRpc` | plugin RPC | plugin 入口 |
| `JsonRpc` | 通用 JSON-RPC | Desktop / 外部 RPC 规划 |
| `Test` | 测试 | 仅用于测试和 fixture |

### CapabilityScope

| 字段 | 类型 | 含义 |
| --- | --- | --- |
| `capabilities` | `Vec<Capability>` | 来源请求的能力上限 |
| `allow_external_workspace_paths` | `bool` | 是否允许 workspace 外路径访问，默认 false |
| `requires_human_approval` | `bool` | 高风险能力是否需要人类审批，默认 true |

当前内置默认:

| 构造函数 | 用途 | 默认能力 |
| --- | --- | --- |
| `CapabilityScope::local_cli()` | CLI 非交互命令 | workspace 读写、model、tools、shell、MCP、memory、config |
| `CapabilityScope::local_tui()` | TUI workspace | 同 local CLI |
| `CapabilityScope::embedded_sdk()` | 嵌入式 SDK | model、memory read、config read |
| `CapabilityScope::remote_a2a()` | 远端 A2A | model、memory read |

### CoreRequest

```rust
pub struct CoreRequest {
    pub request_id: RequestId,
    pub kind: CoreRequestKind,
    pub input: RequestInput,
    pub metadata: RequestMetadata,
}
```

| Kind | 输入要求 | 用途 |
| --- | --- | --- |
| `RunLoop` | 非空 `RequestInput::RunLoop` | 统一的 Chat / Plan loop 请求 |
| `StartTurn` | 非空 `RequestInput::Text` | 兼容入口，后续应收敛到 RunLoop |
| `OpenSession` | `RequestInput::None` | 创建或恢复 session |
| `InspectSession` | `RequestInput::None` | 查询 session snapshot |
| `ListSessions` | `RequestInput::None` | 列出 workspace 下 sessions |
| `ToolQuery` | 待 H2 细化 | 查询或声明工具能力 |
| `CloseSession` | `RequestInput::None` | 关闭 session |
| `ClearConversation` | `RequestInput::None` | 清空当前 session 对话 |
| `ConfigRead` | `RequestInput::None` | 读取当前运行时配置快照 |
| `ConfigValidate` | `RequestInput::None` | 校验配置完整性 |
| `ConfigUpdate` | `RequestInput::ConfigUpdate` | 更新配置项 |
| `ModelList` | `RequestInput::None` | 列出 provider 可用模型 |
| `MemorySave` | `RequestInput::MemoryContent` | 保存记忆条目 |
| `MemoryList` | `RequestInput::None` | 列出记忆条目 |
| `MemoryClear` | `RequestInput::None` | 清空记忆 |
| `ReviewStart` | `RequestInput::None` | 对上一次响应发起 review |
| `ReviewToggle` | `RequestInput::None` | 切换 auto-review |
| `ConfirmToggle` | `RequestInput::None` | 切换 auto-confirm |
| `HealthCheck` | `RequestInput::None` | 系统健康检查（doctor） |

### RunLoopInput

```rust
pub struct RunLoopInput {
    pub content: String,
    pub mode: RuntimeMode,
    pub policy: LoopPolicy,
}
```

```rust
pub enum RuntimeMode {
    Chat,
    Plan,
}
```

```rust
pub struct LoopPolicy {
    pub max_iterations: u32,
    pub tools_enabled: bool,
    pub planning_enabled: bool,
    pub require_convergence_check: bool,
    pub require_approval_for_tools: bool,
}
```

策略约定:

| 策略 | max_iterations | tools_enabled | planning_enabled | require_convergence_check | require_approval_for_tools |
| --- | --- | --- | --- | --- | --- |
| `LoopPolicy::chat()` | 1 | false | false | true | false |
| `LoopPolicy::plan()` | 20 | true | true | true | true |

### CoreCommand

```rust
pub struct CoreCommand {
    pub command_id: CommandId,
    pub kind: CoreCommandKind,
    pub target_run: RunRef,
    pub metadata: CommandMetadata,
}
```

| Kind | 用途 |
| --- | --- |
| `Cancel` | 取消运行中的 run |
| `Approve` | 审批某个 policy/tool/shell gate 请求 |
| `Deny` | 拒绝某个审批请求 |
| `Continue` | 继续等待输入或暂停后的 run |
| `Pause` | 暂停可恢复 run |
| `ApprovePlan` | 批准 Agent 提出的执行计划 |
| `RevisePlan` | 要求 Agent 修改执行计划 |
| `ExecuteSelected` | 只执行选中的 plan 节点 |
| `ApproveReview` | 批准 review 结果 |
| `RequestRevision` | 要求返工 |
| `SwitchModel` | 切换 LLM 模型 |
| `SwitchMode` | 切换 Plan/Bypass 执行模式 |

### CoreEvent

```rust
pub struct CoreEvent {
    pub event_id: EventId,
    pub trace_id: TraceId,
    pub session_ref: Option<SessionRef>,
    pub turn_ref: Option<TurnRef>,
    pub run_ref: RunRef,
    pub sequence: u64,
    pub kind: CoreEventKind,
    pub payload: CoreEventPayload,
    pub created_at: DateTime<Utc>,
}
```

事件类型:

| Kind | 说明 |
| --- | --- |
| `RunStarted` | run 已进入 Loop Engine |
| `LoopIterationStarted` | loop iteration 开始 |
| `SessionOpened` | session 已创建或恢复 |
| `TurnStarted` | turn 开始 |
| `ModelDelta` | 模型流式输出 |
| `ToolCallRequested` | 模型请求工具调用 |
| `ToolCallStarted` | 工具开始 |
| `ToolCallCompleted` | 工具完成 |
| `ConvergenceChecked` | loop iteration 收敛判断完成 |
| `NeedApproval` | 需要用户审批 |
| `NeedUserInput` | 需要用户补充输入 |
| `PolicyDecision` | 权限或审批结果 |
| `BudgetDecision` | 预算检查结果 |
| `MemoryRetrieved` | 记忆检索结果 |
| `MemoryWritten` | 记忆写入结果 |
| `LogRecordEmitted` | 运行日志记录事件 |
| `ErrorRaised` | 异常或错误 |
| `FinalResult` | 本次 run 最终结果 |
| `SessionClosed` | session 已关闭 |
| `ConversationCleared` | 对话已清空 |
| `ConfigChanged` | 配置变更通知 |
| `ModelListResult` | 模型列表查询结果 |
| `HealthCheckResult` | 健康检查结果 |
| `PlanProposed` | Agent 提出执行计划 |
| `PlanStepStarted` | Plan 步骤开始执行 |
| `PlanStepCompleted` | Plan 步骤执行完成 |
| `PlanCompleted` | Plan 全部完成 |
| `ReviewStarted` | Review 开始 |
| `ReviewDelta` | Review 内容流 |
| `ReviewCompleted` | Review 完成 |
| `MemoryListResult` | 记忆列表查询结果 |
| `MemoryCleared` | 记忆已清空 |
| `ToolListResult` | 工具列表查询结果 |
| `ToolConfirmationRequired` | 工具调用需用户确认 |

## 接口方法

`CoreRuntimeApi` trait 是 Core Public API 契约:

| 方法 | 输入 | 输出 | 异常 |
| --- | --- | --- | --- |
| `start(envelope)` | `ProtocolEnvelope<CoreRequest>` | `RunRef` | UnsupportedVersion, InvalidMessage, CapabilityDenied |
| `send(envelope)` | `ProtocolEnvelope<CoreCommand>` | void | RunNotFound, Conflict, CapabilityDenied |
| `subscribe(run_ref)` | `RunRef` | `EventStream` | RunNotFound |
| `inspect(session_ref)` | `SessionRef` | `SessionSnapshot` | SessionNotFound |
| `list_sessions(workspace_ref)` | `WorkspaceRef` | `Vec<SessionSummary>` | WorkspaceMismatch |
| `close_session(session_ref)` | `SessionRef` | void | SessionNotFound |
| `clear_conversation(session_ref)` | `SessionRef` | void | SessionNotFound |
| `config_read()` | - | `ConfigSnapshot` | Internal |
| `config_validate()` | - | `ValidationResult` | Internal |
| `config_update(key, value)` | `&str`, `serde_json::Value` | void | InvalidMessage, Internal |
| `model_list()` | - | `Vec<ModelInfo>` | Internal |
| `memory_save(text, tags)` | `&str`, `Vec<String>` | void | Internal |
| `memory_list()` | - | `Vec<MemoryEntry>` | Internal |
| `memory_clear()` | - | void | Internal |
| `tool_list()` | - | `Vec<ToolInfo>` | Internal |
| `review_start(session_ref)` | `SessionRef` | `RunRef` | SessionNotFound, Internal |
| `health_check()` | - | `HealthReport` | Internal |
| `query_logs(query)` | `LogQuery` | `Vec<LogRecord>` | Internal |

## 传输适配

| 适配器 | 输入 | 输出 | Phase 0 状态 |
| --- | --- | --- | --- |
| Direct Rust API | Rust struct call | `ProtocolEnvelope` | 最小网关已在 `protocol-interface` 落地，默认 CLI/TUI 尚未切换 |
| Plugin RPC Adapter | JSON-RPC / LSP-like | `ProtocolEnvelope` | 规划 |
| C ABI FFI Adapter | FFI-safe payload | FFI-safe event/result | 规划 |
| JSON-RPC Adapter | JSON-RPC | `ProtocolEnvelope` | 规划 |
| A2A Protocol Adapter | A2A task/message | A2A Adapter request | 规划 |

## Direct Rust API 最小实现

当前 `protocol-interface::ProtocolInterface<R>` 是 Direct Rust API 的最小实现，其中 `R` 必须满足 `CoreRuntimeApi`。

最小职责:

- `start(envelope)` 校验协议版本、请求 payload 和 origin capability ceiling，然后调用 `CoreRuntimeApi::start`。
- `send(envelope)` 校验协议版本、capability ceiling 和 command `run_ref` 一致性，然后调用 `CoreRuntimeApi::send`。
- `subscribe(run_ref)` 调用 `CoreRuntimeApi::subscribe`，并将每个 `CoreEvent` 包装为 `ProtocolEnvelope<CoreEvent>`。
- `cancel(run_ref, reason)` 基于已保存的 run context 构造 `ProtocolEnvelope<CoreCommand>`。

当前强制的 capability ceiling:

| Origin | 默认拒绝 |
| --- | --- |
| `RemoteA2A` | workspace 写入、本地工具、shell、MCP、memory 写入、config 写入、外部路径访问 |
| `EmbeddedSdk` | workspace 写入、本地工具、shell、MCP、config 写入、外部路径访问 |

当前限制:

- 不实现 JSON-RPC、A2A、FFI、IDE RPC 传输层。
- 不解决 Core Runtime stub final event 问题；真实 Agent Loop 仍属于 H1 Core Runtime 主链任务。
- 不接管 CLI/TUI 默认路径；CLI/TUI 接入是下一步产品层改造。

## 错误映射

| 错误 | 触发条件 | 对产品层输出 |
| --- | --- | --- |
| `UnsupportedVersion` | 协议版本不支持 | 立即返回错误 |
| `InvalidMessage` | envelope 或 payload 不合法 | 返回错误并记录日志 |
| `CapabilityDenied` | capability 超出 origin 上限或策略拒绝 | 返回 deny，可附审批提示 |
| `RunNotFound` | run_ref 不存在 | 返回错误 |
| `SessionNotFound` | session_ref 不存在 | 返回错误 |
| `WorkspaceMismatch` | 请求 session 不属于当前 workspace | 拒绝加载或执行 |
| `Conflict` | command 与 run 状态冲突 | 返回冲突错误 |
| `Internal` | 协议层内部错误 | 返回可诊断错误并记录 error log |

## 验收标准

- 所有产品入口共享同一套 `CoreRequest`、`CoreCommand`、`CoreEvent` 语义。
- 每条消息都有 `origin`、`capability_scope`、`trace_id`。
- `RunLoop` 和 `StartTurn` 空输入必须被拒绝。
- `CoreRequest`、`CoreCommand`、`CoreEvent` 必须可 serde 序列化。
- Protocol Interface Layer 不保存业务状态。
- Direct Rust API 最小网关必须能返回 `ProtocolEnvelope<CoreEvent>`。
- RemoteA2A 和 EmbeddedSdk 不得声明本地 shell 能力。
- Chat 和 Plan 都必须通过 `RunLoopInput + LoopPolicy` 表达，不再新增 DirectChat / AgentLoop 分叉协议。
- CLI/TUI 后续默认路径必须从 Direct Rust API 构造 `ProtocolEnvelope<CoreRequest>`。
