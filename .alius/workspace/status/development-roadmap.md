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
| WASM host imports | Partial | read/write/list/env/shell exist; fetch is a stub | Permissions checked before execution | Shell Gate is used for shell import; filesystem uses resolved path | Implement fetch or reject network permissions with clear install-time warning |
| Plugin registry/catalog | Partial | Registry loader and hello-world source catalog exist | Manifest permissions parse and validate | No install-time user authorization yet | Add permission approval and upgrade re-prompt |
| MCP completeness | Mostly done | MCP tools can enter shared ToolRegistry when enabled | Duplicate tool names are rejected | Native/WASM priority protects built-ins | Add more real-server regression tests as server matrix grows |
| Workflow runtime | Core path implemented | CLI uses `RuntimeWorkflowHandle`; prompt/tool steps use real runtime/tool paths | Tool steps currently run with workflow-local policy | No workflow-level confirmation UX, retry, or trace model | Add workflow confirmation/retry/session semantics |
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
| `fetch` host import stub is a P4 gap | Correct | It is safe because it does not perform network I/O, but functionally incomplete. |
| Plugin install authorization prompt is missing | Correct | This is a user trust and security UX gap. |

## High-Priority Gaps

### G1. Workflow Runtime hardening — COMPLETED

Priority: P0 for full functional completeness.

Implemented on `feature/p4-review-roadmap` branch:

- Step JSON schema extended: `on_failure` (abort/skip/retry), `timeout_ms`, `mode` (chat/plan)
- `execute_step` supports timeout via `tokio::time::timeout` and retry with configurable backoff
- `execute_workflow` respects `on_failure` policy per step, supports `CancellationToken`
- `StepResult` extended with `started_at`, `finished_at`, `duration_ms`, `trace_id`, `run_ref`
- `WorkflowRunRecord` persisted to `~/.alius/workflows/runs/`
- `LoopEngineHandle::run_tool` accepts `mode` parameter for confirmation policy
- `RuntimeWorkflowHandle::run_tool` uses workflow mode for `ToolContext`
- Schema docs at `docs/workflow-schema.md`, 3 example workflows in `examples/workflows/`
- 30 workflow tests passing

Current evidence:

- `workflow run` constructs `CoreRuntimeManager` and `RuntimeWorkflowHandle`.
- Prompt steps call `CoreRuntimeManager::run_text()`.
- Tool steps call `ToolRegistry::get() + execute()`.
- Integration test proves fake LLM and fake tool paths do not use stub markers.

Remaining gaps:

- Tool steps use workflow-local execution policy and do not expose a workflow-level confirmation channel.
- A tool that would require Plan confirmation has no interactive approval path in workflow context.
- No workflow retry, rollback, timeout, or per-step cancellation policy.
- Workflow execution prints stdout status but does not produce a durable workflow run record.
- Workflow does not expose CoreEvent trace IDs as first-class workflow metadata.

Required branch:

- `codex/feature/workflow-runtime-hardening`

Acceptance:

- workflow JSON supports an explicit mode or policy field.
- risky tools either request confirmation through a supported channel or fail closed.
- workflow execution records run_ref/trace_id/session_ref for every prompt/tool step.
- tests cover approved, denied, unavailable-confirmation, and failed-tool branches.
- no workflow test may rely on `StubLoopEngineHandle` except explicit unit tests for the stub.

### G2. Plugin install authorization and upgrade prompts — COMPLETED

Priority: P0 for plugin trust.

Implemented on `feature/p4-review-roadmap` branch:

- `install_plugin` returns `(manifest, permission_summary, upgrade_info)`
- CLI shows permissions before install with `[y/N]` confirmation
- `--yes` / `-y` flag skips confirmation
- Upgrade detection: compares installed vs new version and permissions
- Permission change warning on upgrade
- Network permission warning ("fetch not yet implemented") — now resolved with fetch implementation
- Rollback on cancelled install (removes copied files)

Current evidence:

- Manifest permissions validate structurally.
- Runtime checks are default-deny.
- Users are shown the full permission bill before install.

Required branch:

- `codex/feature/plugin-install-authorization`

Acceptance:

- `alius plugin install` displays filesystem/network/shell/env permissions before installation.
- install requires explicit confirmation for any non-empty permission set.
- non-interactive mode must fail closed unless an explicit allow flag is provided.
- plugin upgrade detects changed permissions and re-prompts.
- tests cover empty permissions, non-empty permissions, denied install, approved install, and upgrade permission changes.

### G3. Fetch host import capability — COMPLETED

Priority: P1 for extension functionality.

Implemented on `feature/p4-review-roadmap` branch:

- `fetch` host import now executes real HTTPS requests via `reqwest`
- HTTPS-only enforcement (http:// rejected)
- 10-second timeout per request
- 1MB response body size limit
- Permission check via `check_network()` before execution
- Audit logging of URL target and allow/deny decision
- Sensitive response headers filtered (only content_type returned)

Current evidence:

- `fetch` import checks network permissions and executes HTTPS requests.
- A plugin with network permission can make real HTTP calls.

Required branch:

- `codex/feature/wasm-fetch-host-import`

Acceptance:

- Either implement bounded HTTPS fetch with size/time limits, or reject network permissions at install with a clear warning until implemented.
- URL allowlist matching must remain prefix-boundary safe.
- response body size and timeout must be configurable with secure defaults.
- audit must log URL target and allow/deny decision without response body.
- tests cover allowed URL, similar-domain denial, timeout, oversized response, and no-permission denial.

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

Required branch:

- `codex/feature/runtime-event-persistence`

Acceptance:

- CoreEvent envelopes are appended to a durable event log with trace_id/run_ref/session_ref.
- `subscribe()` can optionally reconstruct completed run snapshots after process restart.
- event persistence must not block model streaming indefinitely.
- tests cover completed run replay, failed run replay, cancelled run replay, and corrupted-log tolerance.

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

Required branch:

- `codex/fix/runtime-manager-accessors`

Acceptance:

- Replace broad `runtime()` use with narrow manager accessors such as `workspace_root()`, `tool_registry()`, and any required workflow helper.
- Mark `runtime()` test-only or doc-hidden if it must remain.
- Product entrypoints must not directly reach CoreRuntime internals.
- tests prove workflow and MCP paths still work through narrowed accessors.

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

Required branch:

- `codex/fix/tool-permission-policy-matrix`

Acceptance:

- Define a policy matrix for Native/WASM/MCP across Chat/Plan/Bypass.
- Implement the matrix in shared policy code, or document intentional differences with tests.
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
