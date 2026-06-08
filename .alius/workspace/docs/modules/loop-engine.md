# Loop Engine Module

Primary paths:

- `runtime/core/src/loop_engine/engine.rs`
- `runtime/core/src/loop_engine/model_step.rs`
- `runtime/core/src/loop_engine/tool_step.rs`
- `runtime/core/src/loop_engine/context.rs`
- `runtime/core/src/loop_engine/context_manager.rs`
- `runtime/core/src/loop_engine/convergence.rs`
- `runtime/core/src/loop_engine/policy.rs`
- `runtime/core/src/loop_engine/result.rs`

## Responsibilities

- Execute Chat and Plan modes through one orchestration model.
- Emit `CoreEvent` values.
- Call the model through `runtime-model`.
- Execute tools through `runtime-tools` when enabled and available.
- Track loop iterations and final content.
- Apply context-window management when conversation context grows.

## Chat Mode

Chat mode uses `LoopPolicy::chat()`:

- one iteration
- tools disabled
- planning disabled
- convergence check enabled

It performs a single streaming model call and emits model delta plus final result events.

## Plan Mode

Plan mode uses `LoopPolicy::plan()`:

- up to 20 iterations
- tools enabled
- planning enabled
- convergence check enabled
- tool approval required by policy

Plan mode requires a tool registry for tool execution. If no registry is available, the loop emits an error and final failure.

## Tool Step

`tool_step` dispatches model tool calls through `ToolRegistry`.

The tool step emits:

- `ToolCallStarted`
- `ToolCallCompleted`

Do not assume complete user approval UI integration on every product path.

