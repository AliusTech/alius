# Session Manager

更新时间: 2026-06-05 20:00

Session Manager 管理 workspace 下的 Session、Turn、Context、Task State 和 RunRef。它不是多工程管理器，而是一个工程目录内多个开发轮次和长期任务的管理器。

## 代码位置

| 组件 | 位置 |
| --- | --- |
| SessionManager | `runtime/core/src/session.rs` |
| SessionStore（持久化） | `runtime/store/src/session.rs` |

输入:

- `CoreRequest`
- `CoreCommand`
- `WorkspaceRef`
- `OpenSessionRequest`
- Storage snapshot

输出:

- `SessionRef`
- `RunRef`
- `TurnState`
- `ContextSnapshot`

## 概念边界

| 概念 | 定义 |
| --- | --- |
| Workspace | 一个确定工程目录，一个 workspace 只对应一个工程 |
| Session | 一次开发轮次、一个功能开发过程、一次修复过程或一个长期任务 |
| Turn | Session 中一次用户输入和执行输出 |
| Run | 一次可取消、可审批、可查询的 Core 执行实例 |
| Trace | 一次 run 的诊断链路 |

Session Manager 用于:

- `alius` 进入 workspace 时打开默认 session。
- `/session new` 开始新的功能开发或长期任务。
- `/session list` 查看同一 workspace 下的历史开发轮次。
- `/session load <id>` 恢复某次开发。
- A2A task 映射到本地可追踪 session。
- 长任务中把多个 turn 关联到同一目标。

## 接口定义

```text
open_session(request: OpenSessionRequest) -> Result<SessionRef>
```

参数:

- `workspace_ref`: 当前工程目录引用。
- `session_name`: 可选名称，例如功能名或长期任务名。
- `purpose`: `FeatureDevelopment`、`BugFix`、`Review`、`LongRunningTask`、`A2ATask`。

返回:

- `SessionRef`。

```text
start_run_loop(request: RunLoopInput, session_ref: SessionRef, trace_id: Option<TraceId>) -> Result<RunRef>
```

参数:

- `session_ref`: 目标 session。
- `request`: 本次 `RunLoopInput`，包含用户输入、`RuntimeMode` 和 `LoopPolicy`。
- `trace_id`: 可选外部 trace id。

返回:

- `RunRef`。

```text
apply_command(run_ref: RunRef, command: CoreCommand) -> Result<void>
```

```text
snapshot(session_ref: SessionRef) -> Result<SessionSnapshot>
```

```text
close(session_ref: SessionRef) -> Result<void, ProtocolError>
```

关闭 session，标记状态为 Closed。

```text
all_run_refs() -> Vec<(RunRef, Option<SessionRef>)>
```

获取所有 run 引用及其所属 session。

```text
run_refs_for_session(session_ref: SessionRef) -> Result<Vec<RunRef>, ProtocolError>
```

获取指定 session 下所有 run 引用。

## 内部逻辑

```text
resolve session
-> validate workspace boundary
-> create turn
-> assign run_ref and trace_id
-> load agent card
-> load context snapshot
-> hand off to Loop Engine
-> persist state transitions
-> attach logging metadata
```

## 数据存储

| 路径 | 说明 |
| --- | --- |
| `.alius/memory/communications/sessions/<session-id>/session.json` | session 元数据 |
| `.alius/memory/communications/sessions/<session-id>/messages.jsonl` | 消息记录 |
| `.alius/memory/episodic/episodic.sqlite` | turn、event、decision |
| `.alius/memory/logs/` | 与 session/run/trace 关联的运行日志 |

## 异常处理

- session 文件损坏: 返回错误并建议恢复或新建 session。
- run_ref 不存在: 返回 `RunNotFound`。
- command 与 run 状态冲突: 返回 `Conflict`。
- workspace_ref 不匹配: 返回 `WorkspaceMismatch`。
- session 属于其他 workspace: 拒绝加载。

## 与其他模块的关系

- 上游: Core Public API。
- 下游: Loop Engine、Storage Manager、Memory Manager、Logging Manager。

## 验收标准

- 每次 turn 都有稳定 run_ref 和 trace_id。
- 取消、恢复、审批命令能定位到正确 run。
- 同一 workspace 下可存在多个 session。
- session 可表达功能开发、修复、review 和长期任务。
- session 不允许跨 workspace 加载。
