# Documentation History

All entries use the format:

```text
[YYYY-MM-DD HH:MM] [author]: [path] - [summary]
```

## 2026-06-19

[2026-06-19 01:54] Codex: protocol/src/core.rs + runtime/core/src/loop_engine + runtime/tools/src + entrypoints/cli/src/tui + .alius/workspace/docs - Added Chat/Bypass/Plan runtime permission strategy semantics. Approved Plan execution now defaults to BypassPermissions, active plan execution can switch to AcceptEdits with Shift+Tab, and docs/tests describe the high-risk bypass boundary.

## 2026-06-18

[2026-06-18 20:39] Codex: protocol/src/core.rs + protocol/src/interface.rs + runtime/config/src/views.rs + runtime/config/src/loaders/permissions.rs + runtime/tools/src/shell_gate + .alius/workspace/docs - Removed the deprecated SDK protocol origin and SDK permission block; renamed the internal Shell Gate SDK-style origin to WasmPlugin so WASM plugin safety uses plugin terminology.

[2026-06-18 18:00] Codex: .alius/workspace/docs/modules/agent-team.md + .alius/workspace/SPEC.md + .alius/workspace/docs/standards/documentation-maintenance.md - Removed incorrect Team-mode execution-subject wording and added Agent-centered terminology rules for Agent Team documentation.

[2026-06-18 17:56] Codex: .alius/workspace/docs/modules/agent-team.md + SPEC.md + docs/00-reading-path.md + docs/overview/architecture.md + docs/products/tui-workspace.md + docs/terms/GLOSSARY.md + docs/01-current-state.md - Added Agent CLI to Agent Team Backend WebSocket connection design, including FastAPI backend compatibility, Rust client library choices, registration, heartbeat, presence, work status, task lease, reconnect, permission, and TUI boundary rules.

[2026-06-18 10:00] Alius: .github/workflows/release.yml - Added test-gate job before create-release; moved tag creation to after test-gate passes; added --locked to all release builds; added per-platform test-symbol scan before artifact upload; added cargo-audit to release test-gate.

[2026-06-18 10:00] Alius: .github/workflows/ci.yml - Added cargo-llvm-cov coverage collection with --ignore-filename-regex for testing files; added staged coverage threshold (65%); added provider-smoke job with correct .alius/config/ structure, binary pre-build, config validation with DeepSeek/model assertion, set -o pipefail, and failure-pattern rejection; documented audit exceptions.

[2026-06-18 10:00] Alius: entrypoints/cli/src/main.rs - Fixed `alius run` to return non-zero exit code on failure (ErrorRaised or FinalResult.success=false).

[2026-06-18 10:00] Alius: entrypoints/cli/src/tui/testing.rs - Implemented TuiTestHarness (workspace state harness with key/mouse injection, terminal size variants, block inspection, tool confirmation state, config task injection, execution mode, folding) and VecEventSource (ordered Core event replay with next/peek/reset/drain).

[2026-06-18 10:00] Alius: entrypoints/cli/src/tui/workspace/mod.rs + state_machine_tests.rs - Added 44 TUI state-machine tests covering welcome block, Plan/Bypass toggle, config-task Shift+Tab guard, Esc interrupt, Ctrl+C/D quit, focus zone cycling, tool confirmation inject/clear, terminal size variants, block manipulation, streaming text, folding, execution mode, responsive layout, input/submit, backspace, tab completion, VecEventSource, Core event replay helpers. Tests extracted to separate file for coverage exclusion.

[2026-06-18 10:00] Alius: runtime/core/src/runtime.rs - Stabilized cancel_streaming_run_stops_future_events test with retry loop to fix race condition in serial test execution.

[2026-06-18 10:00] Alius: .alius/workspace/docs/standards/validation.md - Added staged coverage threshold table (Stage 0: 65% → Stage 4: 85%) with per-stage targets, dates, and criteria; updated coverage exclusion regex to include state_machine_tests.rs; clarified --ignore-filename-regex must be passed to all report commands.

[2026-06-18 10:00] Alius: .alius/workspace/docs/overview/implementation-gaps.md - Updated testing infrastructure section to list TuiTestHarness and VecEventSource; updated test count.

[2026-06-18 10:00] Alius: docs/audit-exceptions.md - Documented RUSTSEC-2025-0012 (backoff), RUSTSEC-2024-0384 (instant), RUSTSEC-2024-0436 (paste), RUSTSEC-2026-0002 (lru) with impact, mitigation, owner, and review date.

## 2026-06-17

