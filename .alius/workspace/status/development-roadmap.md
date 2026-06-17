# Alius Development Roadmap and Progress

Last updated: 2026-06-17 23:59
Review branch: `feature/p4-review-roadmap`

This document is the living project roadmap for the current Alius CLI implementation. It tracks actual code-backed progress, known gaps, and the next feature/fix branches. Function completeness has priority over permission completeness and security hardening, but no feature can be accepted if it weakens the workspace boundary or tool authorization model.

## Review Baseline

Current candidate work covers the P4 extension-system maturity batch:

- bundled official souls under `extensions/souls`
- `extensions/registry.toml`
- WASM manifest permissions and runtime matchers
- WASM host imports and host audit sink
- MCP registration and source tagging
- JSON-RPC run control and tool confirmation response
- Workflow parser plus runtime-backed handle
- documentation sync for current implementation state

The current working tree is intentionally uncommitted. Future issue details and follow-up work should be handled on separate feature/fix branches, with reviewer-owned branch creation and merge decisions.

## Progress Dashboard

Percentages are management estimates based on code evidence, functional acceptance, permission coverage, and security posture. They are not line or branch coverage metrics.

| Stage | Scope | Status | Overall | Function | Permission | Security | Merge posture |
| --- | --- | --- | ---: | ---: | ---: | ---: | --- |
| P1 | Runtime Manager boundary | Mostly complete | 85% | 90% | 80% | 80% | Accept with follow-up hardening |
| P2 | Runtime events, state, cancellation, sessions | Mostly complete | 82% | 85% | 75% | 80% | Accept with event persistence follow-up |
| P3 | Tool confirmation and Shell Gate | Mostly complete | 88% | 85% | 88% | 92% | Accept with policy consistency follow-up |
| P4 | Extension system maturity | Core path complete, maturity gaps remain | 86% | 84% | 86% | 88% | Accept only with explicit P5 follow-up plan |
| P5 | Router, memory, config UX, mature extensions | Not started as a stage | 15% | 15% | 10% | 20% | New planning required |

## Current P4 Completion

| P4 item | Current status | Function assessment | Permission assessment | Security assessment | Required follow-up |
| --- | --- | --- | --- | --- | --- |
| Monorepo extension catalog | Done | `soul update`, `soul install`, and local catalog work from bundled assets | No special permission model needed | Removes default network dependency | Add release packaging checks for bundled assets |
| WASM permission matcher | Done | Four domain matchers exist | Filesystem/network/shell/env are modeled separately | Workspace boundary and env wildcard rejection covered | Keep matcher and host execution path unified |
| Host audit sink | Partial | Host calls emit structured audit events | Records allow/deny decisions | Sensitive values are not logged | Feed host audit into runtime trace/session review |
| WASM host imports | Done | read/write/list/env/shell/fetch all implemented; HTTPS-only, timeout, size limit | Permissions checked before execution | Shell Gate is used for shell import; filesystem uses resolved path | — |
| Plugin registry/catalog | Done | Registry loader and hello-world source catalog exist | Manifest permissions parse and validate | Install-time authorization with two-phase plan/apply | — |
| MCP completeness | Mostly done | MCP tools can enter shared ToolRegistry when enabled | Duplicate tool names are rejected | Native/WASM priority protects built-ins | Add more real-server regression tests as server matrix grows |
| Workflow runtime | Done | CLI uses `RuntimeWorkflowHandle`; prompt/tool steps use real runtime/tool paths | Tool steps check preview_confirmation; fail-closed without confirmation channel | Mode/timeout/trace all wired | — |
| JSON-RPC surface | Mostly done | Eight methods are runtime-backed | Confirmation response method exists | Event serialization is still generic | Expand event variant coverage and long-running run semantics |
| Flaky test fix | Done | Serial tests pass | N/A | Reduces review uncertainty | Keep single-threaded full test gate |
| Documentation sync | Partial | Current state docs improved | Gaps are documented | History must avoid overstating maturity | Do not call P4 fully mature without P5 follow-up acceptance |

## Review Report Cross-Check

The attached four-stage review report is mostly directionally correct, but it should be interpreted against the current code state:

