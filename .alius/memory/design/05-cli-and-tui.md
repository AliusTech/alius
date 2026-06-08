# 05. CLI 与交互工作区

更新时间: 2026-06-04 22:10

## CLI 命令结构

`alius-cli` 使用 `clap` 定义命令。

顶层命令:

| 命令 | 当前定位 |
| --- | --- |
| `alius` | 默认进入 Ratatui workspace |
| `alius repl` | 显式进入交互模式 |
| `alius run -p <prompt>` | 单次 prompt 调用 |
| `alius config` | 配置展示、校验、Agent Card 更新；legacy soul 可导入为 Agent Card |
| `alius version` | 输出编译期版本 |
| `alius init` | 项目级初始化 |
| `alius core` | 官方 Soul 仓库管理的兼容命令 |
| `alius soul` | 本地 soul 缓存和安装管理 |
| `alius plugin` | WASM plugin 管理 |
| `alius mcp` | MCP server 管理 |
| `alius workflow` | JSON workflow 管理 |

定义但当前未接线的全局 flag:

- `--model`
- `--provider`
- `--workspace`
- `--config`
- `--verbose`

当前实际生效:

- `run --model` 会覆盖本次运行 model。
- 根级 `--config/--provider/--workspace/--verbose` 在 `run()` 中尚未消费。

## Soul 命令约定

当前 `alius soul` 子命令:

```text
update
list
install <id>
current
remove <id>
```

明确不提供:

```text
use
```

设计原因:

- legacy soul 不再作为项目级目标状态；如需使用，应导入为 `.alius/config/soul.toml`。
- 项目级 Agent Card 应通过 `alius init` 或后续 Agent Card 配置命令更新。
- `alius soul update` 是本地缓存同步。
- `alius soul install` 是安装到全局缓存，不等于项目 Agent Card 更新。

## Ratatui workspace

默认交互入口:

```rust
run_repl(settings)
```

如果没有 `ALIUS_LEGACY_REPL`，会进入:

```rust
crate::tui::workspace::run_workspace(session, initial_missing)
```

主界面布局:

```text
┌────────────────────────────────────────────────────────────┐
│ top bar: Alius version, soul, network status               │
├────────────────────────────────────┬───────────────────────┤
│ Conversation / Agent Team tab       │ Plans panel           │
├────────────────────────────────────┴───────────────────────┤
│ Interaction surface: text input or decision selector        │
├────────────────────────────────────────────────────────────┤
│ status bar: cwd, repo, branch, git dirty counts             │
└────────────────────────────────────────────────────────────┘
```

交互模式:

| 模式 | 用途 |
| --- | --- |
| Plan | 先形成计划和确认，再执行 |
| Bypass | 直接把输入交给模型执行 |

关键按键:

| 按键 | 行为 |
| --- | --- |
| `Shift+Tab` | Plan/Bypass 切换 |
| `Ctrl+Enter` | 提交 |
| `Ctrl+Tab` | Conversation/Agent Team 切换，仅 Agent Team 可见时生效 |
| `Esc` | 清空输入或取消当前 decision |
| `Ctrl+C` / `Ctrl+D` | 退出 |

## Plan 模式当前行为

用户在 Plan 模式输入目标后，当前实现会创建固定计划节点:

1. understand
2. decompose
3. execute
4. review
5. finalize

每个节点有:

- id
- title
- status
- description
- acceptance_criteria
- evidence
- owner

当前计划节点是 UI 状态和交互引导，不是由 LLM 动态生成的完整 plan schema。

执行完成后:

- execute 节点标记 completed。
- review 节点进入 review 状态。
- 弹出 node review decision。
- 可 approve、request revision、view evidence、rerun。

## Conversation blocks

workspace 不直接显示 User/Assistant 传统 transcript 标签，而使用工作流 block:

- Request
- Understanding
- PlanProposal
- Execution
- Decision
- Result
- Error

这符合“Plan-driven Agent Runtime Workspace”的产品语言。

## Agent Team 脚手架

`AgentTeamState` 和 A2A 类型已经存在:

- `A2AMessage`
- `AgentEndpoint`
- `A2AMessageType`
- `A2AMessageStatus`
- `A2ADirection`
- `A2AMessageView`

重要约定:

- local Conversation 和 Agent Team/A2A 流量必须分开。
- A2A 消息视图必须保留 from、to 和 IN/OUT 方向。
- Agent Team tab 只有在存在 team state 或网络状态不是 Standalone 时才应出现。

当前实现状态:

- Header 总是 `AgentHeader::standalone(session.soul())`。
- `WorkspaceState.agent_team` 初始为 `None`。
- 因此 Agent Team 页面是 dormant UI scaffold，没有 live AgentNet plumbing。

## 旧 REPL 命令

旧 REPL 和 workspace 命令转发共用部分命令:

```text
/init
/model
/config
/session
/history
/review
/memory
/doctor
/trace
/confirm
/tools
/clear
/help
/quit
/exit
```

workspace 内对部分命令做了特殊处理:

- `/init` 挂起当前 TUI，打开 init wizard。
- `/model` 挂起当前 TUI，打开 model selector。
- `/config` 挂起当前 TUI，打开 config panel。
- 其他命令转发给 `ReplSession::handle_command()`。

## i18n

交互文案位于:

```text
crates/alius-interactive/locales/en.yml
crates/alius-interactive/locales/zh-CN.yml
crates/alius-interactive/locales/ja.yml
```

locale 来源:

- `Settings.ui.locale`
- init wizard 第一页可选择语言。
- config panel 可切换 language。

## 当前 UI 设计边界

- Workspace 是当前主体验。
- 旧 REPL 是 fallback。
- Agent Team 是预留 UI，不应在文档或产品文案中承诺为已连接能力。
- Plan 当前是本地固定节点，不是完整自动规划引擎。