[2026-06-17 18:00]: test infrastructure - Added `testing` feature flag to all 9 workspace crates with proper feature propagation
[2026-06-17 18:00]: protocol/src/testing.rs - Created shared TestRuntime (CoreRuntimeApi impl)
[2026-06-17 18:00]: runtime/tools/src/testing.rs - Created shared FakeTool, EchoTool, ConfirmationRequiredTool
[2026-06-17 18:00]: runtime/model/src/testing.rs - Created shared FakeProvider, NoOpProvider, FakeToolCallProvider
[2026-06-17 18:00]: runtime/core/src/testing.rs - Created CoreRuntimeHarness, FakeMcpEchoTool, FakeMcpToolCallProvider
[2026-06-17 18:00]: entrypoints/cli/src/testing.rs - Created CwdGuard, enter_temp_cwd, temp_workspace helpers
[2026-06-17 18:00]: entrypoints/cli/src/tui/testing.rs - Created key, key_with, type_text, submit_input helpers
[2026-06-17 18:00]: runtime/tools/src/registry.rs - Migrated inline FakeTool/FakeMcpTool to shared testing module
[2026-06-17 18:00]: runtime/core/src/loop_engine/engine.rs - Migrated inline McpEchoTool/McpToolCallProvider to shared testing module
[2026-06-17 18:00]: entrypoints/jsonrpc/src/lib.rs - Migrated inline FakeMcpTool to shared testing module
[2026-06-17 18:00]: .github/workflows/ci.yml - Added --features testing, release build, symbol scan
[2026-06-17 18:00]: entrypoints/cli/tests/ - Created 30 CLI integration tests (parse, config, extensions, run)
[2026-06-17 18:00]: docs/01-current-state.md - Updated with testing infrastructure status
[2026-06-17 18:00]: docs/overview/implementation-gaps.md - Added testing infrastructure section
[2026-06-17 18:30]: entrypoints/jsonrpc/src/lib.rs - Added 7 tests: run_confirm_tool contract, malformed request, nonexistent run_ref
[2026-06-17 18:30]: runtime/tools/tests/shell_gate_integration.rs - Added 5 tests: low-risk success, bypass mode, authorization
[2026-06-17 18:30]: runtime/core/src/runtime.rs - Added 3 tests: close_session, clear_conversation, nonexistent session
[2026-06-17 18:30]: entrypoints/cli/src/workflow/mod.rs - Added 5 tests: load_workflow from disk, load_workflows directory
[2026-06-17 19:00]: runtime/tools/src/wasm_host/host.rs - Added 5 tests: list/find/remove lifecycle, validate_wasm_module
[2026-06-17 19:00]: entrypoints/cli/src/workflow/mod.rs - Added 3 tests: condition failed operator, nonexistent step

## 2026-06-17

[2026-06-17 23:59] Reviewer: .alius/workspace/docs/products/tui-workspace.md + .alius/workspace/docs/standards/validation.md + .alius/workspace/HISTORY.md - Renamed CI reporting language, replaced provider-network terminology with selected-provider smoke testing, clarified DeepSeek/default-provider configuration coverage, and expanded TUI state-machine test requirements.

[2026-06-17 23:59] Reviewer: .alius/workspace/docs/products/cli.md + .alius/workspace/docs/standards/validation.md + .alius/workspace/HISTORY.md - Added functional test surface requirements for every CLI command family and documented deterministic functional tests, mocked network tests, selected-provider smoke tests, secret handling, and trusted-context CI policy.

[2026-06-17 23:59] Reviewer: .alius/workspace/docs/products/tui-workspace.md + .alius/workspace/docs/standards/validation.md + .alius/workspace/HISTORY.md - Added TUI TestKit design and release isolation rules; documented CI-native reporting, coverage artifacts, pre-build test gate, release smoke checks, and the ban on third-party test report services.

[2026-06-17 23:59] Alius: entrypoints/cli/src/workflow/mod.rs + runtime/tools/src/wasm_host/host.rs + entrypoints/cli/src/main.rs + runtime/tools/src/wasm_host/imports.rs + docs — P5 review blocker fixes: (1) Workflow run_tool now checks preview_confirmation() and fails closed when confirmation is required — no bypass of the confirmation gate; (2) plan_plugin_install now calls validate_wasm_module() to reject invalid WASM bytes at install time; (3) CLI main() replaced expect("Failed to run") with clean error message and exit(1); (4) Added execute_fetch execution-level tests (success, server error, oversized response, connection refused) and WASM integration tests (HTTP rejection, no-permission, undeclared domain, allowed URL audit); (5) Synced implementation-gaps.md, plugin-permissions.md, development-roadmap.md to remove stale gaps and reflect P5 fixes. Added PluginInstallPlan Debug derive, tempfile dev-dependency, and fixed fetch import to use std::thread::spawn for async HTTP in sync WASM context. All 710 tests passing.

[2026-06-17 23:59] Reviewer: .alius/workspace/status/p5-review-blockers-task-card.md + .alius/workspace/HISTORY.md - Added strict P5 review blocker task card after reviewing the completed G1-G6 candidate; documented per-item completion status, blocking issues, required fixes, acceptance tests, prohibited changes, and verification commands.

[2026-06-17 15:02] Codex: .alius/workspace/status/development-roadmap.md + .alius/workspace/HISTORY.md - Added a living development roadmap and progress report for the current P4 candidate, including review-report cross-check, feature/permission/security completeness assessment, high-priority gaps, branch plan, and future review gates.

[2026-06-17 06:00] Alius: entrypoints/cli/src/workflow/mod.rs + entrypoints/cli/src/main.rs - P4-7 Workflow Runtime wiring: LoopEngineHandle trait methods are now async (async_trait); added RuntimeWorkflowHandle that delegates run_prompt to CoreRuntimeManager::run_text (real LLM provider via LoopEngine) and run_tool to ToolRegistry::get + AliusTool::execute (real WASM/native/MCP tool path); CLI workflow run now constructs CoreRuntimeManager and ToolRegistry instead of StubLoopEngineHandle; added test_runtime_handle_uses_real_paths integration test with fake provider + fake tool proving real paths are exercised (output must not contain [prompt] or [tool:*] stub markers); StubLoopEngineHandle retained for unit tests only.

