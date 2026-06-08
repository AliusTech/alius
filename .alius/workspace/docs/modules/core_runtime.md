# Core Runtime

更新时间: 2026-06-05 20:00

## 模块职责

Core Runtime 是 Alius 的统一执行层。

主路径:

```text
Core Public API -> Session Manager -> Loop Engine
```

输入:

- 协议层归一化后的 `ProtocolEnvelope<CoreRequest>`。
- 执行中的 `ProtocolEnvelope<CoreCommand>`。

输出:

- `CoreEvent` stream。
- final `CoreResult`。
- runtime/error/audit log。

## 代码位置

| 组件 | 位置 |
| --- | --- |
| CoreRuntimeApi trait | `protocol/src/core.rs` |
| CoreRuntime 实现 | `runtime/core/src/runtime.rs` |
| SessionManager | `runtime/core/src/session.rs` |
| LoopEngine | `runtime/core/src/loop_engine/` |
| EventAdapter | `runtime/core/src/event_adapter.rs` |
| Config（embedded defaults） | `runtime/core/src/config.rs` + `runtime/core/config/defaults/` |

## 接口定义

`CoreRuntimeApi` trait 定义了 20 个方法，覆盖 turn 执行、session 管理、config、memory、tool、review、health 和 logging。完整接口定义见 `docs/interfaces/core_runtime_api.md`。

## 当前工程基线

当前代码状态见:

```text
docs/overview/ENGINEERING_BASELINE.md
```

阶段判断:

- `core-runtime` crate 已落地，实现 `CoreRuntimeApi` trait 的全部 20 个方法。
- `core-runtime` crate 已新增 `loop_engine/` 结构，Chat 和 Plan 都进入统一 Loop Engine。
- 20 个 CoreRuntimeApi 方法已全部实现；8 个非 start 方法通过协议层委托接入 MemoryStore、ToolRegistry、ConversationStore 子系统。
- EventAdapter 已改为 Core 内部投影事件 → CoreEvent 映射，避免 Core Runtime 反向依赖 CLI 模型模块。
- CLI /memory、/tools、/review、/session clear 和 Chat 模式已改走 ProtocolBridge → ProtocolInterface → CoreRuntime 路径。
- TUI workspace 和 `alius run` 仍通过 `ReplSession`、`LlmClient` 直接执行，后续需灰度接入 CoreEvent stream。
- H1 第二期需要将 `alius run` 与 TUI workspace 灰度接入 `start` / `subscribe`。

## 内部逻辑

```text
ProtocolEnvelope<CoreRequest>
-> validate protocol version
-> validate origin and capability scope
-> initialize logging context
-> Session Manager open turn
-> Loop Engine
-> Build Context
-> Prompt Builder
-> Memory retrieval
-> Model Step
-> Tool Decision / Tool Step
-> Observe Result
-> Convergence Check
-> Model Router / Provider Manager
-> optional Tool Executor
-> Shell Gate / Policy / Budget / Trace
-> Storage Manager
-> Logging Manager
-> CoreEvent stream
```

## 数据存储

- `.alius/memory/communications/`
- `.alius/memory/episodic/`
- `.alius/memory/semantic/`
- `.alius/memory/procedural/`
- `.alius/memory/logs/`
- `.alius/config/`

## 异常处理

- session 无效: `SessionNotFound`。
- provider 不可用: 触发 fallback 或返回 model error。
- tool denied: 输出 `PolicyDenied` event。
- budget 超限: 输出 `BudgetExceeded` event 并终止 run。
- logging 不可用: 输出 `LoggingUnavailable` event，并降级到 stderr。

## 与其他模块的关系

- 由 Protocol Interface Layer 调用。
- 内部调用 Session Manager、Loop Engine、Memory Manager、Tool Executor、Shell Gate、Logging Manager 等。

## 验收标准

- 默认 CLI / TUI 执行路径经过 Core Runtime。
- Chat Mode 和 Plan Mode 都通过 `CoreRequest::RunLoop` 进入 Loop Engine。
- 每轮 loop 都有 `ConvergenceChecked` 事件。
- TUI 消费 CoreEvent stream。
- 工具、权限、预算和 trace 出现在同一条主路径上。
- runtime、error、audit log 与 session/run/trace 可关联查询。
