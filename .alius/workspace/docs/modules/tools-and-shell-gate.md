# Tools and Shell Gate Module

Primary paths:

- `runtime/tools/src/traits.rs`
- `runtime/tools/src/registry.rs`
- `runtime/tools/src/package.rs`
- `runtime/tools/src/permission.rs`
- `runtime/tools/src/shell_gate/`
- `runtime/tools/src/wasm_host/`

## Responsibilities

- Define `AliusTool`.
- Register native tools and Rust WASM module tool adapters through `ToolRegistry`.
- Export provider-compatible tool definitions.
- Load and execute Rust WASM module tools.
- Inspect shell command risk and scope.
- Represent permission levels.

## Main Types

- `AliusTool`
- `ToolRegistry`
- `ToolContext`
- `ToolResult`
- `ConfirmationRequest`
- `ToolPackage`
- `ToolPackageManifest`
- `ToolPackageResolver`
- `ToolRuntimeHost`
- `ToolHostCapability`
- `PermissionLevel`
- `ShellCommandRequest`
- `ShellGateResult`
- `ShellGateDecision`
- `ShellInspection`
- `ScopeAnalysis`

## Tool Implementation Rule

Tools fall into two categories:

- **WASM plugin tools** — sandboxed, third-party-distributable, pure-computation (no direct OS access). Loaded through `WasmPluginTool`.
- **Native tools** (`runtime/tools/src/native/`) — built-in Rust `AliusTool` impls that need OS syscalls the WASM sandbox cannot provide (shell execution, filesystem access). They reuse the same security primitives (Shell Gate, workspace boundary).

`AliusTool` is the trait both categories implement. `ToolRegistry` stores `Arc<dyn AliusTool>` so native and WASM-backed tools coexist in one map. Native tools are registered by `native::register_native_tools` during registry build (`package.rs` `build_registry`/`build_registry_lossy`).

## Shell Gate

Shell Gate is intended to prevent unsafe shell, process, and git behavior by analyzing:

- command string
- arguments
- paths parsed from the raw command when explicit arguments are absent
- redirection targets
- current working directory
- origin
- workspace root
- risk level
- workspace scope
- authorization policy

Shell Gate exists as a subsystem. Documentation should not claim total enforcement until the relevant tool path calls it consistently.

Scope analysis resolves path-like arguments relative to the command cwd and checks whether they stay inside the current workspace. This includes absolute paths (`/etc/passwd`), parent-directory escapes (`../secret`), option values (`--output=/tmp/file`), and redirection targets (`> /tmp/file`, `2>/tmp/error`). URL-like values are ignored as paths.

Git commands are classified by subcommand:

- `git status`, `git log`, `git diff`, `git show`, and `git branch` are low risk.
- `git clone`, `git fetch`, `git pull`, and `git submodule` are medium risk because they write to the workspace and/or use the network.
- `git clean` and `git reset --hard` are high risk.
- Other mutating git subcommands, such as `push`, `checkout`, `switch`, `merge`, `rebase`, `restore`, `add`, and `commit`, are medium risk.

**Workspace boundary violations are hard-denied**: commands that reference paths outside the workspace (via absolute paths, `../` escape, redirections like `> /tmp/out`, or flags like `--output=/external`) are always rejected with `ShellGateDecision::Deny`, regardless of runtime mode. This ensures Chat/Bypass modes cannot execute external-path operations. `ApprovalRequired` is reserved for high-risk commands that remain within workspace boundaries (e.g., `rm -rf ./build`).

In Chat/Bypass mode, `ApprovalRequired` shell decisions execute directly and return tool results via `ToolCallStarted`/`ToolCallCompleted` events. In Plan mode, they trigger user confirmation before execution.

## Rust WASM Module Tools

Rust WASM module tooling exists under `wasm_host`. The runtime ABI currently exposes:

- `alius_plugin_list_tools()`
- `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len)`

The ABI, host capability bridge, schema validation, and package diagnostics still need hardening before the tool runtime is considered production-complete.

## Native Tools

Built-in tools under `runtime/tools/src/native/`. Each implements `AliusTool` directly in Rust and is registered automatically in every registry.

