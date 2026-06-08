# Config Manager

更新时间: 2026-06-05 03:43

## 模块职责

Config Manager 负责读取、合并、校验和迁移项目级配置，是 Core Runtime 的基础模块之一。Core Runtime 不直接读取零散配置文件，而是读取 Config Manager 输出的稳定配置快照。

输入:

- `.alius/config/*.toml`
- `.alius/config/mcp.json`
- legacy `.alius/config.toml`
- legacy `.alius/mcp.json`
- legacy project soul active marker
- 用户级 `~/.alius/config.toml`
- 环境变量和 CLI 参数

输出:

- `ProjectConfigSnapshot`
- `ProviderConfig`
- `ToolConfig`
- `PermissionConfig`
- `ProtocolConfig`
- `SoulConfig`
- `AgentCardView`
- `RuntimeConfigView`
- `LoggingConfig`
- `ShellGateConfig`

## 接口定义

## 代码位置

| 组件 | 位置 |
| --- | --- |
| Embedded defaults 配置 | `runtime/core/config/defaults/*.toml`、`mcp.json` |
| 配置加载与项目结构创建 | `runtime/core/src/config.rs` |
| CLI 配置读取 | `entrypoints/cli/src/config/src/settings.rs` |

当前落地状态:

- `core-runtime/src/config.rs` 已实现 `project_config_exists`、`reset_project_config`、`ensure_full_project_structure`。
- Embedded defaults 已包含 config/providers/soul/tools/permissions/protocol.toml 和 mcp.json。
- 完整分层合并（user/project/env/CLI override）和 `RuntimeConfigView` 输出仍为 H1 研发任务。

```text
load_project_config(cwd: Path) -> Result<ProjectConfigSnapshot>
```

参数:

- `cwd`: 当前工作目录。

返回:

- 合并后的项目配置快照。

异常:

- 配置 TOML/JSON 解析失败。
- 配置字段类型不合法。

```text
find_project_root(cwd: Path) -> Option<Path>
```

返回:

- 向上查找到的项目根目录。

```text
migrate_legacy_config(root: Path) -> Result<MigrationReport>
```

返回:

- 旧路径迁移报告。

```text
build_runtime_config(snapshot: ProjectConfigSnapshot) -> Result<RuntimeConfigView>
```

返回:

- Core Runtime 可直接使用的运行时配置视图。

```text
watch_config(root: Path) -> Stream<ConfigChangeEvent>
```

返回:

- 配置变更事件，用于后续支持热更新。

## 内部逻辑

```text
find_project_root
-> read embedded defaults
-> read user config
-> read .alius/config/config.toml
-> read .alius/config/soul.toml
-> normalize soul.toml into AgentCardView
-> read legacy .alius/config.toml
-> read legacy project soul active marker
-> read config split files
-> apply env overrides
-> apply CLI overrides
-> validate
-> build runtime config view
-> return ProjectConfigSnapshot
```

## 配置分层

| 层级 | 路径或来源 | 优先级 | 说明 |
| --- | --- | --- | --- |
| Embedded defaults | 编译内置默认值 | 最低 | 保证最小可运行默认值 |
| User config | `~/.alius/config.toml` | 低 | 用户全局偏好 |
| Project config | `.alius/config/*.toml` 和 `.alius/config/mcp.json` | 中 | 当前 workspace 的工程配置 |
| Legacy config | `.alius/config.toml`、legacy mcp/soul 路径 | 兼容读取 | 只读兼容和迁移来源 |
| Env overrides | `ALIUS__...` | 高 | CI 或临时覆盖 |
| CLI args | 命令行参数 | 最高 | 当前进程覆盖，不直接持久化 |

## Core Runtime 配置视图

Config Manager 必须输出以下稳定视图:

| 视图 | 用途 |
| --- | --- |
| `ProviderConfig` | Provider Manager 和 Model Router |
| `ToolConfig` | Tool Executor 和 MCP 工具注册 |
| `PermissionConfig` | Security Policy 和 Shell Gate |
| `ProtocolConfig` | Protocol Interface Layer |
| `SoulConfig` | Soul Manager 和 Prompt Builder |
| `AgentCardView` | A2A Adapter 发布和远端发现 |
| `LoggingConfig` | Logging Manager 开关、日志级别、脱敏和 flush 策略 |
| `RuntimeConfigView` | Core Runtime 启动所需的合并视图 |

## 用户配置方式

当前规范先固定文件边界，后续再完善用户交互:

- 用户通过 `alius init` 初始化项目配置。
- 用户通过 `alius config show` 查看合并后的摘要。
- 用户通过 `alius config validate` 校验配置。
- 用户通过 `.alius/config/*.toml` 维护项目配置。
- MCP server 通过 `.alius/config/mcp.json` 维护，保持 MCP 默认 JSON 格式。
- CLI 参数只影响当前进程，不直接写入项目配置。

## 数据存储

| 文件 | 说明 |
| --- | --- |
| `.alius/config/config.toml` | 项目主配置 |
| `.alius/config/soul.toml` | 项目 Agent Card 兼容的 TOML 源配置，记录 agent 身份、组织、supported interfaces、capabilities、skills、交互模式和发布占位字段 |
| `.alius/config/providers.toml` | provider 和模型路由 |
| `.alius/config/tools.toml` | 工具注册策略 |
| `.alius/config/permissions.toml` | 权限策略 |
| `.alius/config/protocol.toml` | 协议接口层配置 |
| `.alius/config/mcp.json` | MCP server 声明，保持 MCP 默认 JSON 格式 |

## 异常处理

- split 配置缺失: 使用默认值。
- legacy 配置存在: 读取兼容，并提示迁移。
- legacy project soul active marker 存在: 读取后迁移到 `.alius/config/soul.toml`，不再创建项目级 soul 目录。
- TOML/JSON 解析失败: 返回配置错误，不进入 Core Runtime。
- 配置冲突: 返回 `ConfigConflict`，高风险配置不自动合并。
- 日志路径固定为 `.alius/memory/logs/`，不从配置文件读取，也不允许项目配置覆盖。
- Shell Gate 配置缺失: 使用最小权限默认值。

## 与其他模块的关系

- 向 Protocol Interface Layer 提供 protocol 和 capability 上限配置。
- 向 Core Runtime 提供 provider、tools、permissions、memory、workspace 路径和 `SoulConfig`。
- 向 A2A Adapter 提供由 `soul.toml` 归一化生成的 `AgentCardView`；对外发布时再导出为 `.well-known/agent-card.json`。
- 向 Shell Gate 提供 shell allowlist、denylist、workspace scope 和 approval 策略。
- 向 Logging Manager 提供 level、路径、轮转和脱敏策略。

## 验收标准

- 能从项目子目录向上找到 `.alius/config/config.toml`。
- 能兼容读取 `.alius/config.toml`。
- 能读取 `.alius/config/soul.toml`。
- 能把 `soul.toml` 归一化为 A2A Agent Card JSON 的字段视图。
- 能将 legacy project soul active marker 迁移到 `.alius/config/soul.toml`。
- 能输出完整 `ProjectConfigSnapshot`。
- 能输出 Core Runtime 启动所需的 `RuntimeConfigView`。
- 能为 Shell Gate 和 Logging Manager 输出默认配置。