| Report claim | Assessment | Notes |
| --- | --- | --- |
| P1 Runtime Manager boundary is largely complete | Reasonable | The `runtime()` escape hatch concern is valid and should become a P5 hardening task. |
| JSON-RPC legacy `dispatch()` stub is risky | Reasonable | It is deprecated but callable. It should be removed or made obviously test-only in a breaking window. |
| P2 run events are memory-only | Partly correct | Subscribe works within process lifetime, but CoreEvent streams are not durable across process restart. Persistent event replay remains a real gap. |
| `start()` non-streaming cancellation is weaker than streaming | Reasonable | Treat as a follow-up unless product paths rely on cancellable non-streaming runs. |
| P3 Native shell and WASM shell differ on `ApprovalRequired` | Reasonable | This may be intentional trust-boundary behavior, but it must be documented as policy, not accidental inconsistency. |
| P4 Workflow Runtime is complete | Too broad | Current code is runtime-backed, but workflow-level confirmation, retry/error recovery, and persistence are incomplete. Call it "runtime-backed core path implemented", not "complete workflow engine". |
| `fetch` host import stub is a P4 gap | Now resolved | Fetch is fully implemented with HTTPS-only, timeout, size limit, and audit logging. |
| Plugin install authorization prompt is missing | Now resolved | Two-phase plan/apply with permission display and upgrade detection implemented. |

## High-Priority Gaps

### G1. Workflow Runtime hardening — COMPLETED

Priority: P0 for full functional completeness.

Implemented on `feature/p4-review-roadmap` branch, hardened on `fix/p5-review-blockers`:

- Step JSON schema extended: `on_failure` (abort/skip/retry), `timeout_ms`, `mode` (chat/plan)
- `execute_step` supports timeout via `tokio::time::timeout` and retry with configurable backoff
- `execute_workflow` respects `on_failure` policy per step, supports `CancellationToken`
- `StepResult` extended with `started_at`, `finished_at`, `duration_ms`, `trace_id`, `run_ref`
- `WorkflowRunRecord` persisted to `~/.alius/workflows/runs/`
- `LoopEngineHandle::run_tool` accepts `mode` parameter for confirmation policy
- `RuntimeWorkflowHandle::run_tool` checks `preview_confirmation()` and fails closed when confirmation is required (no interactive channel in workflows)
- Schema docs at `docs/workflow-schema.md`, 3 example workflows in `examples/workflows/`
- 31 workflow tests passing

Current evidence:

- `workflow run` constructs `CoreRuntimeManager` and `RuntimeWorkflowHandle`.
- Prompt steps call `CoreRuntimeManager::run_text()`.
- Tool steps call `ToolRegistry::get()`, check `preview_confirmation()`, then `execute()`.
- Integration test proves fake LLM and fake tool paths do not use stub markers.
- Confirmation-required tool test proves fail-closed behavior in both chat and plan modes.

Remaining gaps:

- (none — all P4 gaps resolved)

### G2. Plugin install authorization and upgrade prompts — COMPLETED

Priority: P0 for plugin trust.

Implemented on `feature/p4-review-roadmap` branch, hardened on `fix/p5-review-blockers`:

- `install_plugin` returns `(manifest, permission_summary, upgrade_info)`
- CLI shows permissions before install with `[y/N]` confirmation
- `--yes` / `-y` flag skips confirmation
- Upgrade detection: compares installed vs new version and permissions
- Permission change warning on upgrade
- Two-phase install: `plan_plugin_install` (validate) + `apply_plugin_install` (copy)
- `plan_plugin_install` validates WASM module structure via `validate_wasm_module()`
- CLI top-level error handling: clean error message on cancel/denial (no panic)
- Non-interactive detection: fails closed if stdin is not a TTY and `--yes` not provided

Current evidence:

- Manifest permissions validate structurally.
- WASM module structure validated at install time (invalid bytes rejected).
- Runtime checks are default-deny.
- Users are shown the full permission bill before install.
- CLI prints clean "Error: ..." and exits(1) on any failure, no panic.

### G3. Fetch host import capability — COMPLETED

Priority: P1 for extension functionality.

Implemented on `feature/p4-review-roadmap` branch, tested on `fix/p5-review-blockers`:

