# Alius

[![npm version](https://img.shields.io/npm/v/@alius-tech/alius)](https://www.npmjs.com/package/@alius-tech/alius)
[![homebrew version](https://img.shields.io/homebrew/v/alius)](https://formulae.brew.sh/formula/alius)

> LLM Agent CLI -- Engineering Practice of Software Self-Evolution

[中文](README.md) | [日本語](README.ja.md)

Alius is an interactive LLM command-line tool, designed as an engineering practice platform for AI-assisted software development.

## Core Philosophy -- Software Self-Evolution

Software self-evolution is the core design philosophy of Alius.

In traditional software development, developers write code, tests, and documentation, then deliver. But software in the AI era should be able to **participate in its own evolution**.

Alius CLI itself is a living example of software self-evolution -- its development process heavily leverages AI assistance, demonstrating how human-AI collaboration can accelerate the software development lifecycle.

**Self-Iteration** -- AI-assisted code improvement and feature development. Software is no longer a one-time deliverable, but a continuously evolving entity.

**Adaptive Configuration** -- Intelligent parameter adjustment based on usage scenarios. The tool adapts to the user, not the other way around.

**Tool-based Architecture** -- Modular, extensible tool system design, enabling Agents to invoke external capabilities for complex tasks.

## Features

- **Interactive REPL** -- Slash commands and real-time chat
- **Multi-model Support** -- OpenAI, Anthropic, Gemini and more
- **Flexible Configuration** -- YAML/TOML/JSON config files
- **Beautiful UI** -- ASCII logo and colored output

## Installation

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

Update:

```bash
# npm
npm update -g @alius-tech/alius

# Homebrew
brew update && brew upgrade alius
```

Uninstall:

```bash
# npm
npm uninstall -g @alius-tech/alius

# Homebrew
brew uninstall alius && brew untap AliusTech/tap
```

## Usage

```bash
# Interactive mode
alius

# Run a task directly
alius run -p "Explain what Rust is"

# Show configuration
alius config show
```

## REPL Commands

| Command | Description |
|---------|-------------|
| `/model` | Select model |
| `/config` | Show configuration |
| `/help` | Show help |
| `/quit` | Exit |

## Config File

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

## Supported Models

- OpenAI: `gpt-4o`, `gpt-4o-mini`, `gpt-4-turbo`, `gpt-3.5-turbo`
- Anthropic: `claude-3-5-sonnet`, `claude-3-opus`, `claude-3-haiku`
- Google: `gemini-1.5-pro`, `gemini-1.5-flash`

## Development

```bash
git clone https://github.com/AliusTech/alius.git
cd alius
cargo build --release
```

## License

MIT License - see [LICENSE](LICENSE)
