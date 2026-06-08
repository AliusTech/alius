# Episodic Memory

更新时间: 2026-06-04 22:10

## 模块职责

情景记忆保存具体发生过的 session、turn、事件、工具调用和用户决策。它记录“在某个时间和上下文中发生了什么”，不是无条件保存完整原始聊天记录。

## 记录边界

| 内容 | 默认策略 |
| --- | --- |
| session/turn/run 元数据 | 保存 |
| CoreEvent 时间线 | 保存 |
| 工具调用和结果摘要 | 保存 |
| 用户决策和审批 | 保存 |
| 用户输入原文 | 按隐私和配置策略保存引用、摘要或原文 |
| assistant 输出原文 | 按配置保存摘要或原文 |
| 敏感字段 | 脱敏或不保存 |

说明:

- 情景记忆可以记录用户输入，但不应被定义为“原始聊天日志数据库”。
- 如果用户输入包含密钥、隐私信息或超出项目范围的内容，应优先保存摘要或 hash。
- 可检索内容应经过 searchable 标记，不是所有 event 都进入检索索引。

## 接口定义

```text
append_event(event: CoreEvent) -> Result<EventRef>
```

```text
append_message(message: Message) -> Result<MessageRef>
```

```text
append_turn_summary(summary: TurnSummary) -> Result<SummaryRef>
```

```text
mark_searchable(event_ref: EventRef, policy: SearchablePolicy) -> Result<void>
```

```text
list_session_events(session_ref: SessionRef) -> Result<Vec<CoreEvent>>
```

## 内部逻辑

```text
receive CoreEvent
-> normalize session_ref / run_ref / trace_id
-> apply retention and privacy policy
-> persist event
-> update session timestamp
-> update retrieval index if event is searchable
```

## 数据存储

目标文件:

```text
.alius/memory/episodic/episodic.sqlite
```

核心表:

- `sessions`
- `turns`
- `messages`
- `core_events`
- `tool_calls`
- `decisions`

## 异常处理

- SQLite 锁冲突: 指数退避重试。
- event 序号重复: 幂等跳过。
- session 不存在: 创建或返回 `SessionNotFound`，由调用方策略决定。
- 输入包含敏感信息: 脱敏后保存或拒绝保存原文。
- retention policy 不允许原文保存: 只保存摘要和引用。

## 验收标准

- 每个 CoreEvent 可按 trace_id 查回。
- 每个 session 可重建消息和事件时间线。
- 用户输入原文保存策略可配置。
- 检索索引只包含被标记为 searchable 的内容。