[2026-06-17 03:00] Alius: runtime/tools/src/wasm_host/host.rs + runtime/tools/src/wasm_host/mod.rs + .alius/workspace/docs/modules/plugin-permissions.md + .alius/workspace/docs/overview/implementation-gaps.md + .alius/workspace/HISTORY.md - P4-3 Runtime Permission Matcher: added PermissionDecision enum (Allow/Deny{reason}) and four check_* methods on ResolvedPluginPermissions (check_filesystem, check_network, check_shell, check_env); filesystem matcher canonicalizes paths, enforces workspace boundary, matches declared prefix with directory-boundary semantics; network matcher does URL prefix match with domain-boundary enforcement (prevents api.example.com.evil.com matching api.example.com); shell matcher supports exec:readonly (read-only command set including git) and exec:literal (exact match); env matcher enforces exact variable name match; old manifests default-deny all checks; READONLY_SHELL_COMMANDS constant aligns with shell_gate/inspector.rs LOW_RISK_COMMANDS; normalize_lexical helper for path normalization fallback; 30+ new tests covering default deny, filesystem (allow/traversal/absolute/symlink escape/operation mismatch/prefix boundary), network (prefix/undeclared/similar domain), shell (readonly allow/deny/literal/dangerous), env (exact/empty/prefix/undeclared), ToolPackageManifest integration; exported PermissionDecision from mod.rs; updated docs to reflect matcher is implemented but wasm→host imports and audit sink are not.

[2026-06-17 05:00] Alius: P4 Extension System Maturity — candidate checkpoint. 10 of 10 sub-goals completed.

- P4-1: Monorepo Extension Catalog — DONE. extensions/ directory with registry.toml, 6 souls migrated, bundled_souls_path() resolver, OFFICIAL_REMOTE deprecated, 11 formula tests
- P4-2: WASM Permission Matcher — DONE. PermissionDecision with resolved_path, 4 check_* methods, default-deny, 30+ matcher tests
- P4-3: Host Function Audit Sink — DONE. HostAuditEvent/HostAuditSink, TracingAuditSink, sensitive data redaction, 5 audit tests
- P4-4: WASM Host Imports — DONE. 6 host functions with permission matcher → Shell Gate → audit → execute pipeline, 7 integration tests
- P4-5: Plugin Registry — DONE. ExtensionRegistry loader, hello-world sample plugin, build docs
- P4-6: MCP Completeness — DONE. Duplicate tool regression, source tagging verified
- P4-7: Workflow Runtime — DONE. RuntimeWorkflowHandle wires workflow run to CoreRuntimeManager (LLM) and ToolRegistry (tools); async trait; integration test proves real paths; 9 workflow tests total
- P4-8: JSON-RPC Surface — DONE. run_confirm_tool added, 33 tests
- P4-9: Flaky Test Fix — DONE. 3/3 consecutive passes
- P4-10: Documentation Sync — DONE

[2026-06-17 02:15] Alius: runtime/tools/src/wasm_host/host.rs + runtime/tools/src/wasm_host/mod.rs + runtime/tools/src/package.rs + .alius/workspace/docs/modules/plugin-permissions.md + .alius/workspace/docs/overview/implementation-gaps.md + .alius/workspace/HISTORY.md - P4-2 Review fix: validate_env_permission now rejects wildcard targets (*) and invalid env var names (must be [A-Za-z_][A-Za-z0-9_]*); added InvalidEnvVarName error variant; fixed test_wildcard_env_rejected to assert error; restored validate_wasm_module export in mod.rs; added package conversion tests (Some(permissions) preserves four domains, None yields empty); updated docs to reflect env wildcard and empty targets are rejected; updated implementation-gaps.md WASM status.

[2026-06-17 02:00] Alius: runtime/tools/src/wasm_host/host.rs + runtime/tools/src/wasm_host/mod.rs + runtime/tools/src/package.rs + .alius/workspace/docs/modules/plugin-permissions.md + .alius/workspace/docs/overview/implementation-gaps.md - P4-2 WASM Plugin Manifest Permissions: added PluginManifest.permissions field (optional, empty default for backward compatibility); structured permission model covering filesystem/network/shell/env domains with "operation:target" format; install-time validation rejects malformed entries (path traversal, absolute paths, unknown operations, empty targets); ResolvedPluginPermissions with summary_lines() for install-time display; ToolPackageManifest preserves permissions via From<PluginManifest>; 17 new tests covering empty/valid/malformed permissions, path traversal, absolute paths, env wildcards/empty, multiple errors, TOML parsing, unknown fields; updated plugin-permissions.md status to reflect implemented manifest validation; updated implementation-gaps.md WASM section.

[2026-06-17 01:30] Alius: runtime/mcp/tests/mcp_e2e.rs + runtime/mcp/Cargo.toml + .alius/workspace/docs/overview/implementation-gaps.md - P4-1 review fix: 9 E2E tests now verify full MCP chain including ToolRegistry registration and execute_tools execution path. Tests: initialize→server info, tools/list→echo tool, tools/call→echo response, unknown tool error, protocol sequence, McpToolAdapter source==Mcp, McpToolAdapter execute, ToolRegistry register+to_tool_infos with ToolSource::Mcp, registry get+execute, execute_tools full chain with events, source propagation through registry. Updated implementation-gaps.md to reflect full chain verification: initialize → tools/list → tools/call → McpToolAdapter → ToolRegistry → execute_tools.

