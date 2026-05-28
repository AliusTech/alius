# Alius

> LLM Agent CLI -- 探索软件自进化的工程实践
>
> LLM Agent CLI -- Engineering Practice of Software Self-Evolution
>
> LLM Agent CLI -- ソフトウェア自己進化のエンジニアリング実践

[English](#english) | [中文](#中文) | [日本語](#日本語)

---

## English

Alius is an interactive LLM command-line tool, designed as an engineering practice platform for AI-assisted software development.

### Core Philosophy -- Software Self-Evolution

Software self-evolution is the core design philosophy of Alius.

In traditional software development, developers write code, tests, and documentation, then deliver. But software in the AI era should be able to **participate in its own evolution**.

Alius CLI itself is a living example of software self-evolution -- its development process heavily leverages AI assistance, demonstrating how human-AI collaboration can accelerate the software development lifecycle.

**Self-Iteration** -- AI-assisted code improvement and feature development. Software is no longer a one-time deliverable, but a continuously evolving entity.

**Adaptive Configuration** -- Intelligent parameter adjustment based on usage scenarios. The tool adapts to the user, not the other way around.

**Tool-based Architecture** -- Modular, extensible tool system design, enabling Agents to invoke external capabilities for complex tasks.

### Features

- **Interactive REPL** -- Slash commands and real-time chat
- **Multi-model Support** -- OpenAI, Anthropic, Gemini and more
- **Flexible Configuration** -- YAML/TOML/JSON config files
- **Beautiful UI** -- ASCII logo and colored output

### Installation

Install via npm:

```bash
npm install -g @alius-tech/alius
```

Install via Homebrew:

```bash
brew tap AliusTech/tap
brew install alius
```

Or download from [Releases](https://github.com/AliusTech/alius/releases):

```bash
chmod +x alius
mv alius ~/.local/bin/
```

Update / Uninstall:

```bash
npm update -g @alius-tech/alius
npm uninstall -g @alius-tech/alius
```

### Configuration

Set API key environment variables:

```bash
export OPENAI_API_KEY=your-api-key
export ANTHROPIC_API_KEY=your-api-key
export GEMINI_API_KEY=your-api-key
```

### Usage

```bash
# Interactive mode
alius

# Run a task directly
alius run -p "Explain what Rust is"

# Show configuration
alius config show
```

### REPL Commands

| Command | Description |
|---------|-------------|
| `/model` | Select model |
| `/config` | Show configuration |
| `/help` | Show help |
| `/quit` | Exit |

### Config File

Located at `~/.alius/config.toml`:

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

### Supported Models

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

### Development

```bash
git clone https://github.com/AliusTech/alius.git
cd alius
cargo build --release
```

### License

MIT License - see [LICENSE](LICENSE)

---

## 中文

Alius 是一个交互式 LLM 命令行工具，致力于成为 AI 辅助软件开发的工程实践平台。

### 核心理念 -- 软件自进化

软件自进化是 Alius 项目的核心设计哲学。

传统软件开发中，开发者编写代码、测试、文档，然后交付。但 AI 时代的软件应该能够**参与自身的进化过程**。

Alius CLI 本身即是软件自进化的实践案例 -- 其开发过程充分借助 AI 辅助，展示了人机协作如何加速软件开发周期。

**自我迭代** -- 通过 AI 辅助进行代码改进和新功能开发。软件不再是一次性交付的产物，而是持续进化的生命体。

**自适应配置** -- 根据使用场景智能调整参数，让工具适应人，而不是人适应工具。

**工具化架构** -- 模块化、可扩展的工具系统设计，让 Agent 能够调用外部能力，完成复杂任务。

### 功能特性

- **交互式 REPL 模式** -- 支持斜杠命令和实时聊天
- **多模型支持** -- OpenAI、Anthropic、Gemini 等主流模型
- **灵活配置** -- 支持 YAML/TOML/JSON 配置文件
- **美观界面** -- ASCII Logo 和彩色输出

### 安装

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

更新 / 卸载：

```bash
npm update -g @alius-tech/alius
npm uninstall -g @alius-tech/alius
```

### 配置

设置 API Key 环境变量：

```bash
export OPENAI_API_KEY=your-api-key
export ANTHROPIC_API_KEY=your-api-key
export GEMINI_API_KEY=your-api-key
```

### 使用

```bash
# 进入交互模式
alius

# 直接运行任务
alius run -p "解释什么是 Rust"

# 查看配置
alius config show
```

### 交互命令

| 命令 | 功能 |
|------|------|
| `/model` | 选择模型 |
| `/config` | 显示配置 |
| `/help` | 显示帮助 |
| `/quit` | 退出 |

### 配置文件

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

### 支持的模型

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

### 开发

```bash
git clone https://github.com/AliusTech/alius.git
cd alius
cargo build --release
```

### 许可证

MIT License - 详见 [LICENSE](LICENSE)

---

## 日本語

Alius は、AI 駆動のソフトウェア開発のためのエンジニアリング実践プラットフォームを目指すインタラクティブな LLM コマンドラインツールです。

### コア理念 -- ソフトウェア自己進化

ソフトウェア自己進化は、Alius プロジェクトのコア設計思想です。

従来のソフトウェア開発では、開発者がコード、テスト、ドキュメントを書いてから納品します。しかし、AI 時代のソフトウェアは**自身の進化プロセスに参加できる**べきです。

Alius CLI はソフトウェア自己進化の生きた実例です -- その開発プロセスは AI アシスタンスを最大限に活用し、人間と AI の協力がソフトウェア開発サイクルを如何に加速できるかを示しています。

**自己反復** -- AI によるコード改善と新機能開発。ソフトウェアは一度きりの納品物ではなく、継続的に進化する生命体です。

**適応的設定** -- 使用シナリオに基づくインテリジェントなパラメータ調整。ツールが人に適応するのです。

**ツールベースアーキテクチャ** -- モジュラーで拡張可能なツールシステム設計。エージェントが外部機能を呼び出し、複雑なタスクを完了できます。

### 機能

- **インタラクティブ REPL** -- スラッシュコマンドとリアルタイムチャット
- **マルチモデル対応** -- OpenAI、Anthropic、Gemini など
- **柔軟な設定** -- YAML/TOML/JSON 設定ファイル
- **美しい UI** -- ASCII ロゴとカラー出力

### インストール

npm でインストール：

```bash
npm install -g @alius-tech/alius
```

Homebrew でインストール：

```bash
brew tap AliusTech/tap
brew install alius
```

または [Releases](https://github.com/AliusTech/alius/releases) からダウンロード：

```bash
chmod +x alius
mv alius ~/.local/bin/
```

更新 / アンインストール：

```bash
npm update -g @alius-tech/alius
npm uninstall -g @alius-tech/alius
```

### 設定

API キーの環境変数を設定：

```bash
export OPENAI_API_KEY=your-api-key
export ANTHROPIC_API_KEY=your-api-key
export GEMINI_API_KEY=your-api-key
```

### 使い方

```bash
# インタラクティブモード
alius

# タスクを直接実行
alius run -p "Rust とは何か説明して"

# 設定を表示
alius config show
```

### REPL コマンド

| コマンド | 説明 |
|----------|------|
| `/model` | モデル選択 |
| `/config` | 設定表示 |
| `/help` | ヘルプ表示 |
| `/quit` | 終了 |

### 設定ファイル

`~/.alius/config.toml` に配置：

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

### 対応モデル

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

### 開発

```bash
git clone https://github.com/AliusTech/alius.git
cd alius
cargo build --release
```

### ライセンス

MIT License - [LICENSE](LICENSE) を参照
