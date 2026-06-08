# 08. 发布、文档与风险

更新时间: 2026-06-04 22:10

## 版本解析

`crates/alius-cli/build.rs` 在编译期设置 `ALIUS_VERSION`。

解析顺序:

1. `ALIUS_VERSION`
2. `GITHUB_REF_NAME`
3. `GITHUB_REF`
4. 当前 git exact tag，格式 `v[0-9]*`
5. `CARGO_PKG_VERSION`
6. `0.0.0`

版本会去掉:

- `refs/tags/`
- 前缀 `v`

并校验 semver core。

## npm 包装

主包:

```text
@alius-tech/alius
```

平台包:

```text
@alius-tech/alius-darwin-x64
@alius-tech/alius-darwin-arm64
@alius-tech/alius-linux-x64
@alius-tech/alius-linux-arm64
@alius-tech/alius-win32-x64
@alius-tech/alius-win32-arm64
```

主 wrapper:

- 根据 `process.platform` 和 `process.arch` 计算 platform key。
- 查找平台包导出的 binary path。
- 或查找 sibling package、vendor、development binary。
- spawn native binary 并透传 stdio 和信号。

发布辅助:

- `npm-packages/generate-packages.js`
- `npm-packages/verify-release-version.js`
- `scripts/check-version.sh`
- `scripts/publish-npm.sh`

当前注意:

- 主 package version 为 `0.0.3`。
- optionalDependencies 版本为 `0.0.1`。
- Rust workspace version 为 `0.6.4`。
- 版本源可能存在不同步，需要发布前统一校验。

## 顶层文档现状

`README.md`、`README.en.md`、`README.ja.md` 当前仍描述:

- 交互式 REPL 为主。
- 配置位于 `~/.alius/config.toml`。
- 功能表较早。

当前代码现状:

- 默认交互入口是 Ratatui workspace。
- `alius init` 写项目级 `.alius/config.toml`。
- `.alius/memory/` 已成为项目记录目录。
- `alius soul update` 是同步 soul 的主要命令。

`CONTRIBUTING.md` 当前仍按旧单 crate `src/*` 结构描述，已经滞后于 workspace 架构。

`CHANGELOG.md` 记录到 `0.6.1`，当前 workspace 是 `0.6.4`。

## 当前实现风险

### CLI flag 未完全接线

`Cli` 定义了:

- `--model`
- `--provider`
- `--workspace`
- `--config`
- `--verbose`

但 `alius-cli::run()` 当前没有使用这些根级 flag。

影响:

- 用户以为 `alius --config path` 生效，但实际仍走 `Settings::load()`。
- `--workspace` 不会改变 cwd 或 tool workspace。

### MCP 项目配置查找不一致

Config/store/formula 的项目 `.alius` 解析会向上查找。

MCP 当前只查:

```text
.alius/mcp.json
alius/mcp.json
~/.alius/mcp.json
```

且 `.alius/mcp.json` 是相对当前工作目录。

影响:

- 在项目子目录运行 `alius mcp list` 可能读不到项目根 `.alius/mcp.json`。

### 工具系统未接入默认 workspace

Agent loop 和工具注册已经存在，但当前 workspace 的普通执行路径仍直接调用 `chat_stream()`。

影响:

- `/tools` 能列出工具，不代表模型能在默认对话中调用工具。
- 工具权限、确认、结果 evidence 尚未成为主体验的一部分。

### 权限结构未强制执行

`PermissionManager` 存在，但没有在 agent 执行路径中统一检查。

影响:

- 目前主要依赖每个 tool 的 `requires_confirmation()`。
- shell/http 等工具还需要更强策略。

### Conversation append 行为不一致

`ConversationStore::append_message()` 注释为 append-only，但实现使用 `std::fs::write`，会覆盖文件。

影响:

- 当前主路径使用 `save_messages()`，风险暂时有限。
- 如果后续改成增量写 messages，会丢历史消息。

### 文件工具安全边界不完全一致

大多数文件工具会检查 workspace containment。

已观察到的风险:

- `move_file` 校验 source 在 workspace 内，但 destination 没有对称 canonical containment 检查。
- `shell` 只拦截少量危险 pattern。

### Provider 支持文档和实现不一致

代码中:

- OpenAI、BigModel、Custom 已走 OpenAI-compatible provider。
- Anthropic 已实现。
- Google provider 返回未实现错误。

README 中仍列 Google 为支持项，需要修正为“配置枚举存在，但 provider 未实现”。

### Workflow 是 runtime 雏形

workflow 支持 JSON 解析和变量插值，但:

- prompt step 未调用 LLM。
- tool step 未调用 ToolRegistry。
- condition 只是占位。

不能把它描述成完整自动化流水线。

## 文档同步待办

1. 更新 README:
   - 默认 workspace。
   - `alius init` 项目级配置。
   - `.alius/` 目录结构。
   - `alius soul update`。
2. 更新 CONTRIBUTING:
   - workspace crate 架构。
   - 正确验证命令。
   - TUI 和 store/formula 修改注意事项。
3. 更新 CHANGELOG:
   - 0.6.2 到 0.6.4。
   - 项目级 `.alius`。
   - soul update 行为。
   - workspace/TUI 变更。
4. 增加用户文档:
   - project memory。
   - global memory。
   - MCP 配置示例。
   - soul 生命周期。

## 推荐验证命令

局部验证:

```bash
cargo test -p alius-config
cargo test -p alius-store
cargo test -p alius-formula
cargo test -p alius-interactive
cargo check --workspace
```

CLI 包名:

```bash
cargo check -p alius
```

涉及 `alius-formula` 的全仓测试建议串行:

```bash
cargo test -- --test-threads=1
```
