# Config Schema

Project configuration lives under `.alius/config/`.

Current configuration modules live under `runtime/config`.

## Project Files

| File | Purpose |
| --- | --- |
| `config.toml` | Project runtime metadata, default model/provider, UI, session, logging basics. |
| `providers.toml` | Provider definitions, compatibility routing tiers, local model library, Base URLs, and API key env names. |
| `model.toml` | Plan/Execute/Review model assignment backed by enabled local model-library entries. |
| `soul.toml` | Project Agent Card compatible configuration. |
| `tools.toml` | Built-in, MCP project switch (`registry.mcp_tools`, `mcp.load_on_workspace_start`, `mcp.register_as_tools`), plugin, workflow, timeout, confirmation, and registry settings. |
| `permissions.toml` | Filesystem, shell, network, memory, project docs, and remote A2A permissions. |
| `protocol.toml` | Protocol interface enablement and event/command settings. |

### User-level config files (`~/.alius/`)

| File | Description |
| --- | --- |
| `~/.alius/mcp/servers.toml` | MCP server declarations. Loaded by `McpManager` when the project switch in `.alius/config/tools.toml` is enabled. |

`/init` progress is not a project config file. It is persisted separately at `.alius/runtime/init-state.toml` so initialization can resume without mixing transient wizard state into stable configuration schema files.

## Main Runtime Views

| Type | Purpose |
| --- | --- |
| `ProjectConfigSnapshot` | Loaded project configuration view from config files. |
| `ModelAssignmentConfig` | Project Plan/Execute/Review model assignment from `model.toml`. |
| `RuntimeConfigView` | Resolved runtime view built from a snapshot and workspace root. |
| `ResolvedProviderConfig` | Provider list and active routing information. |
| `ResolvedToolConfig` | Enabled tool families and runtime tool settings. |
| `ResolvedPermissionConfig` | Resolved permission booleans for runtime use. |
| `ResolvedSoulConfig` | Resolved Agent Card and soul settings. |
| `ShellGateConfig` | Shell command policy view. |
| `LoggingConfig` | Logging path, level, and redaction view. |
| `SessionConfig` | Session directory and retention view. |

## Loading

`load_project_config(cwd)`:

1. Finds the project root by searching upward for `.alius/`.
2. Loads files from `.alius/config/`.
3. Uses defaults when specific files are missing.
4. Returns `ProjectConfigSnapshot`.

`build_runtime_config(snapshot, workspace_root)` resolves provider, tool, permission, shell gate, logging, session, and soul views.

## Provider And Model Inventory

`providers.toml` keeps provider definitions, compatibility tier routing, and the project-local model library together. `/model` owns this model pool.

The built-in provider catalog is intentionally limited to:

| Provider key | Product label | OpenAI-compatible Base URL | Anthropic-compatible Base URL |
| --- | --- | --- | --- |
| `bigmodel` | `BigModel GLM (Coding Plan)` | `https://open.bigmodel.cn/api/coding/paas/v4` | `https://open.bigmodel.cn/api/anthropic` |
| `xiaomi_mimo` | `Xiaomi MiMo (Token Plan)` | `https://api.xiaomimimo.com/v1` | `https://api.xiaomimimo.com/anthropic` |
| `deepseek` | `DeepSeek` | `https://api.deepseek.com` | `https://api.deepseek.com/anthropic` |

The local provider entry stores the last selected Base URL and a compatible `kind`; each imported model-library entry keeps the exact Base URL chosen during `/model`.

Current top-level sections:

| Section | Purpose |
| --- | --- |
| `[router]` | Routing strategy and default/fallback tier names. |
| `[tiers.light]` | Compatibility router target synchronized from `Plan Model`. |
| `[tiers.medium]` | Compatibility router target synchronized from `Execute Model`. |
| `[tiers.high]` | Compatibility router target synchronized from `Review Model`. |
| `[providers.<name>]` | Provider kind, Base URL, enablement, and API key environment source. |
| `[[model_library.models]]` | Concrete project-local model entries managed by `/model` and assigned by `/config`. |

Each model-library entry contains:

| Field | Meaning |
| --- | --- |
| `id` | Stable entry id, normally derived from provider and model name. |
| `display_name` | Human-facing model label. |
| `provider` | Provider key matching `[providers.<name>]`. |
| `base_url` | Base URL captured for this model entry. |
| `model_name` | Provider-native model id sent to the runtime. |
| `reasoning_note` | Legacy compatibility note retained on model-pool entries. |
| `enabled` | Whether the entry can be selected. |

## Model Assignment

`model.toml` stores the user-facing three-line architecture:

| Field | Meaning |
| --- | --- |
| `schema_version` | Model assignment schema version. |
| `[assignment].plan` | Model-library entry id selected for `Plan Model`. |
| `[assignment].execute` | Model-library entry id selected for `Execute Model`. |
| `[assignment].review` | Model-library entry id selected for `Review Model`. |

`/config` only assigns these fields from enabled entries in the local model pool. It does not allow manual model-name, Base URL, or API key entry in the assignment flow.

For compatibility, saving assignments also updates existing fields:

| Assignment | Compatibility write |
| --- | --- |
| `Plan Model` | `providers.toml` `tiers.light` |
| `Execute Model` | `providers.toml` `tiers.medium`, `Settings.llm.model`, provider, and Base URL |
| `Review Model` | `providers.toml` `tiers.high`, `Settings.llm.review_model` |

If `model.toml` is missing, the config loader migrates the assignment from legacy `tiers.light`, `tiers.medium`, and `tiers.high` by matching enabled model-library entries.

Entering `/config` reads the local model library only. It does not call provider model-list APIs. Remote fetching is only part of the explicit Add Model flow in `/model`. API Key input in that flow is plaintext and supports paste, while saved keys are not shown in model details.

## Initialization

`core_runtime::config::reset_project_config` writes default config files, ensures memory directories, and ensures `.alius/workspace/` exists.

It does not generate this full documentation tree.

## Compatibility

Older flat paths may still be read or migrated by some modules. New documentation and new project writes should prefer `.alius/config/`.