[2026-06-17 01:00] Alius: runtime/mcp/tests/mcp_e2e.rs + runtime/mcp/tests/fixtures/echo_mcp_server.py + runtime/mcp/Cargo.toml + .alius/workspace/docs/overview/implementation-gaps.md - P4-1 MCP real server E2E: added echo_mcp_server.py fixture implementing MCP stdio protocol (initialize, notifications/initialized, tools/list, tools/call); initial 7 integration tests verifying protocol flow; updated implementation-gaps.md.

[2026-06-17 00:35] Alius: runtime/core/src/runtime.rs + runtime/core/src/session.rs + .alius/workspace/docs/modules/tools-and-shell-gate.md - P3-3 Review fix: emit_audit_diagnostic now uses monotonically increasing sequence from SessionManager::next_event_sequence() instead of hardcoded 0; added next_event_sequence() method to SessionManager; updated test to verify sequence > 0 and monotonic increase; documented audit_no_writer diagnostic code in tools-and-shell-gate.md.

[2026-06-17 00:25] Alius: runtime/core/src/runtime.rs - P3-3 Final review fix: delivery_failed audit path now emits LogRecordEmitted diagnostic events on lock/write/flush failures, consistent with tool_step::audit_confirmation semantics; added audit_delivery_failed and emit_audit_diagnostic helper methods; added delivery_failed_audit_emits_diagnostic_on_failure test verifying diagnostic event emission and non-blocking behavior (run status unchanged).

## 2026-06-16

[2026-06-16 06:00] Alius: runtime/core/src/runtime.rs + .alius/workspace/docs/overview/implementation-gaps.md - P3-3 Final review fix: delivery_failed audit tests now use CoreRuntimeBuilder with real LogWriter to verify audit log file contents; tests assert event_type=tool_confirmation, action=delivery_failed, trace_id=envelope.trace_id, tool_call_id correct, tool_name non-empty, no sensitive data leaked; added receiver-dropped audit test preserving original tool_name; updated implementation-gaps.md to document tool_name="unknown" sentinel for no-pending/run-not-found cases.

[2026-06-16 05:45] Alius: runtime/core/src/session.rs + runtime/core/src/runtime.rs + runtime/core/src/loop_engine/tool_step.rs - P3-3 Review fix: deliver_confirmation now checks sender.send() return value and treats Err as delivery failure; store_confirmation_sender stores trace_id alongside tool_name; deliver_confirmation returns tool_name on success and (error, tool_name) on failure; runtime.rs uses envelope.trace_id for audit trace (not session-returned trace); no-pending/run-not-found cases use "unknown" sentinel for tool_name; added 4 new session tests: delivery_failed_receiver_dropped, delivery_failed_no_pending_confirmation, delivery_failed_nonempty_metadata, delivery_success_metadata.

[2026-06-16 05:30] Alius: runtime/core/src/session.rs + runtime/core/src/runtime.rs + runtime/core/src/loop_engine/tool_step.rs + .alius/workspace/docs/overview/implementation-gaps.md - P3-3 Delivery failure audit: enhanced store_confirmation_sender to store tool_name alongside oneshot sender; deliver_confirmation now returns tool_name for audit logging; runtime.rs logs delivery_failed audit event when respond_confirmation fails; delivery failure triggers automatic run cancellation (fail-closed); documentation updated to reflect delivery_failed is now implemented with full audit trail.

[2026-06-16 05:25] Alius: .alius/workspace/docs/overview/implementation-gaps.md + .alius/workspace/HISTORY.md - P3-2 Review fix: corrected audit action names to match runtime implementation. Documentation now uses `denied_by_user` (not `denied`) and `no_session` (not `unavailable`) to match ConfirmationDecision::reason() return values.

[2026-06-16 05:15] Alius: entrypoints/cli/src/tui/workspace/mod.rs + .alius/workspace/docs/overview/implementation-gaps.md + .alius/workspace/HISTORY.md - P3-2 Review fix: fixed UTF-8 unsafe truncation in truncate_details (now uses chars().take() for Unicode safety); added Chinese/emoji truncation tests; corrected implementation-gaps.md audit section to accurately reflect delivery_failed is NOT yet logged as separate audit entry (only observable via TUI error state and run cancellation); audit path for delivery_failed requires TUI to pass LogWriter or emit CoreEvent, documented as not yet implemented.

[2026-06-16 05:00] Alius: entrypoints/cli/src/tui/workspace/mod.rs + runtime/core/src/logging/audit.rs + .alius/workspace/docs/overview/implementation-gaps.md - P3-2 Tool confirmation UX and audit consistency: enhanced show_tool_confirmation to display formatted JSON args (key=value pairs), tool_call_id in all confirmation blocks; fail_tool_confirmation shows user-friendly errors without stack traces; added format_tool_args and truncate_details helpers; added 9 new UX tests (prompt content, approve/deny blocks with ID, fail-safe error display, JSON formatting, truncation); added 2 new audit tests verifying JSON structure and sensitive data exclusion; updated implementation-gaps.md with complete confirmation flow documentation and audit logging details.

[2026-06-16 04:45] Alius: entrypoints/cli/src/repl/protocol_bridge.rs + .alius/workspace/docs/overview/implementation-gaps.md - P3-1 Streaming-path acceptance tests: added streaming_confirmation_approve_path and streaming_confirmation_deny_path tests that verify the full bridge path with fake LLM provider and tool registry. Tests prove: start streaming → ToolConfirmationRequired → respond_confirmation → runtime resumes → final result (approve) or denied result (deny). Updated implementation-gaps.md to reflect bridge-level acceptance tests are now covered.

