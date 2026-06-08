# Product Layer Interface

更新时间: 2026-06-05 03:43

## 职责

Product Layer 负责用户体验、输入输出、产品状态渲染和本产品的操作流程。它不负责 Core Runtime 内部业务逻辑。

输入:

- 用户输入。
- 外部应用请求。
- IDE/desktop/embedded 宿主事件。

输出:

- 产品层展示。
- 传给协议接口层的请求、命令或订阅。

## 产品层必须提供的信息

| 字段 | 说明 |
| --- | --- |
| `origin` | 请求来源，例如 LocalTui、IDE、Desktop、EmbeddedSdk、RemoteA2A |
| `workspace_root` | 当前工作区根目录，CLI/IDE/Desktop 必须提供或可解析 |
| `session_ref` | 目标 session，可为空，由 Session Manager 创建 |
| `input_mode` | 文本、JSON、文件选区、A2A task 等 |
| `requested_capabilities` | 本次请求需要的能力 |
| `client_trace_id` | 产品侧可选 trace id |

## 禁止事项

- 不直接调用 Loop Engine。
- 不直接调用 Provider Manager。
- 不直接写三层 memory 存储。
- 不直接执行 shell 或文件写入。
- 不把 Desktop、IDE、A2A 的权限提升为 LocalTui 权限。

## 验收标准

- 每个产品入口都有明确 origin。
- 每个产品入口都能构造协议层需要的输入信息。
- 产品层错误能映射为 ProtocolError 或 CoreEvent。
