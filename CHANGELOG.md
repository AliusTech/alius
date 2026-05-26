# Changelog

All notable changes to this project will be documented in this file.

## [0.1.0] - 2025-05-26

### Added

- Initial release
- Interactive REPL mode with slash commands
  - `/model` - Model selection menu
  - `/config` - Show current configuration
  - `/help` - Display help
  - `/quit` - Exit REPL
- LLM client supporting multiple providers
  - OpenAI (GPT-4, GPT-3.5)
  - Anthropic (Claude 3 series)
  - Google (Gemini 1.5)
- Configuration management
  - Embedded default config
  - User config at `~/.alius/config.toml`
  - Environment variable overrides (`ALIUS__*`)
- CLI commands
  - `alius` / `alius repl` - Interactive mode
  - `alius run -p "prompt"` - Single task execution
  - `alius config show` - Display config
  - `alius version` - Version info
- ASCII Logo with white block design
- GitHub Actions workflow for multi-platform releases
  - Linux x64
  - macOS x64 / ARM64
  - Windows x64

### Technical

- Built with Rust 2021 edition
- Dependencies: clap, tokio, async-openai, dialoguer, inquire