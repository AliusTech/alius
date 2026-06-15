# Implementation Gaps

This document lists known gaps that should not be described as complete features.

## Runtime Manager Boundary

`CoreRuntimeManager` is the target local entrypoint for default execution. CLI, TUI, and JSON-RPC should route conversation, Plan, one-shot run, memory, tool listing, review, and health operations through:

```text
Product Entrypoint
  -> CoreRuntimeManager
  -> ProtocolInterface<CoreRuntime>
  -> CoreRuntime
  -> runtime-config / runtime-model / runtime-tools / runtime-store
```

Temporary product-level subsystem calls are allowed only for bootstrap or administration surfaces:

- `alius init`
- config task UI and config file management
- init/config model discovery and provider setup
- credential management
- plugin install, list, and remove commands

These exceptions should be reduced over time. They must not be used as examples for default dialogue, Plan, run, tool execution, memory, or review architecture.

The TUI `/config` path is now an in-workspace guided task, but saving settings is still an administration operation that may directly update local settings and rebuild the runtime bridge.

The REPL/TUI compatibility path should not retain a separate local `LlmClient`, `AliusAgent`, `ToolRegistry`, or runtime-model conversation container for default execution. Those execution dependencies belong behind `CoreRuntimeManager`; local product history should use protocol-level messages.

## Default Workspace Tooling

`ToolRegistry` exists, and Plan mode can use a registry through Core Runtime paths. All tools are implemented as Rust WASM modules; Core Runtime loads and schedules them but does not implement concrete tool business logic. The user-facing workspace should still be checked before claiming complete tool approval, evidence capture, and policy enforcement.

## Permission Enforcement

Permission types, config views, and Shell Gate modules exist. Enforcement is not yet uniform across every tool and product path.

Documentation must not imply that all shell, process, git, network, and filesystem operations are fully governed by one complete policy layer.

## Agent Team and A2A

Agent Team UI and state concepts are scaffolded, but live Agent Team or AgentNet traffic is not connected by default.

A2A should be documented as an architecture direction and partial config/protocol surface until runtime adapters are implemented and tested.

## MCP Integration

MCP server config, connection, and tool listing are implemented. When the `mcp` feature is enabled and `~/.alius/mcp/servers.toml` exists, MCP tools are registered into the shared `ToolRegistry` via `McpToolAdapter`. They are visible through `CoreRuntimeManager::tool_list()` and JSON-RPC `tool_list` with `ToolSource::Mcp`. Native/WASM tools take priority on name conflicts. MCP initialization runs in the background and does not block runtime startup.

Remaining gaps:
- MCP tool execution via LoopEngine is structurally possible (tools are in the registry) but needs integration testing.
- `~/.alius/mcp/servers.toml` is the only config path; `mcp.json` is not currently used by the runtime loader.

## WASM Plugin Integration

Plugin management and WASM host code exist. Production ABI, capability policy, sandboxing, and full runtime integration need more hardening.

## Workflow Runtime

Workflow command surfaces and parsing exist. Prompt and tool steps should not be described as a complete automated runtime unless they call the actual model and tool subsystems on the path being documented.

## Google Provider

Google provider code exists, but support should be documented cautiously unless the current provider path is verified for real streaming, model listing, credentials, errors, and tests.

## JSON-RPC Surface

The JSON-RPC package is lightweight and exposes a small method dispatcher. It does not yet represent the full Core Runtime request, command, and event protocol.
