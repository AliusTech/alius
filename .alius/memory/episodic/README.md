# Episodic Memory

情景记忆保存具体发生过的事件和会话上下文。

## 目标文件

```text
.alius/memory/episodic/episodic.sqlite
```

## 核心表

| 表 | 说明 |
| --- | --- |
| `sessions` | session 元数据 |
| `turns` | 每次用户 turn |
| `messages` | 对话消息 |
| `core_events` | CoreEvent trace |
| `tool_calls` | 工具调用请求和结果 |
| `decisions` | 用户审批、选择、回答 |

## 写入来源

- Protocol Interface Layer 的 `CoreRequest`。
- Agent Engine 的 `CoreEvent`。
- Tool Executor 的工具结果。
- TUI / CLI 的用户决策。
