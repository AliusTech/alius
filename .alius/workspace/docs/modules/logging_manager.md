# Logging Manager

更新时间: 2026-06-05 03:43

## 模块职责

Logging Manager 负责实时记录 Alius 运行日志、异常、错误日志和审计日志，并将日志与 workspace、session、run、trace 关联。

输入:

- `LogRecord`
- `CoreEvent`
- `TraceEvent`
- error/exception
- policy decision
- tool execution result

输出:

- persisted log record
- streamed log event
- query result

## 日志类型

| 类型 | 说明 | 示例 |
| --- | --- | --- |
| Runtime Log | 系统运行日志 | Core 启动、session 创建、provider 调用 |
| Error Log | 错误日志 | provider error、tool failure、config parse error |
| Exception Log | 异常日志 | panic、不可恢复异常、FFI 边界异常 |
| Audit Log | 审计日志 | approval、Shell Gate deny、权限变更 |
| Trace Log | 与 trace_id 关联的链路日志 | turn 内模型、工具、memory、budget 事件 |

## 接口定义

```text
emit(record: LogRecord) -> Result<LogRef>
```

参数:

- `record.level`: trace/debug/info/warn/error。
- `record.target`: 模块或子系统。
- `record.message`: 日志文本。
- `record.fields`: 结构化字段。
- `record.trace_id`: 可选 trace id。

返回:

- 持久化后的 `LogRef`。

异常:

- 日志目录不可写。
- 序列化失败。

```text
stream(query: LogStreamQuery) -> Stream<LogRecord>
```

返回:

- 实时日志流。

```text
query(query: LogQuery) -> Result<Vec<LogRecord>>
```

返回:

- 按 workspace、session、run、trace、level、time range 查询的日志。

```text
flush() -> Result<void>
```

说明:

- CLI/TUI 退出前必须 flush。

## 内部逻辑

```text
receive log record
-> attach timestamp
-> attach workspace/session/run/trace metadata
-> redact secrets
-> write append-only log
-> publish to subscribers
-> rotate if needed
```

## 数据存储

固定路径:

| 路径 | 说明 |
| --- | --- |
| `.alius/memory/logs/runtime.log.jsonl` | 运行日志 |
| `.alius/memory/logs/error.log.jsonl` | 错误和异常日志 |
| `.alius/memory/logs/audit.log.jsonl` | 审批、门禁和权限审计 |
| `.alius/memory/logs/trace/<trace-id>.jsonl` | 可选 trace 细分日志 |

约束:

- 日志路径固定，不在 `.alius/config/config.toml` 中指定。
- 项目配置不得覆盖日志根目录和日志文件名。

日志格式:

```text
LogRecord {
  timestamp: DateTime,
  level: LogLevel,
  target: String,
  message: String,
  workspace_ref: Option<WorkspaceRef>,
  session_ref: Option<SessionRef>,
  run_ref: Option<RunRef>,
  trace_id: Option<TraceId>,
  fields: Map<String, Value>
}
```

## 脱敏规则

- API key、token、secret、Authorization header 必须脱敏。
- 用户输入默认不完整写入日志，只记录摘要或 hash。
- Shell 命令可记录命令结构和风险判断；敏感参数需要脱敏。
- RemoteA2A payload 默认记录 envelope metadata，不记录完整业务内容。

## 实时性要求

- error、exception、audit 必须立即 flush。
- runtime log 可批量写，但不得在正常退出时丢失。
- TUI 应可订阅当前 run 的重要日志事件。

## 异常处理

- 日志文件损坏: 新建后续文件并记录 recover 事件。
- 日志目录不可写: 降级到 stderr，并发出 `LoggingUnavailable` CoreEvent。
- 序列化失败: 记录最小错误信息，不能导致 Core 主流程崩溃。

## 与其他模块的关系

- Core Runtime 启动时初始化 Logging Manager。
- Loop Engine、Provider Manager、Tool Executor、Shell Gate、Security Policy、Budget Manager 都必须写日志。
- Storage Manager 提供持久化和轮转底座。
- Session Manager 提供 session/run/trace metadata。

## 验收标准

- 系统运行、异常、错误、审批和 Shell Gate 拒绝都有日志记录。
- 日志可按 workspace、session、run、trace 查询。
- 敏感信息不会出现在日志明文中。
- CLI/TUI 退出前日志 flush。
