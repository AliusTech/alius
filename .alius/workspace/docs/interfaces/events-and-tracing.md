# Events and Tracing

Alius represents long-running execution through Core events and trace ids.

## Identifiers

The protocol layer defines these id wrappers:

- `RequestId`
- `CommandId`
- `EventId`
- `TraceId`
- `SessionRef`
- `TurnRef`
- `RunRef`

Each run should have a trace id that connects request handling, event emission, logs, tool calls, and final result reporting.

## CoreEvent Structure

`CoreEvent` carries:

- `event_id`
- `trace_id`
- `session_ref`
- `turn_ref`
- `run_ref`
- `sequence`
- `kind`
- `payload`
- `created_at`

`sequence` is scoped to a run event stream.

## Event Kinds

Current event kinds include:

- `RunStarted`
- `RunCompleted`
- `RunCancelled`
- `LoopIterationStarted`
- `LoopIterationCompleted`
- `ModelRequestStarted`
- `ModelDelta`
- `ModelCompleted`
- `ToolCallStarted`
- `ToolCallCompleted`
- `ConvergenceChecked`
- `ApprovalRequested`
- `UserInputRequested`
- `ErrorRaised`
- `FinalResult`
- `LogRecord`

## Event Payloads

Payloads can carry:

- text deltas
- JSON values
- final content and success state
- error code and message
- log records

## TUI Direction

The TUI should increasingly reduce UI state from Core events instead of inventing parallel product-only execution state.

Current documentation must still distinguish this direction from paths that are not yet fully event-driven.

## Logging Direction

Logging modules exist under `runtime/core/src/logging`. Logs should eventually use workspace, session, run, and trace identity consistently.

Do not claim complete persisted trace coverage on every runtime path unless verified in code and tests.