[2026-06-16 04:30] Alius: entrypoints/cli/src/tui/workspace/mod.rs + entrypoints/cli/src/repl/protocol_bridge.rs + .alius/workspace/docs/overview/implementation-gaps.md + .alius/workspace/HISTORY.md - P3-1 Review fix: fail-closed confirmation response handling. respond_confirmation errors now show error to user and cancel run instead of silently clearing state. Refactored WorkspaceState to separate confirmation extraction (handle_tool_confirmation_response), success handling (complete_tool_confirmation), and failure handling (fail_tool_confirmation). Rewrote protocol_bridge tautological test to verify error delegation. Updated implementation-gaps.md to accurately describe partial implementation status.

[2026-06-16 04:00] Alius: entrypoints/cli/src/repl/protocol_bridge.rs + entrypoints/cli/src/tui/workspace/mod.rs + protocol/src/core.rs + protocol/src/interface.rs + runtime/core/src/runtime.rs + runtime/core/src/manager.rs + .alius/workspace/docs - P3-1 Tool Confirmation Bridge: implemented end-to-end tool confirmation flow from TUI to Core Runtime. Added `respond_confirmation()` to ProtocolBridge, CoreRuntimeManager, and ProtocolInterface. CoreRuntime::send() now handles RespondToolConfirmation command kind. TUI shows confirmation prompt for Plan mode tools, handles Approve/Deny responses, and sends results back through protocol bridge. Added ToolConfirmationState struct and 6 unit tests. Updated docs to reflect tool confirmation is now fully wired.

[2026-06-16 04:15] Alius: entrypoints/cli/src/tui/workspace/mod.rs - P3-1 Review fix: added ToolConfirmationResponse variant to ExecutionInputAction enum; updated handle_execution_key to route keys to prompt input when confirmation is pending; updated execute_goal and execute_plan_step to handle confirmation responses through bridge; added ToolConfirmationRequired event handling in execute_plan_step; all execution paths now support tool confirmation approve/deny/cancel flow.

[2026-06-16 03:30] Alius: runtime/core/src/loop_engine/engine.rs - P4-4 review fix: replaced execute_tools-level test with two engine-level tests using LoopEngine::run + fake LlmProvider: mcp_tool_executed_through_engine_chat_mode (no confirmation, verifies final_content includes MCP output, no ToolConfirmationRequired), mcp_tool_executed_through_engine_plan_mode_with_confirmation (Plan mode, verifies ToolConfirmationRequired emitted, approved → executes correctly, source==Mcp); updated implementation-gaps.md.

[2026-06-16 03:15] Alius: runtime/core/src/loop_engine/engine.rs + .alius/workspace/docs/overview/implementation-gaps.md - P4-4 MCP tool LoopEngine execution test: added mcp_tool_executed_through_registry test that registers a fake MCP-source tool (mcp_echo) in ToolRegistry, executes it through execute_tools (same path as LoopEngine), verifies source==Mcp, output matches, ToolCallStarted/ToolCallCompleted events emitted; updated implementation-gaps.md to reflect MCP tool execution is now tested.

[2026-06-16 03:00] Alius: entrypoints/jsonrpc/src/lib.rs + entrypoints/jsonrpc/Cargo.toml + runtime/core/src/lib.rs + runtime/core/src/manager.rs - P4-3 review fixes v5: JSON-RPC test_dispatch_tool_list_mcp_source_visible constructs CoreRuntimeManager with fake MCP tool via CoreRuntimeBuilder, asserts tool_list returns source=="mcp"; re-exported LlmClient from core-runtime; added runtime-tools and async-trait as dev-dependencies; fixed start_mcp_init comment to list all three config flags.

[2026-06-16 02:45] Alius: .alius/workspace/docs + entrypoints/jsonrpc/src/lib.rs - P4-3 review fixes v4: synced SPEC.md/01-current-state.md/implementation-gaps.md with full MCP startup condition (mcp feature + three tools.toml flags + servers.toml); fixed config-schema.md and config-manager.md to separate project config files from user-level MCP config.

[2026-06-16 02:30] Alius: runtime/core/src/manager.rs + runtime/core/src/runtime.rs + .alius/workspace/docs/modules/extensions.md - P4-3 review fixes v3: mcp_config_enabled uses workspace_root not process cwd; added load_on_workspace_start to MCP startup condition (all three flags required); added CoreRuntimeManager config gating tests (default disabled, all-true enabled, individual flag disabled, workspace_root vs cwd); CoreRuntime::tool_list test verifies MCP source metadata through CoreRuntimeBuilder; extensions.md clarifies project switch vs server declarations vs legacy mcp.json.

[2026-06-16 02:15] Alius: runtime/core/src/manager.rs + runtime/tools/src/mcp_bridge.rs + runtime/tools/src/registry.rs + entrypoints/jsonrpc/src/lib.rs + .alius/workspace/docs - P4-3 review fixes v2: MCP init gated by tools.toml config (registry.mcp_tools + mcp.register_as_tools must both be true); McpToolAdapter preview_confirmation returns true in Plan mode; added MCP source propagation tests (mcp_tool_has_mcp_source, mcp_and_native_sources_coexist, mcp_duplicate_name_rejected); synced extensions.md, config-manager.md, SPEC.md MCP config path.

