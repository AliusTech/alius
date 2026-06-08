# Protocol Interface Layer

更新时间: 2026-06-05 20:00

## 模块职责

协议接口层是 Product Layer 和 Core Runtime 之间的唯一工程边界。

输入:

- Local Rust 调用。
- JSON-RPC request。
- IDE RPC / LSP-like request。
- C ABI FFI request。
- A2A protocol message。

输出:

- `ProtocolEnvelope<CoreRequest>`
- `ProtocolEnvelope<CoreCommand>`
- `ProtocolEnvelope<CoreEvent>`
- `ProtocolError`
- log metadata

统一执行请求:

- Chat Mode 和 Plan Mode 都使用 `CoreRequest::RunLoop`。
- Chat Mode 使用 `RuntimeMode::Chat + LoopPolicy::chat()`。
- Plan Mode 使用 `RuntimeMode::Plan + LoopPolicy::plan()`。
- `StartTurn` 只作为兼容入口保留，不作为新产品入口首选。

## 代码位置

| 组件 | 位置 | 状态 |
| --- | --- | --- |
| 协议契约类型 | `protocol/src/core.rs` | 已落地 |
| Direct Rust API 网关 | `protocol/src/interface.rs` | 已落地（12 个非 start 委托方法） |
| Core Runtime 实现 | `runtime/core/src/runtime.rs` | 已落地（20 方法全部实现） |
| JSON-RPC Adapter | `entrypoints/jsonrpc/src/lib.rs` | 最小序列化适配已建立 |
| C ABI FFI Adapter | 未实现 | 后续 |
| A2A Protocol Adapter | 未实现 | 后续 |

## 接口定义

```text
start(request: ProtocolEnvelope<CoreRequest>) -> Result<RunRef>
```

当 `request.payload.kind = RunLoop` 时，协议层只校验 envelope 和 capability，不解释 loop 业务逻辑。

```text
send(command: ProtocolEnvelope<CoreCommand>) -> Result<void>
```

```text
subscribe(run_ref: RunRef) -> Stream<ProtocolEnvelope<CoreEvent>>
```

```text
cancel(run_ref: RunRef) -> Result<void>
```

## 内部逻辑

```text
receive transport message
-> validate protocol version
-> normalize origin
-> apply capability ceiling
-> build trace context
-> attach log context
-> call Core Gateway
-> stream CoreEvent back to product
```

当前最小实现:

```text
ProtocolInterface<CoreRuntime>
-> start(ProtocolEnvelope<CoreRequest>)
-> CoreRuntimeApi::start
-> subscribe(run_ref)
-> Vec<ProtocolEnvelope<CoreEvent>>
```

当前最小版本覆盖 Direct Rust API:

- 校验 `protocol_version`。
- 校验 `CoreRequest` payload。
- 对 `RemoteA2A` 和 `EmbeddedSdk` 执行 capability ceiling。
- 保存 run 的 origin、capability、workspace、session、trace context。
- 调用 `CoreRuntimeApi::start`、`send`、`subscribe`。
- 将 `CoreEvent` 包装回 `ProtocolEnvelope<CoreEvent>`。
- 提供 `cancel(run_ref, reason)` 便利入口。
- 12 个非 start 委托方法（config_read/update、model_list、health_check、close_session、clear_conversation、memory_save/list/clear、tool_list、review_start、query_logs），均含 `require_capability` 检查。

不在当前最小版本内:

- JSON-RPC transport。
- IDE RPC / LSP-like transport。
- C ABI FFI transport。
- A2A task/message 映射。
- 真实 Agent Loop 接入；当前仍取决于 Core Runtime 实现状态。

## 数据存储

协议层自身不拥有业务状态。它只写 trace metadata，业务状态由 Core Runtime 和 Storage Manager 管理。

## 异常处理

- unsupported protocol version: 返回 `UnsupportedVersion`。
- malformed envelope: 返回 `InvalidMessage`。
- capability 超出上限: 返回 `CapabilityDenied`。
- run 不存在: 返回 `RunNotFound`。
- 日志上下文构造失败: 返回 `InvalidMessage` 或降级为无 client trace。

## 与其他模块的关系

- 上游: CLI / TUI / Desktop / IDE / Embedded / Third-party Agent。
- 下游: Core Public API。
- 配置来源: Config Manager。
- 日志输出: Logging Manager 通过 Core Runtime 记录协议错误和请求边界。

## 验收标准

- CLI / TUI 只能通过 Local Rust Interface 进入 Core。
- 所有传输共享同一套 request、command、event、error 语义。
- 每条消息都有 origin、capability_scope、trace_id。
- 每条消息都能关联日志上下文。
- `protocol-interface` 单元测试覆盖 start/subscribe、协议版本拒绝、RemoteA2A shell capability 拒绝、command run_ref mismatch 和 cancel。