- **`shell`** (`native/shell.rs`) — runs a command via `sh -c` (Unix) or `cmd /C` (Windows). `PermissionLevel::Execute`. Pipeline: parse args → resolve cwd under workspace → `shell_gate::authorize` → on `Deny` reject, on `Allow` run, and on `ApprovalRequired` pause for user confirmation in Plan mode but run directly in Chat/Bypass mode. 120 s default timeout (`tokio::time::timeout`), env fully inherited, output `[exit:N]\n<stdout>\n<stderr>` truncated at 20 000 chars.
- **`read_file`** (`native/fs.rs`) — `Read`. Workspace-boundary canonicalize, `tokio::fs::read_to_string`.
- **`write_file`** — `Write`. Boundary; in Plan mode returns "confirmation required" (Stage B confirmation flow will pause instead).
- **`list_dir`** — `Read`. Boundary; sorted `file\tname` / `dir\tname` lines.
- **`edit_file`** — `Write`. Boundary; replaces all occurrences of `find` with `replace`; Plan mode requires confirmation.

`resolve_within_workspace(path, workspace, must_exist)` is the shared boundary helper (join + canonicalize + `startswith workspace`). It rejects absolute paths, `../` traversal, and symlink escape in one place.

### ToolContext

`ToolContext` carries `workspace`, `session_id`, `working_directory`, and `mode: RuntimeMode` (Plan/Chat). Tools consult `mode` to decide whether a risky operation needs confirmation (Plan) or executes directly (Chat/Bypass).

### Confirmation Chain (Stage B)

When `preview_confirmation` returns `true` (Plan mode + risky op), the tool step follows this chain:

1. **Request**: emit `ToolConfirmationRequired` event, store a oneshot sender in `SessionManager`, transition run status to `WaitingForApproval`.
2. **Await**: the loop blocks on the oneshot receiver.
3. **Approved** (`rx.await` returns `Ok(true)`): restore status to `Running` only if still in `WaitingForApproval`; execute the tool; emit `ToolCallCompleted`.
4. **Denied** (`rx.await` returns `Ok(false)`): tool is NOT executed; emit `ToolCallCompleted` with `success: false`, `denied: true`, `denial_reason: "denied_by_user"`. The entire batch is aborted — remaining tool calls are skipped.
5. **Cancelled** (sender dropped, `rx.await` returns `Err`): batch aborted, status is NOT restored from `Cancelled`.
6. **No session**: fail-closed — batch aborted, returns `unavailable` reason.

**Fail-fast**: `execute_tools` processes tool calls sequentially. Once any confirmation is denied, cancelled, or unavailable, the remaining tool calls in the batch are filled with "skipped" error placeholders and NOT executed.

**Terminal state protection**: `confirm_and_await` and `deliver_confirmation` only restore status to `Running` on explicit `Approved` decision and only when the current status is `WaitingForApproval`. The status check + update in `deliver_confirmation` is atomic (under the same write lock) to prevent a race with `cancel_run`.

**Loop termination on denial**: `run_plan` and `run_chat_with_tools` check `ToolBatchResult.batch_denied` after `execute_tools` returns. On denial, the engine emits `ErrorRaised(code: "tool_denied")` and `FinalResult(success: false)`, preventing the model from continuing after user denial. This uses the structured `batch_denied` field — no string matching.

**Confirmation decision type**: `confirm_and_await` returns a `ConfirmationDecision` enum (`Approved`, `Denied`, `Cancelled`, `Unavailable`) instead of a raw `bool`, preserving the reason for audit and error reporting.

**Audit logging**: confirmation events are written to the event log via `audit::log_confirmation`. The `LogWriter` is created by `CoreRuntime` and passed through `LoopContext` → `execute_tools` → `confirm_and_await`. Events include `requested`, `approved`, `denied_by_user`, `cancelled`, and `no_session` with `tool_name`, `tool_call_id`, `run_ref`, `trace_id`. Raw args and sensitive content are NOT logged. Audit write failures are non-blocking — they emit `ErrorRaised` events (codes `audit_lock_poisoned`, `audit_write_failed`, `audit_flush_failed`) so failures are observable without blocking tool execution.