- `fetch` host import now executes real HTTPS requests via `reqwest`
- HTTPS-only enforcement (http:// rejected)
- 10-second timeout per request
- 1MB response body size limit
- Permission check via `check_network()` before execution
- Audit logging of URL target and allow/deny decision
- Sensitive response headers filtered (only content_type returned)
- Execution-level tests: success, server error, oversized response, connection refused
- WASM integration tests: HTTP rejection, no-permission, undeclared domain, allowed URL audit

Current evidence:

- `fetch` import checks network permissions and executes HTTPS requests.
- A plugin with network permission can make real HTTP calls.
- `execute_fetch` tested with local TCP server for success/error/oversized paths.
- Full WASM pipeline tested: permission check → HTTPS enforcement → HTTP execution → audit.

### G4. Durable CoreEvent persistence — COMPLETED

Priority: P1 for observability and replay.

Implemented on `feature/p4-review-roadmap` branch:

- `LogWriter::append_core_event()` writes CoreEvent to `events.jsonl`
- `SessionManager::set_event_sink()` injects LogWriter for automatic persistence
- `push_event()` now writes to both in-memory buffer AND disk
- `get_events()` falls back to disk for process restart recovery
- `query_logs()` indirectly supports disk events via `get_events()` fallback

Current evidence:

- SessionManager keeps run events in memory AND persists to `events.jsonl`.
- ConversationStore persists conversation messages.
- audit log persists selected security events.

### G5. Runtime Manager boundary hardening — COMPLETED

Priority: P1 for architecture integrity.

Implemented on `feature/p4-review-roadmap` branch:

- `CoreRuntimeManager::workspace_root()` added as narrow accessor
- `CoreRuntimeManager::tool_registry()` added as narrow accessor
- `runtime()` marked `#[doc(hidden)]` with documentation guiding to narrow accessors
- Workflow module migrated: `self.manager.runtime().session_manager().workspace_root()` → `self.manager.workspace_root()`
- MCP init path migrated: `self.runtime().tool_registry()` → `self.tool_registry()`
- main.rs workflow path migrated to `manager.tool_registry()`

Current evidence:

- `CoreRuntimeManager::runtime()` is doc-hidden; product code uses narrow accessors.
- Workflow and MCP integration use narrow manager methods.

### G6. Permission policy consistency — COMPLETED

Priority: P1 for user predictability.

Implemented on `feature/p4-review-roadmap` branch:

- `runtime/tools/src/policy.rs` defines unified `evaluate_policy(source, mode, risk)` function
- Policy matrix covers Native/WASM/MCP × Chat/Plan/Bypass × Low/Medium/High/Critical
- WASM medium+ denied (no interactive confirmation channel)
- Native High confirms, Native Critical denies
- MCP Medium confirms in Plan mode
- Bypass mode always allows
- `ShellGate::authorize()` now returns `(ShellGateDecision, RiskLevel)` for policy integration
- Policy docs at `docs/permissions.md`
- 11 policy matrix tests covering all 36 cells

Current evidence:

- Native/WASM/MCP tools use unified policy matrix via `evaluate_policy()`.
- Policy is documented and tested.
- Tests cover `rm -rf ./build`, `/etc/passwd`, `../outside`, redirection, and `--output=/tmp/out` across tool sources and modes.

## Development Path

| Order | Branch | Goal | Exit criteria |
| ---: | --- | --- | --- |
| 1 | `codex/feature/workflow-runtime-hardening` | Close workflow functional gaps | confirmation/failure/session semantics tested |
| 2 | `codex/feature/plugin-install-authorization` | Add install and upgrade permission UX | third-party plugin install is explicit and fail-closed |
| 3 | `codex/feature/wasm-fetch-host-import` | Make network permission meaningful | bounded fetch works or permission is rejected clearly |
| 4 | `codex/feature/runtime-event-persistence` | Durable CoreEvent replay | completed run can be reconstructed after restart |
| 5 | `codex/fix/runtime-manager-accessors` | Remove broad runtime escape hatch | product paths use narrow manager methods |
| 6 | `codex/fix/tool-permission-policy-matrix` | Normalize permission behavior | mode/source matrix documented and tested |
| 7 | `codex/feature/jsonrpc-event-contract` | Strengthen remote protocol surface | event variants and errors have stable JSON contract |
| 8 | `codex/feature/model-routing-memory-config` | Continue P5 product maturity | router, memory retrieval, config UX are product-verified |

## Review Gate For Future Branches

Every future branch must include:

- scope statement with one main objective
- feature acceptance tests, not only unit tests
- permission and safety tests when tools, filesystem, shell, network, or plugins are touched
- documentation update in `.alius/workspace/docs/` or `.alius/workspace/status/`
- `HISTORY.md` entry
- quality gates:
  - `cargo fmt --all -- --check`
  - `cargo check --workspace --all-targets --all-features`
  - `cargo clippy --workspace --all-targets --all-features -- -D warnings`
  - `cargo test --workspace -- --test-threads=1`

## Current Acceptance Recommendation

The current P4 candidate can be accepted only as a scoped checkpoint if the following statement is approved:

> P4 completes the extension-system core path. Workflow runtime hardening, fetch HTTP execution, plugin install authorization, audit trace persistence, and manager boundary hardening are explicitly deferred to P5 follow-up branches.

If that statement is not accepted, P4 remains incomplete because workflow and plugin functionality are not fully mature.
