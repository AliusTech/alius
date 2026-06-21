# Config Manager Module

Primary paths:

- `runtime/config/src/config_manager.rs`
- `runtime/config/src/views.rs`
- `runtime/config/src/init_wizard.rs`
- `runtime/config/src/project_init.rs`
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
- Support resumable `/init` state transitions and init-state persistence.
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
- `InitWizard`
- `InitViewModel`
- `InitCommand`

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

The local model library is stored in `.alius/config/providers.toml` under `[[model_library.models]]`. The TUI `/model` flow manages this inventory and is the only standalone TUI flow that fetches remote provider model lists.

Model imports from `/model` and `/init` persist the provider model library immediately after the user imports models. This keeps a later `/config` task aligned with the model pool even when the user leaves and re-enters configuration before a full settings save. Successful `/init` model fetches also write the entered API Key into the active runtime settings so chat readiness checks do not report a missing `api_key` after initialization. Non-model configuration saves must preserve the existing on-disk model library instead of overwriting it with an empty in-memory task state.

Each entry records the concrete provider, Base URL, provider-native model name, display name, legacy reasoning note, and enabled state.

Built-in provider choices are limited to `bigmodel`, `xiaomi_mimo`, and `deepseek`. Each supports OpenAI-compatible and Anthropic-compatible Base URLs; Xiaomi MiMo offers both China and Singapore region URLs for each API mode. `/model` records the selected exact protocol and region URL on the imported model entry so `/config` assignments can restore the correct runtime provider mode.

`.alius/config/model.toml` stores the assignment:

- `Plan Model`
- `Execute Model`
- `Review Model`

The `/config` flow assigns each role from enabled model-pool entries only. `/model` model pool management exposes the same assignment path for request-readiness repair. Neither flow accepts manual model names, Base URLs, or API keys in the assignment step.

Deleting a model from `/model` is allowed even when `model.toml` still references that entry. The assignment is not silently cleared or rewritten during deletion. Runtime request entrypoints must validate the three assignments immediately before starting a model request: every role must be configured, the referenced model id must exist in the model library, and the entry must be enabled. TUI request paths stop and open model pool management when validation fails; legacy REPL paths print the same issue list and direct the user to `/model`.

Compatibility writes are still maintained:

- `Plan Model` updates `tiers.light`.
- `Execute Model` updates `tiers.medium` and the active legacy model fields in settings.
- `Review Model` updates `tiers.high` and `Settings.llm.review_model`.

If `model.toml` is missing, loading migrates assignments from the legacy tiers by matching them to model-library entries.

## Initialization

`runtime/config/src/init_wizard.rs` owns pure `/init` state, transition, recovery, and view-model logic. It does not perform filesystem IO, network calls, model requests, role activation, or config writes.

`runtime/config/src/project_init.rs` owns the retryable local filesystem effects used by `/init`:

- `.alius/config/`
- `.alius/config/model.toml`
- `.alius/memory/`
- `.alius/runtime/init-state.toml`
- `.alius/workspace/`

The CLI TUI adapter executes `InitCommand` values, saves state after successful transitions, clears state after completion/cancel, and keeps provider model-list fetching in `entrypoints/cli` through `runtime-model`. The user-facing `/init` flow completes immediately after role configuration is saved; capability resolution, workspace template creation, final validation, and Enter Copilot confirmation are not performed during initialization. The workspace defaults to Copilot mode, while Team mode is switched by a separate workspace operation.

Fresh `/init` starts from a clean wizard context and does not prefill language, role, model pool, or Plan/Execute/Review assignment from the currently hydrated runtime settings. Reinitializing project defaults clears `.alius/config/model.toml`, clears the model library, clears the selected role in `.alius/config/soul.toml`, and does not preserve the previous UI locale. Existing init-state is still resumable from `.alius/runtime/init-state.toml`.

`runtime/core/src/config.rs` still provides legacy-compatible embedded default helpers for older init entrypoints. It does not generate this full documentation set.
