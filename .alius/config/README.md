# Alius Project Config

本目录保存项目级配置。目标结构服务 v10 架构中的 Product Layer、Protocol Interface Layer 和 Core Runtime。

## 文件说明

| 文件 | 作用 |
| --- | --- |
| `config.toml` | 项目主配置，定义默认 provider、model、soul 配置、运行模式和目录根 |
| `soul.toml` | 项目 Agent Card 兼容的 TOML 源配置，记录 agent 身份、组织、能力、skills、交互模式和发布占位字段 |
| `providers.toml` | 模型供应商、模型别名、light/medium/high 路由配置 |
| `tools.toml` | 工具注册、MCP、plugin、workflow tool 的项目级策略 |
| `permissions.toml` | 文件、Shell、网络、A2A 等能力边界 |
| `protocol.toml` | Local Rust、JSON-RPC、A2A、FFI 等协议接口层配置 |
| `mcp.json` | 项目 MCP server 声明，保持 MCP 默认 JSON 格式 |

## 读取顺序

1. 内嵌默认配置。
2. 用户配置 `~/.alius/config.toml`。
3. 项目配置 `.alius/config/config.toml`。
4. legacy 项目配置 `.alius/config.toml`。
5. 环境变量 `ALIUS__...`。
6. CLI 显式参数。

新实现应优先写入本目录；legacy 路径只用于兼容读取和迁移。除 MCP 标准配置 `mcp.json` 外，项目配置源文件统一使用 TOML；JSON 仍可作为 MCP 默认配置格式、协议载荷、发布导出格式或历史兼容读取来源。

## Core Runtime 配置职责

Config Manager 必须把本目录配置归一化为 Core Runtime 使用的稳定视图:

| 配置视图 | 来源 | 使用模块 |
| --- | --- | --- |
| `ProviderConfig` | `providers.toml`、`config.toml`、CLI 参数 | Model Router、Provider Manager |
| `ToolConfig` | `tools.toml`、`mcp.json` | Tool Executor |
| `PermissionConfig` | `permissions.toml` | Security Policy、Shell Gate |
| `ProtocolConfig` | `protocol.toml` | Protocol Interface Layer |
| `SoulConfig` | `soul.toml` | Soul Manager、Prompt Builder |
| `AgentCardView` | `soul.toml` | A2A Adapter |
| `LoggingConfig` | `config.toml` | Logging Manager |

后续用户配置入口可以通过 CLI/TUI 面板完善，但工程配置源文件仍以本目录为准。

## Shell 门禁配置

Shell 能力由 `permissions.toml` 中 `[shell]` 和 `[shell.scope]` 控制。原则:

- 默认限制在当前 workspace。
- 读写 workspace 外路径必须授权。
- `rm -rf`、递归删除、根路径删除、home 目录删除等高风险命令默认拒绝。
- RemoteA2A 默认不能请求 shell。
- 无法判断作用范围时，不自动允许。

## 日志配置

日志路径是固定工程约定，不在配置文件中指定。`config.toml` 中 `[logging]` 只控制是否开启、日志级别、脱敏和错误日志 flush 策略。

```text
.alius/memory/logs/
```

固定文件:

```text
.alius/memory/logs/runtime.log.jsonl
.alius/memory/logs/error.log.jsonl
.alius/memory/logs/audit.log.jsonl
.alius/memory/logs/trace/<trace-id>.jsonl
```

日志必须覆盖:

- runtime log。
- error log。
- exception log。
- audit log。
- 与 trace_id 关联的 trace log。

API key、token、Authorization header 和敏感用户输入必须脱敏。

## Agent Card 配置

项目级不再维护单独的 soul 状态。当前 Agent 身份和能力写入:

```text
.alius/config/soul.toml
```

`soul.toml` 是项目内的 TOML 源文件，不是发布后的 JSON 文件。它保存 Agent Card 可映射字段；发布为 A2A 服务时，再由该文件生成 `.well-known/agent-card.json`。

至少包含:

- `[agent]`: `name`、`description`、`version`。
- `[provider]`: `organization`、`url`。
- `[[supported_interfaces]]`: `url`、`protocol_binding`、`protocol_version`；其中 `url` 发布后才能确定，可先留空。
- `[agent_card]`: `documentation_url`、`icon_url`、`export_path`；公网 URL 可先留空。
- `[capabilities]`: `streaming`、`push_notifications`、`extended_agent_card`。
- `[interaction]`: `default_input_modes`、`default_output_modes`。
- `[[skills]]`: role/skill 的 `id`、`name`、`description`、`tags`、`examples`、输入输出模式。

导出为 A2A Agent Card JSON 时，字段按协议要求映射为 `supportedInterfaces`、`documentationUrl`、`iconUrl`、`defaultInputModes`、`defaultOutputModes` 等 camelCase 名称。
