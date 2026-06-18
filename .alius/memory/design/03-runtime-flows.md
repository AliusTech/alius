# 03. 运行流

更新时间: 2026-06-04 22:10

## `alius init`

定位: 项目级配置初始化。

当前流程:

1. CLI 进入 `Command::Init`。
2. 调用 `alius_interactive::tui::run_init_wizard()`。
3. 用户选择:
   - language
   - provider
   - base_url
   - api_key
   - model
   - Agent Card
4. Agent Card 可由用户选择 template、填写文本，或从 legacy installed soul 导入。
5. 如果使用 legacy soul 导入，本地没有对应 soul 时，init wizard 可尝试 `alius_formula::sync_all_souls()`。
6. 最终 Agent Card 写入 `.alius/config/soul.toml`。
7. 调用 `Settings::save_to_project_config()`。

写入结果:

```text
.alius/config.toml
.alius/mcp.json
.alius/memory/
.alius/memory/communications/sessions/
.alius/memory/design/
.alius/config/soul.toml
```

设计结论:

- `alius init` 不应写全局 `~/.alius/config.toml`。
- `alius init` 是项目 onboarding，而不是用户全局配置面板。

## 默认交互 `alius`

当前流程:

1. `alius-cli::run()` 加载 `Settings::load()`。
2. 设置 locale。
3. 无子命令时调用 `run_repl(settings)`。
4. 如果没有 `ALIUS_LEGACY_REPL`，进入 Ratatui workspace。
5. `ReplSession::new(settings)`:
   - 构建 `LlmClient`。
   - 注册内置工具。
   - 构建 `AliusAgent`。
   - 根据 `.alius/config/soul.toml` 中的 Agent Card 构建 system prompt。
   - 创建 `SessionMetadata`。
   - 创建 `SessionStore` 和 `ConversationStore`。
6. workspace 渲染 header、conversation/plans、interaction、status bar。
7. 用户输入:
   - Plan 模式先进入计划草稿和批准流程；批准后默认以 `BypassPermissions` 连续执行计划节点。
   - Plan 执行期可以切换到 `AcceptEdits`，后续工具确认点等待用户确认。
   - Bypass 模式直接执行。
8. 执行时调用 `collect_model_response()`。
9. `collect_model_response()` 当前走 `client.chat_stream(&conversation)`。
10. 结束后保存 messages 并更新 session。

关键点:

- 当前普通 workspace 响应没有走 `AliusAgent::handle_message()`，所以工具调用能力尚未接入默认交互执行路径。
- `ReplSession` 中已经构建了 agent 和 tool registry，为后续接线保留。

## 旧 REPL

进入条件:

```bash
ALIUS_LEGACY_REPL=1 alius
```

旧 REPL 基于 `rustyline`:

- 提供命令补全。
- 支持 `/init`、`/model`、`/config`、`/session`、`/memory`、`/doctor`、`/trace` 等命令。
- 直接打印 streaming delta。

旧 REPL 是兼容和调试路径，不是当前主产品体验。

## `alius run -p`

定位: 单次非交互调用。

当前流程:

1. 加载配置。
2. 如果传入 `run --model`，覆盖 settings 的 model。
3. 构建 `LlmClient`。
4. 调用 `chat_once(prompt, None)`。
5. 打印结果。

限制:

- 不注入当前 Agent Card system prompt。
- 不注入 global/project memory。
- 不保存 session。
- 不执行 tool loop。

## `/session`

旧 REPL 和 workspace 命令转发都支持:

- `/session current`
- `/session new`
- `/session list`
- `/session load <id-prefix>`
- `/session clear`

新 session 默认写入:

```text
.alius/memory/communications/sessions/<session-id>/session.json
.alius/memory/communications/sessions/<session-id>/messages.jsonl
```

读取时兼容:

```text
.alius/sessions/
~/.alius/sessions/
```

## `/memory`

当前命令行为:

- `/memory save <text>` 写全局 `~/.alius/memory/global.json`。
- `/memory list` 或 `/memory show` 同时读取 global 和 project memory。
- `/memory clear` 清空全局 memory。

上下文注入:

`ReplSession::build_system_prompt()` 会读取:

- `~/.alius/memory/global.json`
- `.alius/memory/project.json`

然后拼入 system prompt。

当前差距:

- 缺少显式保存 project memory 的命令，例如 `/memory save --project <text>`。
- `memory/design` 是 Markdown 文档目录，和 `project.json` 的结构化 memory 不同。

## `/review`

当前流程:

1. 找到最近一条 assistant message。
2. 构造英文 review prompt。
3. 如果配置了 `llm.review_model`，使用 review model。
4. 否则使用主 model。
5. 调用 `chat_once()`。

workspace 的 `auto_review` 打开后，会在执行完成后自动调用 `/review` 并将结果作为 decision block。

## `alius core update`

定位: 官方 Soul 仓库管理的兼容命令。

当前行为:

- 更新 `~/.alius/repos/souls`。
- 如果已有 git 仓库，执行 fetch + reset 到 `origin/main`。
- 如果不存在，优先用 SSH clone，失败后用 HTTPS clone。

它不负责同步 soul 到 `~/.alius/soul`。

命名说明:

- 远程仓库已经从 `AliusTech/alius-core` 改名为 `AliusTech/alius-souls`。
- 后续应避免把这个仓库称为 core，给 Rust Core Runtime 保留 Core 语义。

## `alius soul update`

定位: 本地 soul 缓存同步。

当前行为:

1. 调用 `sync_all_souls()`。
2. 先更新或克隆 alius-souls。
3. 读取 `Formula/souls/*.toml`。
4. 对每个 soul 调用 `install_soul()`。
5. 复制 formula 和 prompt 文件到 `~/.alius/soul/<id>/versions/<version>/`。

这是 CLI 中刷新 soul 列表的正确命令。

## legacy `alius config soul --role <id>`

当前行为:

- 确认对应 legacy soul 可用，必要时先安装到全局缓存。
- 如果本地没有该 legacy soul，会尝试从 official repo 同步。
- 成功后将 legacy soul 映射为 `.alius/config/soul.toml`，不创建项目级 soul 目录。

设计说明:

- 由于没有 `alius soul use`，项目级 Agent Card 由 init 或后续 Agent Card 配置命令承担；`config soul` 只作为 legacy 导入入口。