[2026-06-16 02:00] Alius: runtime/tools/src/traits.rs + runtime/tools/src/registry.rs + runtime/tools/src/mcp_bridge.rs + runtime/core/src/runtime.rs + runtime/core/src/manager.rs + .alius/workspace/docs - P4-3 review fixes: added AliusTool::source() trait method (default RustWasm); McpToolAdapter overrides to ToolSource::Mcp; ToolRegistry::to_tool_infos() returns ToolInfo with source metadata; CoreRuntime::tool_list() uses to_tool_infos(); removed misleading mcp_tools() empty stub from CoreRuntimeManager; synced docs (01-current-state.md, implementation-gaps.md, config-schema.md) with actual MCP integration state.

[2026-06-16 01:45] Alius: runtime/tools/src/registry.rs + runtime/tools/src/native/mod.rs + runtime/tools/src/mcp_bridge.rs + runtime/core/src/mcp_manager.rs + runtime/core/src/manager.rs - P4-3 fix: ToolRegistry now uses interior RwLock so register() takes &self; MCP tools registered directly into shared ToolRegistry (visible via tool_list/to_tool_defs/LoopEngine); mcp_bridge Box::leak moved to constructor; McpManager::start_background_init takes Arc<ToolRegistry>.

[2026-06-16 01:30] Alius: runtime/tools/src/mcp_bridge.rs + runtime/core/src/mcp_manager.rs + runtime/core/src/manager.rs + .alius/workspace/SPEC.md - P4-3 MCP runtime tool bridge: McpToolAdapter wraps MCP tools as AliusTool; McpManager::register_tools creates adapters and skips duplicate names (native/WASM priority); no-config does not block runtime; SPEC.md F-010 updated.

[2026-06-16 01:15] Alius: entrypoints/jsonrpc/src/lib.rs - P4-2 review fix: test_run_cancel_observable_via_subscribe now asserts RunCancelled event is present in subscribe snapshot, not just correlation fields; fixed run_start doc comment to match actual return (removed turn_ref).

[2026-06-16 01:00] Alius: entrypoints/jsonrpc/src/lib.rs + .alius/workspace/SPEC.md + .alius/workspace/docs/products/jsonrpc.md - P4-2 JSON-RPC run control: run_start returns run_ref + trace_id + session_ref; run_subscribe returns full event fields (trace_id, run_ref, session_ref, turn_ref, kind, payload, sequence, created_at); run_cancel returns success; cancel observable via subscribe test; product docs updated with complete method table and error codes; 32 jsonrpc tests total.

## 2026-06-15

[2026-06-15 23:59] Alius: entrypoints/jsonrpc/src/lib.rs + entrypoints/jsonrpc/Cargo.toml + .alius/workspace/SPEC.md - P4-1 JSON-RPC runtime-backed protocol surface: serve() now creates CoreRuntimeManager with RuntimeManagerContext::json_rpc() and passes it to serve_with_runtime(); dispatch_with_runtime() expanded with model_list and tool_list methods; config_read returns real runtime config, not hardcoded; invalid_params helper added with -32602 error code; SPEC.md F-011 updated; 18 jsonrpc tests passing.

[2026-06-15 23:56] Codex: runtime/core/src/loop_engine/engine.rs + runtime/model/src/client.rs - P3-3 test hardening: upgraded Chat denial test to run LoopEngine::run and assert ToolConfirmationRequired -> ToolCallCompleted -> ErrorRaised(tool_denied) -> FinalResult(success=false); added hidden provider injection constructor for engine tests.

[2026-06-15 18:50] Alius: runtime/core/src/loop_engine/engine.rs - P3-3 Chat path denial parity: run_chat_with_tools now emits ErrorRaised(code: "tool_denied") before FinalResult(success: false) on batch denial, matching Plan path semantics; added chat_denial_batch_returns_denied test; 31 engine+tool_step tests total.

[2026-06-15 18:45] Alius: .alius/workspace/docs/modules/tools-and-shell-gate.md - P3-3 doc fix: corrected audit failure event from ErrorRaised to LogRecordEmitted to match implementation; clarified these are non-status-changing diagnostic events with monotonic sequences.

[2026-06-15 18:30] Alius: runtime/core/src/loop_engine/tool_step.rs + runtime/core/src/loop_engine/engine.rs + runtime/core/src/loop_engine/context.rs + runtime/core/src/runtime.rs - P3-3 structured denial, fail-fast, audit wiring: ToolBatchResult replaces Vec return — engine uses batch_denied field instead of fragile string matching; ConfirmationDecision enum (Approved/Denied/Cancelled/Unavailable); execute_tools aborts batch on first denial; LogWriter wired through LoopContext from CoreRuntime (workspace_ref.root/.alius/logs) into real Plan/Chat paths; audit_confirmation uses LogRecordEmitted (non-status-changing) on failure with monotonic sequence; engine-level plan_denial_produces_error_and_failed_final test; audit_failure_uses_log_record_emitted test verifies no ErrorRaised and no sequence=0; 30 engine+tool_step tests total.

[2026-06-15 18:00] Alius: runtime/core/src/session.rs + runtime/core/src/logging/audit.rs - P3-3 session terminal-state and audit: deliver_confirmation atomic check+update prevents cancel_run race; ConfirmationDecision reason preserved; get_run_status accessor; audit::log_confirmation; 11 session tests + 7 audit tests.

