# Session Manager Module

Primary path:

- `runtime/core/src/session.rs`

## Responsibilities

- Track workspace-scoped sessions.
- Create sessions.
- Create turns and runs.
- Generate `TurnRef`, `RunRef`, and `TraceId`.
- Store run events in memory.
- Update run status.
- Close sessions.
- Return session and run summaries.

## Main Type

- `SessionManager`

## Lifecycle

```text
create_session
  -> create_turn
  -> push_event
  -> update_run_status
  -> get_events or subscribe through CoreRuntimeApi
```

## State Model

The current `SessionManager` stores sessions and run events in process memory. Persistence and cross-process recovery should be verified in store modules before being documented as complete.

## Boundary

The Session Manager should not know TUI layout, provider-specific model behavior, or tool implementation details.

