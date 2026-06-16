# Extension Systems

This document covers extension-related systems and their current maturity.

## Soul and Agent Card

Related code lives mainly under:

- `entrypoints/cli/src/formula/`
- `runtime/config/src/agent_card.rs`
- `runtime/config/src/soul.rs`
- `runtime/config/src/soul_source.rs`

Current model:

- Official soul repository cache is managed separately from project Agent Card config.
- Project Agent Card compatible config is represented through `.alius/config/soul.toml`.
- `alius soul update` syncs installed soul cache entries.
- `alius core update` updates the official repository cache.

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

## Workflow

Related code lives under:

- `entrypoints/cli/src/workflow/`

Current maturity:

- CLI command surface exists.
- Workflow parsing and command handling exist.
- Prompt and tool step execution should not be documented as a complete automation runtime unless it calls real model and tool systems on the described path.

## Agent Team and A2A

Related code and state concepts appear in TUI workspace and config/protocol surfaces.

Current maturity:

- Agent Team UI concepts exist.
- A2A is an architecture direction.
- Live Agent Team or AgentNet traffic is not connected by default.

