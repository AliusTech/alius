# Soul Manager

更新时间: 2026-06-04 22:10

## 模块职责

Soul Manager 负责项目级 Agent 身份、skills、policy 和 Agent Card-compatible 信息的读取、归一化和迁移。

当前项目口径:

- 项目配置源是 `.alius/config/soul.toml`。
- 不创建项目级 `.alius/soul/` 目录。
- legacy soul 只作为导入来源。
- A2A 发布时由 `soul.toml` 导出 `.well-known/agent-card.json`。

输入:

- `.alius/config/soul.toml`
- legacy `.alius/soul/.active`
- global soul cache
- official soul repository sync result

输出:

- `SoulConfig`
- `AgentCardView`
- prompt identity source
- policy source
- skill registry source

## 接口定义

```text
load_soul_config(root: Path) -> Result<SoulConfig>
```

```text
normalize_agent_card(config: SoulConfig) -> Result<AgentCardView>
```

```text
migrate_legacy_soul(root: Path) -> Result<MigrationReport>
```

```text
export_agent_card(config: SoulConfig, target: Path) -> Result<ExportReport>
```

异常:

- `soul.toml` TOML 解析失败。
- 必需 identity 或 skill 字段不合法。
- 发布导出时 required public URL 缺失。

## 内部逻辑

```text
read .alius/config/soul.toml
-> validate agent / provider / capabilities / interaction / skills
-> normalize snake_case TOML fields to Agent Card-compatible view
-> provide identity to Prompt Builder
-> provide skills and policy to A2A Adapter
-> optionally export .well-known/agent-card.json when publishing
```

## 数据存储

| 路径 | 说明 |
| --- | --- |
| `.alius/config/soul.toml` | 项目级 Agent 身份、组织、skills、capabilities、发布 URL 占位 |
| `~/.alius/soul/` | 全局 legacy soul 缓存 |
| `~/.alius/repos/souls/` | 官方 soul 仓库缓存 |

## 字段映射

| `soul.toml` | Agent Card JSON |
| --- | --- |
| `[agent].name` | `name` |
| `[agent].description` | `description` |
| `[agent].version` | `version` |
| `[provider]` | `provider` |
| `[[supported_interfaces]]` | `supportedInterfaces[]` |
| `[agent_card].documentation_url` | `documentationUrl` |
| `[agent_card].icon_url` | `iconUrl` |
| `[capabilities].push_notifications` | `capabilities.pushNotifications` |
| `[capabilities].extended_agent_card` | `capabilities.extendedAgentCard` |
| `[interaction].default_input_modes` | `defaultInputModes` |
| `[interaction].default_output_modes` | `defaultOutputModes` |
| `[[skills]]` | `skills[]` |

## 异常处理

- legacy `.alius/soul/.active` 存在时只迁移，不继续写项目级 soul 目录。
- 发布 URL 为空时允许本地运行，但禁止发布 Agent Card JSON。
- skill id 冲突时返回配置错误。

## 与其他模块的关系

- 驱动 Prompt Builder。
- 向 A2A Adapter 提供 Agent Card 和 policy。
- 由 Config Manager 读取和校验。
- 可从 Storage Manager 读取 global cache。

## 验收标准

- 能读取 `.alius/config/soul.toml`。
- 能归一化为 `AgentCardView`。
- 能从 legacy soul 导入到 `soul.toml`。
- 能在发布阶段导出 `.well-known/agent-card.json`。
