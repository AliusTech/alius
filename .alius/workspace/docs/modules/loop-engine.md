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

- Execute Chat, Bypass, and Plan modes through one orchestration model.
- Emit `CoreEvent` values.
- Call the model through `runtime-model`.
- Execute tools through `runtime-tools` when enabled and available.
- Track loop iterations and final content.
- Apply context-window management when conversation context grows.

## Runtime Modes

`RuntimeMode` has three product-facing presets:

- `Chat` — single-turn conversational execution with bounded tool continuation.
- `Bypass` — direct execution without local plan drafting, using `BypassPermissions`.
- `Plan` — approved plan-node execution through the loop engine.

`LoopPolicy` carries `permission_strategy`:

- `AcceptEdits` — high-risk tool calls emit `ToolConfirmationRequired` and wait for user approval.
- `BypassPermissions` — Alius confirmation, workspace-boundary, manifest, and Shell Gate interception are skipped for that execution path. This does not bypass operating-system permissions, missing files, process spawn failures, command exit failures, network errors, or other lower-level runtime errors.

## Chat Mode

Chat mode uses `LoopPolicy::chat()`:

- up to 10 bounded tool-call continuations within one user turn
- tools enabled
- planning disabled
- convergence check enabled
- `permission_strategy = AcceptEdits`

It does not perform Plan-style goal decomposition or approved plan-node execution. Tool calls are allowed inside the same user turn so tool results can be returned to the model before the final answer.

## Bypass Mode

Bypass mode uses `LoopPolicy::bypass()`:

- bounded direct execution without local plan drafting
- tools enabled when a registry is available
- planning disabled
- convergence check enabled
- `permission_strategy = BypassPermissions`

Bypass mode is an explicit high-risk mode. It should still emit normal tool start/completion events and preserve audit visibility where the execution path has an audit sink.

## Plan Mode

Plan mode uses `LoopPolicy::plan()`:

- up to 20 iterations
- tools enabled
- planning enabled
- convergence check enabled
- `permission_strategy = BypassPermissions` by default after the user approves the plan proposal

Use `LoopPolicy::plan_accept_edits()` for step-by-step confirmation mode:

- tools enabled
- planning enabled
- convergence check enabled
- `permission_strategy = AcceptEdits`
- high-risk tools emit `ToolConfirmationRequired` and wait for the user decision

Plan mode requires a tool registry for tool execution. If no registry is available, the loop emits an error and final failure.

Plan step prompts must drive a complete local execution loop for implementation work:

- inspect existing backend/frontend code with `search_code`, `read_file`, and `list_dir` before editing when local context is needed
- run relevant tests, checks, or build commands after implementation or bug fixes
- when the task affects a locally runnable app or API, call `run_local_service` to verify a loopback service URL
- include commands run, test/build outcome, verified local service URL when applicable, whether that service was stopped, and changed files in the step result

The default `run_local_service` behavior is evidence-oriented: unless `keep_running=true` is explicitly requested, the service is verified and then stopped before the tool returns.

## Tool Step

`tool_step` dispatches model tool calls through `ToolRegistry`.

The tool step emits:

- `ToolCallStarted`
- `ToolCallCompleted`

Do not assume complete user approval UI integration on every product path.
