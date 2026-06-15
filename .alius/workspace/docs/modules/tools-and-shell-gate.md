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
- Register Rust WASM module tool adapters through `ToolRegistry`.
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
- current working directory
- origin
- workspace root
- risk level
- workspace scope
- authorization policy

Shell Gate exists as a subsystem. Documentation should not claim total enforcement until the relevant tool path calls it consistently.

Git commands are classified by subcommand:

- `git status`, `git log`, `git diff`, `git show`, and `git branch` are low risk.
- `git clone`, `git fetch`, `git pull`, and `git submodule` are medium risk because they write to the workspace and/or use the network.
- `git clean` and `git reset --hard` are high risk.
- Other mutating git subcommands, such as `push`, `checkout`, `switch`, `merge`, `rebase`, `restore`, `add`, and `commit`, are medium risk.

In Chat/Bypass mode, `ApprovalRequired` shell decisions are executable tool operations rather than a user confirmation prompt. For example, a model-requested `shell` tool call with `git clone <url>` executes directly in Bypass mode and returns its output through tool results plus `ToolCallStarted`/`ToolCallCompleted` events.

## Rust WASM Module Tools

Rust WASM module tooling exists under `wasm_host`. The runtime ABI currently exposes:

- `alius_plugin_list_tools()`
- `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len)`

The ABI, host capability bridge, schema validation, and package diagnostics still need hardening before the tool runtime is considered production-complete.

## Native Tools

Built-in tools under `runtime/tools/src/native/`. Each implements `AliusTool` directly in Rust and is registered automatically in every registry.

- **`shell`** (`native/shell.rs`) — runs a command via `sh -c` (Unix) or `cmd /C` (Windows). `PermissionLevel::Execute`. Pipeline: parse args → resolve cwd under workspace → `shell_gate::authorize` → on `Allow`/`ApprovalRequired`-in-Chat run, on `Deny` reject, on `ApprovalRequired`-in-Plan return "confirmation required". 120 s default timeout (`tokio::time::timeout`), env fully inherited, output `[exit:N]\n<stdout>\n<stderr>` truncated at 20 000 chars.
- **`read_file`** (`native/fs.rs`) — `Read`. Workspace-boundary canonicalize, `tokio::fs::read_to_string`.
- **`write_file`** — `Write`. Boundary; in Plan mode returns "confirmation required" (Stage B confirmation flow will pause instead).
- **`list_dir`** — `Read`. Boundary; sorted `file\tname` / `dir\tname` lines.
- **`edit_file`** — `Write`. Boundary; replaces all occurrences of `find` with `replace`; Plan mode requires confirmation.

`resolve_within_workspace(path, workspace, must_exist)` is the shared boundary helper (join + canonicalize + `startswith workspace`). It rejects absolute paths, `../` traversal, and symlink escape in one place.

### ToolContext

`ToolContext` carries `workspace`, `session_id`, `working_directory`, and `mode: RuntimeMode` (Plan/Chat). Tools consult `mode` to decide whether a risky operation needs confirmation (Plan) or executes directly (Chat/Bypass). The `AliusTool::preview_confirmation(args, mode)` hook (default `false`) is the future integration point for the Stage B confirmation pause/resume flow.
