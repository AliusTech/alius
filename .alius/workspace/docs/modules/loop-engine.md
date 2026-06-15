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

- up to 10 iterations
- tools enabled
- planning disabled
- convergence check enabled

It performs direct Chat/Bypass execution. When the model does not request tools,
the loop is a single streaming model call and emits model delta plus final result
events. When the model requests tools, the loop stores that assistant turn as one
message carrying `tool_calls`, executes the requested tools, and sends tool
results back to the model.

OpenAI-compatible APIs require every `tool` result message to directly follow
the preceding assistant message that carries `tool_calls`. The Loop Engine must
not insert synthetic assistant text, such as progress labels, between those two
message types. Tool progress belongs in `ToolCallStarted` and
`ToolCallCompleted` events.

Before sending tool results back to the model, the Loop Engine normalizes the
result list against the previous assistant `tool_calls`: results are ordered by
the assistant call order, every call id must have a result, and missing results
are converted into explicit error tool results. Context truncation must not run
while there are pending tool results, because removing the assistant tool-call
message would break the provider protocol frame.

After a successful continuation request, the normalized tool results are
persisted into the runtime conversation as protocol `tool` messages before the
next assistant turn is appended. This keeps multi-step tool runs valid, for
example:

```text
user
assistant(tool_calls: shell git clone)
tool(call_id: shell result)
assistant(tool_calls: list_dir)
tool(call_id: list_dir result)
assistant(final answer)
```

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
