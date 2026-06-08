# Product Interface Matrix

更新时间: 2026-06-04 22:10

## 产品接口矩阵

| 产品 | 主接口 | Core 入口 | 默认 Origin | 默认能力 | 实现状态 |
| --- | --- | --- | --- | --- | --- |
| Alius CLI（含 TUI） | Direct Rust API | Core Public API | LocalTui | workspace 内文件、受控 shell、模型、memory、workflow | 当前主产品 |
| IDE 插件 | Plugin RPC Adapter | Core Public API | IDE | workspace-scoped 文件、diagnostics、受限 workflow | 规划 |
| 嵌入式第三方 SDK | C ABI FFI Adapter | Embedded Core Lite 或 Core Public API 子集 | EmbeddedSdk | remote model、light memory cache、minimal config | 规划实现目标 |
| Desktop | JSON-RPC Adapter | Core Public API | Desktop | workspace UI、session、approval、logs、受限工具 | 规划接口 |
| 第三方 Agent 应用 | A2A Protocol Adapter | A2A Adapter -> Session Manager | RemoteA2A | task submit、streaming result、显式授权能力 | 协议入口 |

## 默认权限矩阵

| Origin | 文件读 | 文件写 | Shell | MCP | A2A | 备注 |
| --- | --- | --- | --- | --- | --- | --- |
| LocalTui | workspace 内允许 | workspace 内需按风险判断 | 受 Shell Gate 限制 | 按配置允许 | 可选启用 | 当前 CLI/TUI 主路径 |
| IDE | workspace 内允许 | workspace 内需用户确认 | 默认关闭或审批 | 按配置允许 | 默认关闭 | 遵守编辑器权限模型 |
| Desktop | workspace 内受限 | workspace 内需审批 | 默认审批 | 按配置允许 | 可选启用 | 当前只规划 |
| EmbeddedSdk | 不依赖本地 FS | 禁止 | 禁止 | 禁止 | 默认关闭 | Core Lite |
| RemoteA2A | 禁止 | 禁止 | 禁止 | 显式授权 | 允许协议通信 | 不继承本地权限 |

## 越权规则

- 读写作用范围超过 workspace 时，必须返回 approval required。
- destructive 操作默认 deny 或强制审批。
- RemoteA2A、EmbeddedSdk 默认不得请求本地 shell。
- capability_scope 是上限，Security Policy 才是最终授权。
