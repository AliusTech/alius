# @aliustech/alius

**Alius CLI** - An AI-powered assistant with intelligent tool calling capabilities.

[![npm version](https://img.shields.io/npm/v/@aliustech/alius.svg)](https://www.npmjs.com/package/@aliustech/alius)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![GitHub](https://img.shields.io/badge/GitHub-AliusTech/alius-black.svg)](https://github.com/AliusTech/alius)

## Installation

```bash
# Install globally via npm
npm install -g @aliustech/alius

# Or using yarn
yarn global add @aliustech/alius

# Or using pnpm
pnpm add -g @aliustech/alius
```

## Quick Start

```bash
# Start interactive REPL mode
alius

# Run a single prompt
alius run "What files are in this directory?"

# Show help
alius --help

# Show version
alius version
```

## Features

- 🤖 **AI-Powered Assistant** - Powered by LLM models (OpenAI GPT, Anthropic Claude)
- 🔧 **Built-in Tools** - File operations, shell commands, git, HTTP requests, and more
- 💬 **Interactive REPL** - Conversation-style interface with session management
- 🔐 **Permission System** - Granular control over tool execution permissions
- 📝 **Session Persistence** - Save and resume conversations
- 🎯 **Smart Tool Calling** - Automatic tool selection based on context

## Built-in Tools

| Tool | Description |
|------|-------------|
| `read_file` | Read file contents |
| `write_file` | Write/create files |
| `edit_file` | Edit existing files |
| `list_dir` | List directory contents |
| `shell` | Execute shell commands |
| `git_status` | Check git repository status |
| `git_diff` | View git differences |
| `http_request` | Make HTTP requests |
| `json` | Parse and query JSON data |
| `code_stats` | Analyze code statistics |

## Configuration

Create a config file at `~/.alius/config.toml`:

```toml
[llm]
provider = "openai"  # or "anthropic"
model = "gpt-4o-mini"  # or "claude-3-haiku-20240307"

[ui]
theme = "dark"
```

## Supported Platforms

| Platform | Architecture | Package |
|----------|-------------|---------|
| macOS | Intel x64 | `@aliustech/alius-darwin-x64` |
| macOS | Apple Silicon arm64 | `@aliustech/alius-darwin-arm64` |
| Linux | x64 | `@aliustech/alius-linux-x64` |
| Linux | arm64 | `@aliustech/alius-linux-arm64` |
| Windows | x64 | `@aliustech/alius-win32-x64` |
| Windows | arm64 | `@aliustech/alius-win32-arm64` |

The correct platform package is automatically installed as an optional dependency.

## Alternative Installation Methods

### Homebrew (macOS)

```bash
brew tap AliusTech/alius
brew install alius
```

### Direct Download

Download pre-built binaries from [GitHub Releases](https://github.com/AliusTech/alius/releases):

```bash
# macOS/Linux
curl -sSL https://github.com/AliusTech/alius/releases/latest/download/alius-$(uname -s)-$(uname -m).tar.gz | tar xz

# Windows (PowerShell)
Invoke-WebRequest -Uri "https://github.com/AliusTech/alius/releases/latest/download/alius-windows-x64.zip" -OutFile "alius.zip"
Expand-Archive alius.zip
```

### From Source

```bash
git clone https://github.com/AliusTech/alius.git
cd alius
cargo build --release
cargo install --path crates/alius-cli
```

## API Keys

Alius requires API keys to communicate with LLM providers:

- **OpenAI**: Set `OPENAI_API_KEY` environment variable or configure in config file
- **Anthropic**: Set `ANTHROPIC_API_KEY` environment variable or configure in config file

```bash
export OPENAI_API_KEY="your-api-key"
alius
```

## Requirements

- Node.js >= 16 (for npm installation)
- Rust >= 1.70 (for building from source)

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](https://github.com/AliusTech/alius/blob/main/CONTRIBUTING.md).

## License

MIT License - see [LICENSE](https://github.com/AliusTech/alius/blob/main/LICENSE).

## Links

- [GitHub Repository](https://github.com/AliusTech/alius)
- [Issue Tracker](https://github.com/AliusTech/alius/issues)
- [Changelog](https://github.com/AliusTech/alius/blob/main/CHANGELOG.md)

---

Made with ❤️ by **Alius Tech**