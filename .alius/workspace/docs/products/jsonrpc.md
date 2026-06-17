# JSON-RPC Entrypoint

The `jsonrpc` package lives at `entrypoints/jsonrpc`. It currently provides a lightweight JSON-RPC 2.0 request dispatcher and TCP line server.

## Current API Shape

Main types:

- `JsonRpcRequest`
- `JsonRpcResponse`
- `JsonRpcError`

Main functions:

- `dispatch(request: &JsonRpcRequest) -> JsonRpcResponse`
- `dispatch_with_runtime(request: &JsonRpcRequest, manager: &CoreRuntimeManager) -> JsonRpcResponse`
- `serve(addr: SocketAddr) -> Result<()>`

## Implemented Methods

All methods below are dispatched through `dispatch_with_runtime` backed by `CoreRuntimeManager`. The legacy `dispatch` stub only supports `health_check`, `config_read`, and `version` for backward-compat tests.

| Method | Description | Params | Returns |
| --- | --- | --- | --- |
| `health_check` | Delegates to `CoreRuntimeManager::health_check()`. | — | `{"workspace_ok": bool, ...}` |
| `config_read` | Returns real runtime configuration. | — | `{"provider": "...", "model": "...", ...}` |
| `model_list` | Returns the model library via `CoreRuntimeManager::model_list()`. | — | `[{"id": "...", ...}]` |
| `tool_list` | Returns registered tools via `CoreRuntimeManager::tool_list()`. | — | `[{"name": "...", ...}]` |
| `version` | Returns the package version. | — | `{"version": "..."}` |
| `run_start` | Starts a streaming run. Returns correlation IDs for subsequent subscribe/cancel. | `{"text": "...", "mode": "Chat"|"Plan"}` | `{"run_ref": "...", "trace_id": "...", "session_ref": "..."}` |
| `run_subscribe` | Returns a snapshot of events for a run. Not a push/long-poll subscription. | `{"run_ref": "..."}` | `{"events": [{event with trace_id, run_ref, session_ref, turn_ref, kind, payload, sequence, created_at}]}` |
| `run_cancel` | Cancels a running execution. | `{"run_ref": "...", "reason": "optional"}` | `{"success": true}` |
| `run_confirm_tool` | Responds to a tool confirmation request. | `{"run_ref": "...", "tool_call_id": "...", "approved": bool}` | `{"success": true}` |

### Error Codes

| Code | Meaning |
| --- | --- |
| `-32601` | Method not found |
| `-32602` | Invalid params (missing required fields, wrong types) |
| `-32000` | Runtime / internal error |

### Run Control Semantics

- `run_start` returns immediately with `run_ref` and correlation IDs. The run executes asynchronously.
- `run_subscribe` returns a point-in-time event snapshot. It does not push events or long-poll.
- `run_cancel` triggers runtime cancellation. After cancellation, `run_subscribe` will show `RunCancelled` or terminal `FinalResult` events.
- `run_confirm_tool` responds to a `ToolConfirmationRequired` event. The `tool_call_id` must match the one from the event payload. `approved: true` resumes execution; `approved: false` denies and fails the batch.
- No server push, continuous subscription, or long-lived connections are supported.

## Runtime Maturity

This package depends on `core-runtime` and `protocol-interface`. The compatibility `dispatch` function remains lightweight for existing callers and tests. New runtime-backed calls should use `dispatch_with_runtime` so JSON-RPC enters Core Runtime through the same local manager boundary as CLI and TUI.

Do not document JSON-RPC as a full remote Core Runtime API until it maps real `CoreRequest`, `CoreCommand`, and `CoreEvent` semantics.

## Transport

The server binds a TCP listener, reads one newline-terminated JSON request, dispatches it, and writes one newline-terminated JSON response.

Default address behavior should be checked in the caller before documenting a user-facing default port as a stable contract.
