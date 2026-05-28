# Alius

[![npm version](https://img.shields.io/npm/v/@alius-tech/alius)](https://www.npmjs.com/package/@alius-tech/alius)
[![homebrew version](https://img.shields.io/homebrew/v/alius)](https://formulae.brew.sh/formula/alius)

> LLM Agent CLI -- 探索软件自进化的工程实践

[English](README.en.md) | [日本語](README.ja.md)

Alius 是一个交互式 LLM 命令行工具，致力于成为 AI 辅助软件开发的工程实践平台。

## 核心理念 -- 软件自进化

软件自进化是 Alius 项目的核心设计哲学。

传统软件开发中，开发者编写代码、测试、文档，然后交付。但 AI 时代的软件应该能够**参与自身的进化过程**。

Alius CLI 本身即是软件自进化的实践案例 -- 其开发过程充分借助 AI 辅助，展示了人机协作如何加速软件开发周期。

**自我迭代** -- 通过 AI 辅助进行代码改进和新功能开发。软件不再是一次性交付的产物，而是持续进化的生命体。

**自适应配置** -- 根据使用场景智能调整参数，让工具适应人，而不是人适应工具。

**工具化架构** -- 模块化、可扩展的工具系统设计，让 Agent 能够调用外部能力，完成复杂任务。

## 功能特性

- **交互式 REPL 模式** -- 支持斜杠命令和实时聊天
- **多模型支持** -- OpenAI、Anthropic、Gemini 等主流模型
- **灵活配置** -- 支持 YAML/TOML/JSON 配置文件
- **美观界面** -- ASCII Logo 和彩色输出

## 安装

通过 npm 安装：

```bash
npm install -g @alius-tech/alius
```

通过 Homebrew 安装：

```bash
brew tap AliusTech/tap
brew install alius
```

或从 [Releases](https://github.com/AliusTech/alius/releases) 下载：

```bash
chmod +x alius
mv alius ~/.local/bin/
```

更新：

```bash
# npm
npm update -g @alius-tech/alius

# Homebrew
brew update && brew upgrade alius
```

卸载：

```bash
# npm
npm uninstall -g @alius-tech/alius

# Homebrew
brew uninstall alius && brew untap AliusTech/tap
```

## 使用

```bash
# 进入交互模式
alius

# 直接运行任务
alius run -p "解释什么是 Rust"

# 查看配置
alius config show
```

## 交互命令

| 命令 | 功能 |
|------|------|
| `/model` | 选择模型 |
| `/config` | 显示配置 |
| `/help` | 显示帮助 |
| `/quit` | 退出 |

## 配置文件

配置位于 `~/.alius/config.toml`：

```toml
[llm]
provider = "openai"
model = "gpt-4o-mini"
api_key_env = "OPENAI_API_KEY"
base_url = "https://api.openai.com/v1"

[agent]
max_retries = 3
timeout_seconds = 60
```

## 支持的模型

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

## 开发

```bash
git clone https://github.com/AliusTech/alius.git
cd alius
cargo build --release
```

## 许可证

MIT License - 详见 [LICENSE](LICENSE)
