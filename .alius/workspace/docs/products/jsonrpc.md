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

| Method | Current result |
| --- | --- |
| `health_check` | `dispatch` returns `{"status": "ok"}`; `dispatch_with_runtime` delegates to `CoreRuntimeManager::health_check`. |
| `config_read` | `dispatch` returns a compatibility provider/model shape; `dispatch_with_runtime` delegates to `CoreRuntimeManager::config_read`. |
| `version` | Returns the package version. |

Unknown methods return JSON-RPC error code `-32601`.

## Runtime Maturity

This package depends on `core-runtime` and `protocol-interface`. The compatibility `dispatch` function remains lightweight for existing callers and tests. New runtime-backed calls should use `dispatch_with_runtime` so JSON-RPC enters Core Runtime through the same local manager boundary as CLI and TUI.

Do not document JSON-RPC as a full remote Core Runtime API until it maps real `CoreRequest`, `CoreCommand`, and `CoreEvent` semantics.

## Transport

The server binds a TCP listener, reads one newline-terminated JSON request, dispatches it, and writes one newline-terminated JSON response.

Default address behavior should be checked in the caller before documenting a user-facing default port as a stable contract.