[2026-06-15 17:45] Alius: runtime/tools/src/package.rs + runtime/tools/src/registry.rs - P3-2 Native tools enter default ToolRegistry: build_registry always registers native tools first; register rejects duplicate names to prevent WASM shadowing; 17 tests.

[2026-06-15 17:30] Alius: runtime/tools/src/shell_gate + runtime/tools/src/native - P3-1 Shell Gate scope and boundary hardening: scope.rs redirection target detection; shell.rs cwd boundary; authorizer.rs boundary-before-risk reordering; 58 tests.

[2026-06-15 17:00] Alius: runtime/core + runtime/memory + protocol - P2 Runtime state persistence and cancellation: streaming events persist to SessionManager; RunStatus lifecycle auto-transitions; cancel_run triggers CancellationToken; Cancelled is terminal state; conversation messages persist to ConversationStore.

[2026-06-15 14:32] Codex: .alius/workspace/docs/standards/development-workflow.md - Documented reviewer-owned branch setup before task assignment and reviewer-owned merge decisions after all goals and tests pass.

[2026-06-15 14:29] Codex: .alius/workspace/docs/standards/development-workflow.md - Added the local feature-branch review workflow: developers submit functional branches, reviewers check design and functional acceptance, and accepted work is merged into the local master integration branch with commits allocated by function.

[2026-06-15 13:27] Codex: entrypoints/cli/src/tui/workspace - Wired runtime tool confirmation responses during normal execution, plan drafting, and approved plan-step execution loops.

[2026-06-15 10:30] Alius: entrypoints/cli/src/tui/workspace - Implemented TUI conversation block folding (3-line default with title+first-line merged) and Ctrl+O global expand/collapse toggle; added block ID tracking and click-to-toggle support; preserved Shift-based native terminal text selection for system right-click copy.

[2026-06-15 09:30] Alius: runtime/core + runtime/tools - Stage B B3+B6: tool_step pauses on preview_confirmation (Plan mode + risky op) — emits ToolConfirmationRequired, awaits oneshot; LoopContext gains session; Shell/WriteFile/EditFile implement preview_confirmation and execute path no longer self-gates Plan mode. B4 (bridge respond_confirmation — needs CoreRuntimeManager to hold session_manager) + B5 (TUI confirmation UI) pending.

[2026-06-15 08:46] Codex: runtime/core + runtime/model + protocol - Persisted tool result messages into conversation history so multi-step Bypass tool runs keep assistant(tool_calls) -> tool result ordering across iterations.

[2026-06-15 08:35] Codex: runtime/core + runtime/model + runtime/tools + entrypoints/cli - Documented and implemented Bypass shell tool protocol safeguards, git subcommand risk classification, and TUI tool started/completed status blocks.

[2026-06-15 08:10] Alius: protocol + runtime/core - Stage B confirmation foundation (B1-B2): CoreCommandKind::RespondToolConfirmation + CoreEventPayload::ToolConfirmation; SessionManager RunState.confirmation oneshot map + store_confirmation_sender/deliver_confirmation/cancel_pending_confirmations + WaitingForApproval status; unit tests pass. B3-B6 (loop pause/resume + TUI + wiring) pending — next session.

[2026-06-15 07:48] Codex: .alius/workspace/docs/modules/loop-engine.md - Documented Chat/Bypass tool-call message ordering for OpenAI-compatible APIs.

[2026-06-15 14:00] Alius: runtime/mcp + runtime/tools - Implemented MCP v2024-11-05 protocol client, tool bridge adapter, and CLI command scaffolds for server management. Core runtime integration pending.

[2026-06-15 17:14] Codex: .alius/workspace/SPEC.md, .alius/workspace/docs/modules/loop-engine.md - Clarified Chat/Bypass as a single user turn with bounded tool-call continuations, tools enabled, and planning disabled.

## 2026-06-13

[2026-06-13 23:41] Alius: entrypoints/cli + runtime/config - Added /config overview list as the landing section (status-marked entries for Models/Language/Soul + Save/Cancel) and made /init auto-create .alius on fresh workspaces without a project-structure prompt; existing .alius now offers only Reinitialize/Exit.

[2026-06-13 22:37] Alius: entrypoints/cli/src/tui/workspace - Extended raw-command echo suppression from config family to all slash commands, swapped saved-config emoji to checkmark, and localized /config tab labels and section prompts via rust-i18n.

[2026-06-13 22:03] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented status-only configuration feedback without raw command echo.

[2026-06-13 21:49] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented symmetric available-width expansion for bordered Welcome layouts.

[2026-06-13 21:43] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented final responsive TUI welcome layouts for Wide, Medium, Compact, and Tiny terminal sizes.

[2026-06-13 21:04] Codex: .alius/workspace - Documented startup welcome card, Git status hiding outside repositories, /init reset semantics, and side-panel-only /init progress output.

[2026-06-13 15:45] Codex: .alius/workspace - Documented /init completion after role configuration, removal of capability resolution and Enter Copilot confirmation, startup settings hydration, and Copilot/Team mode wording.

[2026-06-13 15:20] Codex: .alius/workspace - Documented /init API Key persistence for chat readiness, removal of workspace-template/final-validation steps, Copilot default completion, and Chinese UI wording from Soul role to role.

[2026-06-13 14:53] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented localized /init workflow rendering, hidden cwd/git footer metadata, and SOUL activation error handling.

[2026-06-13 10:36] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented /init workflow rendering in the right-side panel while Conversation records step results and errors.

