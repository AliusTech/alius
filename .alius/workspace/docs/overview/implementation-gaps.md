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

MCP server config, connection, and tool listing are implemented. MCP auto-initialization requires all of: (1) `mcp` Cargo feature enabled, (2) `.alius/config/tools.toml` has `registry.mcp_tools = true`, `mcp.load_on_workspace_start = true`, `mcp.register_as_tools = true`, (3) `~/.alius/mcp/servers.toml` exists. When conditions are met, MCP tools register into the shared `ToolRegistry` via `McpToolAdapter` and are visible through `CoreRuntimeManager::tool_list()` and JSON-RPC `tool_list` with `ToolSource::Mcp`. Native/WASM tools take priority on name conflicts. MCP initialization runs in the background and does not block runtime startup.

## Tool Confirmation Flow

Plan mode tool confirmation is implemented end-to-end:

1. When a tool's `preview_confirmation()` returns `true` (e.g., high-risk shell commands, file writes in Plan mode), `tool_step::execute_tools()` emits a `ToolConfirmationRequired` event and blocks on a oneshot channel.

2. The TUI streaming event loop receives this event and displays a confirmation prompt showing:
   - Tool name and tool_call_id
   - Formatted tool arguments (JSON key=value pairs)
   - Approve/Deny choices
   - A detailed confirmation block in the conversation area

3. User input is sent back to the runtime via `ProtocolBridge::respond_confirmation()` → `CoreRuntimeManager::respond_confirmation()` → `CoreRuntime::send()` → `SessionManager::deliver_confirmation()`.

4. The loop engine resumes: approved tools execute normally, denied tools produce `ToolCallCompleted(success=false, denied=true)`.

5. Cancel/drop of the confirmation sender is treated as denial (fail-closed).

6. Failure in confirmation delivery triggers fail-closed: UI shows user-friendly error message with tool_call_id, current run is cancelled to prevent runtime from hanging.

**Audit Logging:**
- Confirmation events logged via `audit::log_confirmation`:
  - `requested` — emitted when confirmation prompt is sent to user
  - `approved` — emitted when user approves the tool
  - `denied_by_user` — emitted when user denies the tool
  - `cancelled` — emitted when run is cancelled while waiting
  - `no_session` — emitted when no session exists (fail-closed)
  - `delivery_failed` — emitted when `respond_confirmation` fails (run not found, no pending confirmation, receiver dropped)
- Audit records include: `run_ref`, `tool_call_id`, `tool_name`, `action`, `trace_id`
- Sensitive arguments are NOT logged (only tool name + call ID)
- Audit failures emit `LogRecordEmitted` diagnostic events (non-blocking)
- `delivery_failed` audit is logged in `runtime.rs` when `SessionManager::deliver_confirmation` returns an error. The run is automatically cancelled after logging to prevent hanging.
- **tool_name sentinel**: When `deliver_confirmation` cannot recover the original tool_name (run not found, no pending confirmation), it returns `"unknown"` as a sentinel value. For receiver-dropped scenarios, the original tool_name from the stored confirmation metadata is preserved.

**Fail-Closed Behavior:**
- No session available → `ConfirmationDecision::Unavailable`, tool not executed
- User denial → tool not executed, `ToolCallCompleted(success=false, denied=true)`
- Channel dropped (cancel) → `ConfirmationDecision::Cancelled`, tool not executed
- Delivery failure → TUI shows error, run is cancelled, tool not executed
- All failures result in the tool NOT being executed (fail-closed)

Tools that trigger confirmation in Plan mode:
- Shell commands (high-risk)
- File write operations
- File edit operations
- MCP tools (all in Plan mode)

Remaining gaps:
- **TUI streaming-path integration test**: While the `ProtocolBridge` streaming acceptance test and unit tests verify the full bridge path and UI state, a test exercising the TUI event loop with actual key input simulation is still missing.
- MCP tool execution via LoopEngine is tested with a fake MCP-source tool registered in the shared ToolRegistry. Real MCP server end-to-end execution (with actual MCP protocol) is not yet tested.
- `~/.alius/mcp/servers.toml` is the runtime config path; `.alius/config/mcp.json` is a legacy/CLI reference not used by the runtime loader.

## WASM Plugin Integration

Plugin management and WASM host code exist. Production ABI, capability policy, sandboxing, and full runtime integration need more hardening.

## Workflow Runtime

Workflow command surfaces and parsing exist. Prompt and tool steps should not be described as a complete automated runtime unless they call the actual model and tool subsystems on the path being documented.

## Google Provider

Google provider code exists, but support should be documented cautiously unless the current provider path is verified for real streaming, model listing, credentials, errors, and tests.

## JSON-RPC Surface

The JSON-RPC package is lightweight and exposes a small method dispatcher. It does not yet represent the full Core Runtime request, command, and event protocol.
