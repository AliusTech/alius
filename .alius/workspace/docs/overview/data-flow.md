# Data Flow

Alius data flow is centered on project-local state under `.alius/` and user-level caches under `~/.alius/`.

## Project State

```text
.alius/
  config/
  memory/
  workspace/
```

| Area | Purpose |
| --- | --- |
| `.alius/config/` | Project runtime configuration. |
| `.alius/memory/` | Runtime memory, sessions, logs, and communication records. |
| `.alius/workspace/` | Project documentation and design source. |

## User State

`~/.alius/` is used for user-level state such as global config, global memory, official soul cache, plugin cache, and workflow cache.

Project state should not be silently replaced by user state. User state can provide defaults and caches.

## Request Data

Product code builds user input into:

```text
Product input
  -> CoreRuntimeManager
  -> CoreRequest
  -> ProtocolEnvelope<CoreRequest>
  -> ProtocolInterface<CoreRuntime>
  -> CoreRuntime
```

The envelope carries:

- protocol version
- origin
- capability scope
- workspace root
- session reference
- run reference
- trace id
- payload

## Event Data

Core Runtime emits `CoreEvent` values:

- `RunStarted`
- `LoopIterationStarted`
- `ModelDelta`
- `ModelCompleted`
- `ToolCallStarted`
- `ToolCallCompleted`
- `ConvergenceChecked`
- `ApprovalRequested`
- `UserInputRequested`
- `ErrorRaised`
- `FinalResult`

The event stream is the preferred way to connect product UI, logs, traces, and future remote adapters.

## Tool Data

Tools are implemented as Rust WASM modules and exposed through `ToolRegistry` and `AliusTool`.

Tool calls carry:

- tool name
- JSON arguments
- workspace context
- session id
- result output or error

The permission and Shell Gate layers are the intended safety path for dangerous operations, but documentation must state where enforcement is not yet complete.

## Memory Data

Runtime memory and documentation memory are separate.

| Area | Purpose |
| --- | --- |
| `.alius/memory/` | Runtime data, project memories, conversations, logs, retrieval stores. |
| `.alius/workspace/` | Human and agent-readable project documentation. |
| `.alius/memory/design/` | Historical design memory and migration input. |
