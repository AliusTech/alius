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

Current maturity:

- CLI management exists.
- Server listing, start, and tool listing behavior exists.
- MCP tools should not be described as fully connected to the default workspace tool loop unless that path is verified.

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

