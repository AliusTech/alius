# 01. 项目概览

更新时间: 2026-06-04 22:10

## 产品定位

Alius 是一个交互式 LLM Agent CLI，目标是成为 AI 辅助软件开发和“软件自进化”的工程实践平台。它不是单纯聊天窗口，而是面向项目工作的 Agent Runtime Workspace:

- 能读取项目级配置和项目记忆。
- 能用 Agent Card 组织系统提示词、工作风格、skills 和 A2A 暴露能力。
- 能在 TUI 中围绕目标形成计划、执行、复核、证据记录。
- 能通过工具、MCP、插件、工作流扩展能力边界。

当前代码已经从早期文本 REPL 过渡到 Ratatui workspace 为默认交互入口；旧 `rustyline` REPL 仍可通过 `ALIUS_LEGACY_REPL=1` 进入。

## 顶层设计目标

1. 项目优先
   - `alius init` 初始化当前项目。
   - 项目配置和记录写入 `.alius/`。
   - 全局 `~/.alius` 只承担用户级缓存、全局 memory、插件和官方 Soul 仓库。

2. Agent Card 驱动
   - Agent Card 是 Agent 的身份、能力、skills 和 A2A 发现信息来源。
   - legacy official soul 可作为导入或迁移来源，但项目主路径使用 `.alius/config/soul.toml`。
   - 项目只保存 Agent Card 配置，不复制完整 legacy soul 内容，也不创建项目级 soul 目录。

3. Plan-driven workspace
   - 默认交互不是普通聊天 UI，而是围绕目标、计划节点、验收、证据、review 的工作区。
   - `Shift+Tab` 切换 Plan/Bypass 模式。
   - `Ctrl+Enter` 提交。
   - `Ctrl+Tab` 预留给 Agent Team 页面切换。

4. 可扩展 Agent Runtime
   - 模型 provider 可替换。
   - 工具通过统一 trait 和 JSON Schema 暴露给 LLM。
   - MCP、WASM Plugin、Workflow 是扩展方向。

## Rust workspace 成员

当前根 `Cargo.toml` 有 11 个 crate:

| Crate | 职责 |
| --- | --- |
| `alius-cli` | 二进制入口、Clap 命令、命令分发、版本输出 |
| `alius-config` | 配置结构、默认配置、配置加载合并、项目级保存 |
| `alius-protocol` | 跨 crate 共享类型、错误、消息、session、provider、tool def |
| `alius-model` | LLM provider 抽象、OpenAI/Anthropic 实现、Conversation、Agent loop |
| `alius-interactive` | REPL、Ratatui workspace、init/config/model TUI、i18n |
| `alius-store` | memory、session、conversation 落盘 |
| `alius-tools` | 内置工具 trait、registry、权限结构、文件/git/http/json/todo/shell 工具 |
| `alius-formula` | alius-souls 仓库管理、formula 解析、soul 安装和项目激活 |
| `alius-mcp` | MCP stdio server 配置、启动、tools/list、tools/call |
| `alius-plugin` | WASM plugin 安装、列举、调用 ABI |
| `alius-workflow` | JSON workflow 解析、变量插值、执行器雏形 |

## 非 Rust 发布包装

`npm-packages/` 提供 npm 分发包装:

- `@alius-tech/alius` 是主 wrapper。
- 平台包包含或下载 native binary。
- wrapper 根据 `process.platform` 与 `process.arch` 查找平台包二进制并透传参数。

`scripts/` 和 `npm-packages/*.js` 提供发布辅助:

- 版本生成和校验。
- 平台包 package.json 生成。
- npm 发布脚本。

## 当前重要状态

- 当前 workspace 包版本为 `0.6.4`。
- `CHANGELOG.md` 记录到 `0.6.1`，已有滞后。
- `README.md` 与 `CONTRIBUTING.md` 仍有旧结构描述，不能作为唯一真相。
- 代码实现已进入项目级 `.alius` 方向，外部文档需要后续同步。
