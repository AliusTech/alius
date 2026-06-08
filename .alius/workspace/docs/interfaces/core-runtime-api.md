# Core Runtime API

`CoreRuntimeApi` is the trait that Core Runtime implements for protocol callers.

Current definition lives in `protocol/src/core.rs`; the primary implementation lives in `runtime/core/src/runtime.rs`.

`CoreRuntimeManager` lives in `runtime/core/src/manager.rs`. It is the local Runtime Manager facade used by product entrypoints, but it does not replace this trait. The manager assembles a local `CoreRuntime`, holds `ProtocolInterface<CoreRuntime>`, and calls the same API through the protocol boundary.

## Execution Methods

| Method | Purpose |
| --- | --- |
| `start` | Start a request and return a `RunRef`. |
| `send` | Send a command to an existing run. |
| `subscribe` | Return events for a run. |
| `start_streaming` | Start execution and return a run ref with a `CoreEvent` receiver. |

## Config Methods

| Method | Purpose |
| --- | --- |
| `config_read` | Read current runtime config snapshot. |
| `config_update` | Update a configuration key. |

## Model Methods

| Method | Purpose |
| --- | --- |
| `model_list` | List available models from the current provider path. |

## Session Methods

| Method | Purpose |
| --- | --- |
| `open_session` | Open a session. |
| `list_sessions` | List sessions. |
| `inspect_session` | Inspect a session. |
| `close_session` | Close a session. |
| `clear_conversation` | Clear conversation history for a session. |

## Memory Methods

| Method | Purpose |
| --- | --- |
| `memory_save` | Save a memory entry. |
| `memory_list` | List memory entries. |
| `memory_clear` | Clear memory entries. |

## Tool and Review Methods

| Method | Purpose |
| --- | --- |
| `tool_list` | List known tools. |
| `review_start` | Start a review run for a session. |

## Health and Logs

| Method | Purpose |
| --- | --- |
| `health_check` | Return runtime health status. |
| `query_logs` | Query runtime log records. |

## Implementation Notes

`CoreRuntime` currently builds a runtime from:

- workspace reference
- settings
- `LlmClient`
- optional `ToolRegistry`
- session manager
- project and global memory stores when available
- conversation store

Runtime API methods should return protocol-level errors, not product-specific UI errors.

## Runtime Manager Facade

Product entrypoints should prefer the manager methods instead of assembling model and tool execution dependencies directly:

| Method | Purpose |
| --- | --- |
| `new_local` | Build a local CLI manager from workspace root and settings. |
| `new_local_tui` | Build a local TUI manager from workspace root and settings. |
| `from_runtime` | Wrap an existing runtime for tests or integration code. |
| `run_text` | Execute text and collect protocol event envelopes. |
| `start_streaming` | Execute text and return a run reference with a `CoreEvent` receiver. |
| `config_read` | Read runtime configuration through the protocol boundary. |
| `model_list` | List models through the managed runtime path. |
| `memory_save`, `memory_list`, `memory_clear` | Manage runtime memory through Core Runtime. |
| `tool_list` | List tools through Core Runtime. |
| `review_start` | Start review through Core Runtime. |
| `health_check` | Read runtime health through Core Runtime. |

The manager is local process infrastructure. Remote or serialized callers should still be modeled in protocol terms: request, command, event, origin, and capability scope.
