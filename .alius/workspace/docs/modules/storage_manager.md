# Storage Manager

更新时间: 2026-06-05 03:43

## 模块职责

Storage Manager 负责配置、会话、缓存、Trace、日志和 Keychain 的统一存储访问。

输入:

- storage request
- session/run/trace metadata
- config snapshot
- credential access request

输出:

- persisted record
- storage snapshot
- keychain credential
- trace query result
- log query result

## 接口定义

```text
save_record(record: StorageRecord) -> Result<RecordRef>
```

```text
load_record(ref: RecordRef) -> Result<StorageRecord>
```

```text
append_trace(event: TraceEvent) -> Result<TraceRef>
```

```text
append_log(record: LogRecord) -> Result<LogRef>
```

```text
query_logs(query: LogQuery) -> Result<Vec<LogRecord>>
```

```text
read_secret(key: SecretKeyRef) -> Result<SecretValue>
```

异常:

- 存储文件损坏。
- 权限不足。
- secret 不存在。

## 内部逻辑

```text
receive storage operation
-> resolve storage namespace
-> apply path / permission guard
-> read or write backend
-> attach trace metadata
-> return result
```

## 数据存储

| 类型 | 路径 |
| --- | --- |
| 项目配置 | `.alius/config/` |
| 会话记录 | `.alius/memory/communications/sessions/` |
| cache | `.alius/memory/cache/` |
| trace | `.alius/memory/episodic/` 或专用 trace store |
| logs | `.alius/memory/logs/` |
| keychain | 系统 keychain 或用户级 secret store |

## 异常处理

- 写入失败必须返回可诊断错误，不能静默丢数据。
- cache 损坏可删除重建。
- keychain 不可用时允许环境变量 fallback，但必须记录来源。
- 日志写入失败时通知 Logging Manager 降级到 stderr。
- error/audit 日志需要 append-only 写入。

## 与其他模块的关系

- 被 Loop Engine、Session Manager、Provider Manager、Tool Executor 调用。
- 受 Security & Policy Manager 限制。
- 为 Memory Manager 提供持久化底座。
- 为 Logging Manager 提供日志持久化、轮转和查询底座。

## 验收标准

- 所有 trace 事件可按 run_ref 查询。
- 配置、会话、cache、secret 命名空间隔离。
- 存储错误不会被误报为模型或工具错误。
- 日志可按 workspace、session、run、trace、level 查询。