[2026-06-13 10:29] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented that slash commands remain reachable before plan-draft continuation input so /init can start after missing-runtime errors.

[2026-06-13 10:05] Codex: .alius/workspace - Documented that non-model /config saves preserve the existing provider model library instead of clearing it from an empty task snapshot.

[2026-06-13 09:49] Codex: .alius/workspace - Documented immediate provider model-pool persistence after /model and /init model imports so /config sees the current pool.

[2026-06-13 08:42] Codex: .alius/workspace - Documented /init extraction into runtime-config InitWizard/project_init modules, resumable .alius/runtime/init-state.toml, and CLI adapter command execution.

[2026-06-13 01:30] Codex: .alius/workspace - Documented /init as an inline InitWizard state machine with operation-specific prompt scopes, model-pool import, Plan/Execute/Review assignment, capability resolution, workspace creation, and final validation.

[2026-06-13 01:10] Codex: .alius/workspace - Documented the three built-in model providers, OpenAI/Anthropic API mode selection in /model, and provider-specific Base URLs for BigModel GLM, Xiaomi MiMo, and DeepSeek.

[2026-06-13 00:41] Codex: .alius/workspace - Updated TUI/config documentation for Plan/Execute/Review assignment in /config, model-pool ownership in /model, plaintext API Key input, and model.toml compatibility migration.

## 2026-06-12

[2026-06-12 21:51] Codex: .alius/workspace - Corrected /config documentation to the three-section configuration flow, documented /model as the dedicated three-level model routing setup, and removed obsolete confirmation/output wording.

[2026-06-12 15:38] Codex: .alius/workspace - Documented the local tabbed /config center, project-local model library, explicit Add Model fetch flow, inline /model selector, and Reasoning Notes over light/medium/high router tiers.

[2026-06-12 14:56] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented responsive Plan drafting and Esc confirmation interrupt behavior while the model drafts clarification questions or plan proposals.

[2026-06-12 10:34] Codex: .alius/workspace - Corrected clarification prompt documentation: Conversation shows the question, while the interaction surface renders single-select, multi-select, or text controls for answers.

[2026-06-12 10:22] Codex: .alius/workspace - Documented choice-first confirmation surfaces with optional bottom reply input instead of custom reply as a normal choice.

[2026-06-12 10:11] Codex: .alius/workspace - Documented interactive model-controlled TUI Plan drafting, approval-gated Plans panel visibility, stepwise execution, and completion confirmation.

## 2026-06-11

[2026-06-11 00:00] Codex: .alius/workspace - Created the English authoritative workspace documentation set from the current Alius code baseline.

[2026-06-11 00:00] Codex: .alius/workspace - Documented Phase 1 Runtime Manager boundary: core-runtime owns local runtime assembly, while runtime-* crates remain managed subsystems.

[2026-06-11 00:00] Codex: .alius/workspace - Documented Phase 2 CLI/TUI compatibility cleanup: REPL no longer retains model client, agent, tool registry, or runtime-model conversation state for default execution.

[2026-06-11 00:00] Codex: .alius/workspace - Documented the in-workspace conversational /config task and its administration boundary.

[2026-06-11 00:00] Codex: .alius/workspace - Documented inline prompt input for /config options, custom values, and checkbox-capable future prompts.

[2026-06-11 00:00] Codex: .alius/workspace - Documented the tool runtime rule that all tools are implemented as Rust WASM modules, plus the long-term ABI, Soul selection, approval, audit, and distribution roadmap.

[2026-06-17 23:59] Codex: entrypoints/cli/src/workflow/mod.rs - G1 Workflow Runtime Hardening complete: on_failure policy (abort/skip/retry), timeout_ms, CancellationToken, StepResult timing metadata, WorkflowRunRecord persistence, tool mode propagation, schema docs, 3 example workflows, 30 tests.

[2026-06-17 23:59] Codex: runtime/tools/src/wasm_host/host.rs - G2 Plugin Install Authorization complete: permission summary display, --yes flag, upgrade detection with permission change warning, install-time confirmation prompt.

[2026-06-17 23:59] Codex: runtime/tools/src/wasm_host/imports.rs - G3 Fetch Host Import complete: real HTTPS execution via reqwest, HTTPS-only enforcement, 10s timeout, 1MB body limit, permission check, audit logging.

[2026-06-17 23:59] Codex: runtime/core/src/session.rs, runtime/core/src/logging/log_writer.rs - G4 Event Persistence complete: CoreEvent auto-persisted to events.jsonl via LogWriter sink, get_events falls back to disk for restart recovery.

[2026-06-17 23:59] Codex: runtime/core/src/manager.rs - G5 Manager Boundary Hardening complete: workspace_root() and tool_registry() narrow accessors, runtime() marked doc-hidden, workflow and MCP paths migrated.

[2026-06-17 23:59] Codex: runtime/tools/src/policy.rs - G6 Permission Policy Matrix complete: evaluate_policy(source, mode, risk) unified function, 36-cell matrix for Native/WASM/MCP x Chat/Plan/Bypass, ShellGate authorize returns RiskLevel, docs/permissions.md, 11 tests.

[2026-06-17 23:59] Codex: entrypoints/jsonrpc/src/lib.rs - E1 legacy dispatch() stub marked #[cfg(test)].

[2026-06-17 23:59] Codex: runtime/core/src/runtime.rs - E2 start() method documented as non-cancellation-capable.
