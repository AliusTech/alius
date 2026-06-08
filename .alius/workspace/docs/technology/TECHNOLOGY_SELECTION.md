# Technology Selection By Product

更新时间: 2026-06-04 22:10

## 文档定位

本文按产品形态记录技术选型。实现时以本文件和对应产品文档为依据；如果选型变化，必须同步更新产品文档、接口文档和模块设计。

## 总览

| 产品 | 主语言 / 框架 | 接口 | 构建目标 | 状态 |
| --- | --- | --- | --- | --- |
| Alius CLI | Rust, Clap, Tokio, Ratatui | Direct Rust API | `cli-full/default` | 当前主产品 |
| 嵌入式第三方 SDK | Rust Core + C ABI | C ABI FFI Adapter | `embedded-sdk`, staticlib/cdylib | 规划实现目标 |
| Desktop | Electron 或待确认 Tauri 候选 | JSON-RPC Adapter | `desktop-planned` | 只规划接口 |
| IDE 插件 | VS Code / JetBrains 插件技术栈 | Plugin RPC Adapter | 插件端独立构建 | 规划产品 |
| 第三方 Agent 应用 | HTTP+JSON A2A | A2A Protocol Adapter | runtime switch | 协议入口 |

## CLI 技术选型

| 领域 | 选型 | 原因 |
| --- | --- | --- |
| 语言 | Rust | 与现有仓库一致，适合 CLI、并发、跨平台和安全边界 |
| 参数解析 | Clap | 已在 CLI 使用 |
| 异步运行时 | Tokio | 支持模型流、MCP、长任务和并发 IO |
| TUI | Ratatui | 已用于全屏终端工作区 |
| 配置格式 | TOML | 除 MCP `mcp.json` 外统一使用 TOML |
| MCP 配置 | JSON | MCP 标准默认格式 |
| 图表文档 | Markdown Mermaid | AI 可解析、diff 友好 |
| 日志 | tracing + rolling file appender | 支持结构化日志、实时写入、span 和 level |

## Embedded SDK 技术选型

| 领域 | 选型 | 原因 |
| --- | --- | --- |
| Core 实现 | Rust 裁剪 Core Lite | 复用 Core 模块并控制 feature |
| 外部接口 | C ABI FFI | C/C++/嵌入式宿主通用 |
| 产物 | staticlib / cdylib | 适配不同宿主链接方式 |
| 模型调用 | Remote model API | 设备侧资源受限 |
| 记忆 | light cache 或 memory gateway | 避免本地 heavy vector store |
| 禁用项 | shell、local tools、LanceDB、local embedding、plugin runtime | 降低资源占用和安全风险 |

## Desktop 技术选型

Desktop 只做规划，不在当前工程交付产品本体。

| 候选 | 状态 | 说明 |
| --- | --- | --- |
| Electron | 当前架构图记录的规划方向 | 通过 JSON-RPC Adapter 与 Core 通信 |
| Tauri | 待确认候选 | 用户提到的 `Tears` 暂记录为待确认项，确认后再替换术语 |

Desktop 相关接口只需要在本工程中保留 JSON-RPC 和 A2A 可选接入设计。

## IDE 插件技术选型

| 领域 | 选型 | 原因 |
| --- | --- | --- |
| VS Code | TypeScript Extension Host | 编辑器生态标准 |
| JetBrains | Kotlin/Java Plugin | JetBrains 生态标准 |
| 通信 | JSON-RPC / LSP-like, stdio 或 socket | 与编辑器和语言工具链兼容 |
| Core 接入 | Plugin RPC Adapter | 不直接绑定 Core 内部模块 |

## A2A 技术选型

| 领域 | 选型 | 原因 |
| --- | --- | --- |
| 协议 | A2A v1.0 Agent Card 兼容结构 | 支持外部 Agent 发现和协作 |
| 配置源 | `.alius/config/soul.toml` | 项目内可维护，发布时导出 JSON |
| 传输 | HTTP+JSON | 与 Agent Card supportedInterfaces 对齐 |
| 权限 | origin + capability_scope + Security Policy | RemoteA2A 默认最小权限 |

## 选型更新规则

- 新增产品必须先补产品文档，再补技术选型。
- 改动接口选型必须同步 `docs/interfaces/`。
- 改动构建 feature 必须同步 `overview/BUILD_FEATURES.md`。
- Desktop 相关技术不得写成当前仓库已实现能力。
