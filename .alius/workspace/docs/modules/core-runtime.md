# Core Runtime Module

Primary paths:

- `runtime/core/src/runtime.rs`
- `runtime/core/src/manager.rs`
- `runtime/core/src/session.rs`
- `runtime/core/src/loop_engine/`
- `runtime/core/src/event_adapter.rs`
- `runtime/core/src/config.rs`
- `runtime/core/src/logging/`
- `runtime/core/src/patch/`

## Responsibilities

- Implement `CoreRuntimeApi`.
- Export `CoreRuntimeManager` as the local Runtime Manager facade for product entrypoints.
- Build runtime state from settings, model client, workspace root, and optional tool registry.
- Manage sessions, turns, runs, and trace ids.
- Convert request input into loop input.
- Run Chat and Plan modes through `LoopEngine`.
- Store run events through `SessionManager`.
- Expose config, model, memory, tool, review, health, and log operations.

## Main Types

- `CoreRuntime`
- `CoreRuntimeManager`
- `RuntimeManagerContext`
- `CoreRuntimeBuilder`
- `SessionManager`
- `LoopContext`
- `LoopEngine`
- `EventAdapter`

## Main Chain

```text
CLI / TUI / JSON-RPC
  -> CoreRuntimeManager
  -> ProtocolInterface<CoreRuntime>
  -> CoreRuntime::start or start_streaming
  -> SessionManager
  -> LoopEngine
  -> LlmClient
  -> optional ToolRegistry
```

## Runtime State

`CoreRuntime` owns:

- `SessionManager`
- `Settings`
- `LlmClient`
- active run map
- global memory store when available
- project memory store when available
- optional tool registry
- conversation store

`CoreRuntimeManager` owns:

- `ProtocolInterface<CoreRuntime>`
- workspace root
- origin and capability context for local callers
- assembly of `LlmClient`
- assembly of `ToolRegistry`
- construction of `CoreRuntime` through `CoreRuntimeBuilder`

## Public Facade

The manager exposes local product-level methods while preserving protocol semantics:

- `new_local`
- `new_local_tui`
- `new_with_context`
- `from_runtime`
- `from_runtime_with_context`
- `run_text`
- `start_streaming`
- `subscribe`
- `config_read`
- `config_update`
- `model_list`
- `close_session`
- `clear_conversation`
- `memory_save`
- `memory_list`
- `memory_clear`
- `tool_list`
- `review_start`
- `query_logs`
- `health_check`

Product entrypoints should use this facade for default conversation, Plan, run, memory, tool listing, review, and health operations.

## Known Gaps

- Some logging and trace structures are present but should be verified before being described as complete persisted observability.
- Tool approval and permission enforcement need careful path-specific documentation.
