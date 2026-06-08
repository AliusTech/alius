# Interface Documents

更新时间: 2026-06-05 03:43

## 目录定位

本目录记录每一层接口和产品到接口的映射。实现阶段必须先满足接口文档，再实现模块内部逻辑。

## 文件清单

| 文件 | 说明 |
| --- | --- |
| `product_interface_matrix.md` | 产品到协议接口、Core 能力和默认权限矩阵 |
| `product_layer.md` | 产品层接口责任和边界 |
| `protocol_interface_layer.md` | 协议接口层统一 envelope、request、command、event、error |
| `core_runtime_api.md` | Core Public API、Session、Run、Event stream 的实现依据 |

## Phase 0 冻结项

当前最小协议契约已在 `protocol/src/core.rs` 中落地。后续 Core Runtime、CLI/TUI、Embedded SDK、A2A、Desktop/IDE 规划接口都必须优先复用以下对象:

- `ProtocolEnvelope<T>`
- `CoreRequest`
- `CoreCommand`
- `CoreEvent`
- `ProtocolError`
- `CoreRuntimeApi`
- `WorkspaceRef`、`SessionRef`、`TurnRef`、`RunRef`、`TraceId`

## 接口原则

- Product Layer 不直接调用 Core Runtime 内部模块。
- Protocol Interface Layer 是唯一入口边界。
- Core Public API 是 Core Runtime 的唯一公开入口。
- 每条消息必须包含 origin、capability_scope、trace_id。
- 每个接口必须说明默认权限和越权审批规则。
