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

All tools are implemented as Rust WASM modules. Rust native code may implement host, loader, policy, registry, event, audit, and distribution behavior, but it must not implement concrete model-callable tool business logic.

`AliusTool` is the host adapter trait. In the runtime registry, executable adapters come from Rust WASM modules through `WasmPluginTool`.

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

## Rust WASM Module Tools

Rust WASM module tooling exists under `wasm_host`. The runtime ABI currently exposes:

- `alius_plugin_list_tools()`
- `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len)`

The ABI, host capability bridge, schema validation, and package diagnostics still need hardening before the tool runtime is considered production-complete.
