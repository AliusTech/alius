# Config Manager Module

Primary paths:

- `runtime/config/src/config_manager.rs`
- `runtime/config/src/views.rs`
- `runtime/config/src/loaders/`
- `runtime/config/src/settings.rs`
- `runtime/config/src/merger.rs`
- `runtime/config/src/migration.rs`
- `runtime/core/src/config.rs`

## Responsibilities

- Find project root.
- Load project config files.
- Build `ProjectConfigSnapshot`.
- Resolve `RuntimeConfigView`.
- Expose schema views for provider, tool, permission, protocol, soul, logging, and session config.
- Support project initialization defaults.
- Support legacy config migration where implemented.

## Main Types

- `Settings`
- `ProjectConfigSnapshot`
- `RuntimeConfigView`
- `ResolvedProviderConfig`
- `ResolvedToolConfig`
- `ResolvedPermissionConfig`
- `ResolvedSoulConfig`
- `ShellGateConfig`
- `LoggingConfig`
- `SessionConfig`

## Project Config Root

Preferred project config root:

```text
.alius/config/
```

### Project config files (`.alius/config/`)

- `config.toml`
- `providers.toml`
  - provider definitions
  - compatibility tier routing
  - project-local model library entries
- `model.toml`
  - Plan/Execute/Review model assignment
- `soul.toml`
- `tools.toml` — includes MCP project switch: `registry.mcp_tools`, `mcp.load_on_workspace_start`, `mcp.register_as_tools`
- `permissions.toml`
- `protocol.toml`

### User-level config files (`~/.alius/`)

- `~/.alius/mcp/servers.toml` — MCP server declarations. Loaded by `McpManager` when the project switch in `tools.toml` is enabled.
- `.alius/config/mcp.json` — legacy MCP config path referenced in `tools.toml` defaults. Not used by the current runtime loader.

## Local Model Library

`runtime/config/src/views.rs` defines `ModelLibraryConfig`, `ModelLibraryEntry`, `ModelAssignmentConfig`, and `ModelAssignmentRole`.

The local model library is stored in `.alius/config/providers.toml` under `[[model_library.models]]`. The TUI `/model` flow manages this inventory and is the only TUI flow that fetches remote provider model lists.

Each entry records the concrete provider, Base URL, provider-native model name, display name, legacy reasoning note, and enabled state.

Built-in provider choices are limited to `bigmodel`, `xiaomi_mimo`, and `deepseek`. Each supports OpenAI-compatible and Anthropic-compatible Base URLs; `/model` records the selected protocol URL on the imported model entry so `/config` assignments can restore the correct runtime provider mode.

`.alius/config/model.toml` stores the assignment:

- `Plan Model`
- `Execute Model`
- `Review Model`

The `/config` flow assigns each role from enabled model-pool entries only. It does not accept manual model names, Base URLs, or API keys in the assignment flow.

Compatibility writes are still maintained:

- `Plan Model` updates `tiers.light`.
- `Execute Model` updates `tiers.medium` and the active legacy model fields in settings.
- `Review Model` updates `tiers.high` and `Settings.llm.review_model`.

If `model.toml` is missing, loading migrates assignments from the legacy tiers by matching them to model-library entries.

## Initialization

`runtime/core/src/config.rs` writes embedded defaults and ensures:

- `.alius/config/`
- `.alius/config/model.toml`
- `.alius/memory/`
- `.alius/workspace/`

It does not generate this full documentation set.
