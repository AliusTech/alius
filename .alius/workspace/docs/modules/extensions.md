# Extension Systems

This document covers extension-related systems and their current maturity.

## Soul and Agent Card

Related code lives mainly under:

- `entrypoints/cli/src/formula/`
- `runtime/config/src/agent_card.rs`
- `runtime/config/src/soul.rs`
- `runtime/config/src/soul_source.rs`
- `extensions/souls/` (bundled official souls)

Current model:

- Official souls are bundled in the main repository under `extensions/souls/`. The `alius soul update` command syncs from this bundled path — no network access is required.
- A legacy git clone fallback to `alius-souls.git` still exists for cases where the bundled path is not found (e.g. custom builds). The old remote constants (`OFFICIAL_REMOTE`, `OFFICIAL_HTTPS_REMOTE`) are deprecated and kept only for backward compatibility.
- Project Agent Card compatible config is represented through `.alius/config/soul.toml`.

### Legacy: `alius core update`

`alius core update` clones or fetches from the legacy `alius-souls.git` remote. It is kept for backward compatibility but is **no longer the official extension path**. Prefer `alius soul update`, which reads from the bundled `extensions/souls/` directory without hitting the network.

## Extension Registry

Related file: `extensions/registry.toml`

The extension registry is a TOML file that describes all official extensions bundled with the CLI. Each entry declares:

| Field | Description |
| --- | --- |
| `id` | Unique extension identifier (e.g. `backend-engineer`, `hello-world`) |
| `type` | Extension type: `soul` or `wasm_plugin` |
| `path` | Path relative to the `extensions/` directory |
| `version` | Semver version string |
| `description` | Human-readable description |

Current registry entries:

- **Souls**: `backend-engineer`, `embedded-engineer`, `frontend-engineer`, `motion-control-engineer`, `ops-engineer`, `personal-tutor`
- **WASM Plugins**: `hello-world` (example plugin demonstrating the plugin ABI)

## MCP

Related code lives under:

- `entrypoints/cli/src/mcp/`
- `~/.alius/mcp/servers.toml` (user-level MCP server declarations)

### MCP Config Paths

| Purpose | Path | Description |
| --- | --- | --- |
| Project switch | `.alius/config/tools.toml` | Controls whether MCP tools load on workspace start. Settings: `registry.mcp_tools`, `mcp.load_on_workspace_start`, `mcp.register_as_tools`. All three must be `true` for MCP auto-init. |
| Server declarations | `~/.alius/mcp/servers.toml` | User-level MCP server definitions. Loaded by `McpManager` at runtime when the project switch is enabled. |
| Legacy path | `.alius/config/mcp.json` | Historical reference in `tools.toml` default. Not used by the current runtime loader. May be used by CLI tooling. |

### Current maturity:

- CLI management exists.
- Server listing, start, and tool listing behavior exists.
- MCP tools enter the shared `ToolRegistry` when project switch is enabled and `~/.alius/mcp/servers.toml` exists.
- MCP initialization runs in background and does not block runtime startup.
- Native/WASM tools take priority — MCP tools with duplicate names are skipped.

## WASM Plugin

Related code lives under:

- `entrypoints/cli/src/plugin/`
- `runtime/tools/src/wasm_host/`

Current maturity:

- CLI management exists.
- Runtime tool registration code exists.
- ABI, sandbox, and permission model need more hardening before production claims.

## WASM Host Imports

Related code: `runtime/tools/src/wasm_host/imports.rs`

Six host functions are registered under the `"alius_host"` Wasmtime namespace. Every call follows the same pipeline: parse WASM memory parameters (JSON) -> permission matcher check -> domain security primitive -> audit log -> execute or return denial.

| Import | Description | Security model |
| --- | --- | --- |
| `read_file` | Read a file relative to workspace | Workspace boundary enforcement; paths must not escape the workspace root |
| `write_file` | Write a file relative to workspace | Workspace boundary enforcement |
| `list_dir` | List directory entries relative to workspace | Workspace boundary enforcement |
| `env_get` | Read an environment variable | Env var name validation; values are never logged in audit |
| `shell` | Execute a shell command | Permission matcher gate; stdout/stderr are never logged in audit |
| `fetch` | HTTP fetch | Deny-by-default; permission check runs but real HTTP execution is **not yet implemented** |

Each import returns a packed `(ptr, len)` i64 to WASM memory containing a JSON response with `{ok: true, data}` or `{ok: false, error, code}`.

Permissions are declared per-plugin in `plugin.toml` via the `PluginPermissions` struct (domains: `filesystem`, `network`, `shell`, `env`). The `ResolvedPluginPermissions` type is checked at runtime for every host call.

## Host Audit Sink

Related code: `runtime/tools/src/wasm_host/audit.rs`

Every WASM host function call (allow or deny) emits a `HostAuditEvent` through the `HostAuditSink` trait.

**Security invariants:**
- File content, env values, and shell stdout/stderr are **never** recorded in audit events.
- Sensitive arguments (passwords, tokens) are redacted before reaching the sink.
- Sink failures are non-blocking: a diagnostic is logged but execution continues (deny/allow is determined by the permission matcher, not the audit sink).

`HostAuditEvent` fields:

| Field | Description |
| --- | --- |
| `trace_id` | Links the call to a broader execution context |
| `plugin_id` | Plugin that initiated the call |
| `action` | Host function name (`read_file`, `write_file`, `list_dir`, `env_get`, `shell`, `fetch`) |
| `target` | Resource path, URL, variable name, or command base |
| `allowed` | Whether the operation was allowed |
| `reason` | Deny reason, or `"ok"` for allowed calls |
| `ts` | Unix timestamp in milliseconds |

Implementations:

- `TracingAuditSink` — default; logs via `tracing::info!` (allow) / `tracing::warn!` (deny)
- `NoopAuditSink` — no-op, used in tests
- Custom sinks can be injected via `WasmHostState::with_audit()`

A separate runtime-level audit module exists at `runtime/core/src/logging/audit.rs`. It logs permission decisions, tool invocations, Shell Gate decisions, and confirmation events to an append-only `event-log.jsonl` file. It does not contain raw arguments or sensitive content.

## Workflow

Related code lives under:

- `entrypoints/cli/src/workflow/`

Current maturity:

- CLI command surface and parsing exist.
- `LoopEngineHandle` trait provides the integration surface for runtime calls.
- Prompt/tool/condition step execution through the trait.
- Condition step with `contains`, `success`, `failed` operators.
- `StubLoopEngineHandle` for testing; 7 unit tests.
- **CLI `workflow run` uses `StubLoopEngineHandle`** — it does NOT call the real `CoreRuntimeManager` or `LoopEngine`. Workflow steps are scaffold, not a complete automation engine.

## Agent Team and A2A

Related code and state concepts appear in TUI workspace and config/protocol surfaces.

Current maturity:

- Agent Team UI concepts exist.
- A2A is an architecture direction.
- Live Agent Team or AgentNet traffic is not connected by default.
