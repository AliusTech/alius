# P3-2: Native Tools Enter Default ToolRegistry

**Date:** 2026-06-15
**Branch:** fix/tools-native-registry-confirmation
**Status:** Complete

## Problem

`ToolPackageResolver::build_registry()` only loaded WASM tools. Without WASM plugins the
registry was empty; native tools were never auto-registered despite documentation stating
otherwise.

## Changes

### package.rs — always register native tools

`build_registry()` now calls `crate::native::register_native_tools()` before loading WASM
packages. `build_registry_lossy()` does the same in its error fallback path, so native tools
are present even when WASM loading fails entirely.

### registry.rs — reject duplicate tool names

`register()` now returns `Result<(), String>`. If a tool with the same name is already
registered the call fails. This prevents a WASM plugin from silently replacing a built-in
native tool (e.g. replacing `shell` to bypass Shell Gate).

WASM loading in `package.rs` and `lib.rs` logs the conflict and skips the duplicate.

### Tests

**registry.rs unit tests (5):**
- `test_native_tools_registered`
- `test_get_native_tools`
- `test_to_tool_defs_includes_native`
- `test_duplicate_name_rejected`
- `test_all_native_names_rejected_on_duplicate`

**native_registry.rs integration tests (12):**
- `test_default_registry_contains_native_tools`
- `test_get_returns_native_tools`
- `test_to_tool_defs_includes_native_tools`
- `test_native_tools_have_valid_schemas`
- `test_shell_preview_confirmation_in_plan_mode`
- `test_shell_preview_confirmation_in_chat_mode`
- `test_write_file_preview_confirmation_in_plan_mode`
- `test_write_file_no_confirmation_in_chat_mode`
- `test_edit_file_preview_confirmation_in_plan_mode`
- `test_edit_file_no_confirmation_in_chat_mode`
- `test_chat_mode_workspace_internal_executes`
- `test_chat_mode_external_path_denied`

## Acceptance

| Requirement | Status |
|---|---|
| Default registry contains native tools without WASM plugins | Pass |
| `get()` and `to_tool_defs()` return native tools | Pass |
| Plan mode shell/write/edit require confirmation preview | Pass |
| Chat mode shell/write/edit skip confirmation preview | Pass |
| Chat mode workspace-internal command executes | Pass |
| Chat mode external-path command denied | Pass |
| WASM plugin cannot shadow native tool name | Pass |

## Quality Gates

```
cargo fmt --all -- --check          pass
cargo check --workspace --all-targets --all-features  pass
cargo clippy --workspace --all-targets --all-features -- -D warnings  pass
cargo test -p runtime-tools --test native_registry -- --test-threads=1  12 passed
cargo test --workspace -- --test-threads=1  all passed
```
