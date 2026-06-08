# Product Documents

更新时间: 2026-06-04 22:10

## 目录定位

本目录按产品形态记录 Alius 的产品定位、使用方式、用户流程、接口边界和实现注意事项。产品文档用于连接“用户如何使用”和“工程如何实现”。

## 产品清单

| 产品 | 文档 | 当前状态 | 主接口 |
| --- | --- | --- | --- |
| Alius CLI（含 TUI） | `cli.md` | 当前主产品 | Direct Rust API |
| 嵌入式第三方 SDK | `embedded_sdk.md` | 当前附属产品 | C ABI FFI Adapter |
| Desktop 应用 | `desktop_planning.md` | ！关联产品，本工程不实现产品本体 | JSON-RPC Adapter |
| IDE 插件 | `ide_extension_planning.md` | ！关联产品 | Plugin RPC Adapter |
| 第三方 Agent 应用 | `third_party_agent_app.md` | 协议协作入口 | A2A Protocol Adapter |

## 每个产品文档必须包含

- 产品定位。
- 目标用户和市场定位。
- 使用方式。
- 操作设计流程。
- 用户设计流程。
- 接口边界。
- 配置和权限注意事项。
- 与 Core Runtime 的映射。
- 验收标准。

## 产品与 Workspace 的关系

`.alius/workspace/` 是一个工程的设计文档目录，不是产品目录。产品文档描述产品形态；模块文档描述实现模块；接口文档描述产品如何进入 Core Runtime。
