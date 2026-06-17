# P5 Review Blockers Task Card

Date: 2026-06-17
Branch: `fix/p5-review-blockers`
Base candidate: `feature/p4-review-roadmap`

## Review Result

The candidate commit `feat(P5): close G1-G6 implementation gaps` passes the mechanical quality gates, but it does not pass strict functional review. The implementation closes several important code paths, but some completion claims are broader than the actual behavior.

This task card replaces the previous G1-G6 follow-up card. Development must focus only on the blockers below.

## Quality Gate Baseline

The following commands passed on the base candidate:

- `cargo fmt --all -- --check`
- `cargo check --workspace --all-targets --all-features`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace -- --test-threads=1`

Passing these again is required but not sufficient. Functional acceptance below is mandatory.

## Completion Status By Item

| Item | Candidate status | Review verdict | Reason |
| --- | --- | --- | --- |
| G1 Workflow Runtime hardening | Partial | Not accepted | Step retry and per-step timeout exist, but workflow-level timeout is not enforced, prompt mode is hardcoded to Plan, and run records do not capture runtime trace/run references. |
| G2 Plugin install authorization | Partial | Not accepted | Confirmation happens after copying files; denied upgrades remove the existing installed plugin. Cancellation also exits through a panic-style top-level error path. |
| G3 Fetch host import | Partial | Not accepted | Real HTTPS fetch exists, but validation still warns that fetch is not implemented and there are no functional tests for allowed fetch, timeout, oversized response, or audited success. |
| G4 Durable CoreEvent persistence | Partial | Not accepted | Events are written to `events.jsonl`, but restart replay/corrupted-log behavior is not tested, and persistence is synchronous on the streaming event path. |
| G5 Runtime Manager boundary hardening | Mostly complete | Accepted with follow-up | Product paths use narrow accessors; `runtime()` remains doc-hidden for integration code/tests. No immediate blocker. |
| G6 Permission policy consistency | Partial | Not accepted | A policy matrix exists, but Native shell does not enforce it in Chat mode; existing tests still assert high-risk Chat shell commands do not require confirmation. |

## Required Fixes

### 1. Enforce workflow semantics end to end

Scope:

- `entrypoints/cli/src/workflow/mod.rs`
- `docs/workflow-schema.md`
- workflow examples if schema or behavior changes

Requirements:

- `execute_step_inner()` must pass the actual workflow mode to prompt steps. Do not hardcode `"Plan"`.
- `workflow.timeout_ms` must be enforced as a whole-workflow timeout, not merely deserialized.
- `WorkflowRunRecord` must persist enough runtime identity to correlate workflow steps with CoreEvent streams.
- Prompt steps using `CoreRuntimeManager::run_text()` must capture `run_ref` and `trace_id` from the returned event envelopes, or the design must remove those fields and stop claiming trace correlation.
- Tool steps that require confirmation in workflow context must either use a supported confirmation channel or fail closed with a clear error. Silent execution is not acceptable for risky tools.

Acceptance tests:

- workflow with `mode: "chat"` proves prompt steps call `run_prompt(..., "chat")`.
- workflow with `mode: "plan"` proves prompt/tool steps receive Plan mode.
- workflow-level `timeout_ms` cancels or fails a long-running workflow even if the current step has no step timeout.
- saved `WorkflowRunRecord` includes non-empty `run_ref` and `trace_id` for prompt steps that entered CoreRuntime.
- workflow Plan-mode risky shell tool without confirmation channel fails closed.

### 2. Make plugin install authorization pre-install and upgrade-safe

Scope:

- `entrypoints/cli/src/main.rs`
- `entrypoints/cli/src/plugin/mod.rs`
- `runtime/tools/src/wasm_host/host.rs`

Requirements:

- Split plugin install into validate/plan/apply phases.
- Permission summary and upgrade warning must be shown before any file is copied into `~/.alius/plugins`.
- If a user denies an upgrade, the previously installed plugin must remain intact.
- Denied installs must return a clean CLI error, not a panic message from `main()`.
- Non-interactive install must fail closed unless `--yes` is explicitly provided.
- Install must validate the WASM module before copying.

Acceptance tests:

- empty-permission plugin installs without prompt.
- non-empty-permission plugin prompts before copy.
- denied fresh install leaves no plugin directory.
- denied upgrade preserves old `plugin.toml` and `plugin.wasm`.
- changed-permission upgrade prompts even when a plugin with the same id already exists.
- invalid `plugin.wasm` is rejected before install.

### 3. Finish fetch host import behavior and tests

Scope:

- `runtime/tools/src/wasm_host/imports.rs`
- `runtime/tools/src/wasm_host/host.rs`
- `runtime/tools/src/wasm_host/audit.rs`
- `docs/permissions.md`
- `.alius/workspace/docs/modules/plugin-permissions.md`

Requirements:

- Remove stale warning text that says fetch is not implemented.
- Fetch must remain HTTPS-only.
- Fetch must enforce URL permission boundaries before network I/O.
- Fetch must enforce timeout and response size limits.
- Audit must record allow/deny, URL target, and reason without response body.
- Tests must not depend on external public internet. Use a local HTTPS test server if feasible, or isolate request execution behind a testable client abstraction.

Acceptance tests:

- allowed URL succeeds and records allow audit.
- no network permission denies before request.
- similar-domain URL is denied.
- `http://` URL is denied.
- timeout returns controlled error and audit denial.
- oversized response returns controlled error and audit denial.

### 4. Prove durable CoreEvent replay

Scope:

- `runtime/core/src/session.rs`
- `runtime/core/src/logging/log_writer.rs`
- `runtime/core/src/runtime.rs`

Requirements:

- Keep writing CoreEvent envelopes with `trace_id`, `run_ref`, and `session_ref` where available.
- Add a restart-style test that creates one SessionManager/runtime, writes events, then constructs a fresh manager over the same workspace and reads the events back.
- Add corrupted-line tolerance tests for `events.jsonl`.
- Decide whether synchronous disk writes on the streaming hot path are acceptable. If not, move to buffered/asynchronous persistence with bounded failure behavior.

Acceptance tests:

- completed run replay after fresh manager/runtime construction.
- failed run replay after fresh manager/runtime construction.
- cancelled run replay after fresh manager/runtime construction.
- corrupted `events.jsonl` lines are skipped without breaking valid events.

### 5. Enforce or revise the permission policy matrix

Scope:

- `runtime/tools/src/policy.rs`
- `runtime/tools/src/native/shell.rs`
- `runtime/tools/src/shell_gate/authorizer.rs`
- `runtime/core/src/loop_engine/tool_step.rs`
- `runtime/tools/tests/native_registry.rs`
- `runtime/tools/tests/shell_gate_integration.rs`
- `docs/permissions.md`

Requirements:

- The documented matrix and actual tool behavior must match.
- If Native Chat High is `Confirm`, then `Shell::preview_confirmation()` must return true for high-risk workspace-internal commands in Chat mode, and the loop/tool layer must not execute it without confirmation.
- If product design decides Chat should directly execute user-authored high-risk native shell commands, then update `policy.rs`, `docs/permissions.md`, and tests to state that explicitly. Do not leave code and docs contradictory.
- Add end-to-end tests for `rm -rf ./build` in Native Chat and Plan modes.

Acceptance tests:

- `evaluate_policy()` expectations match `Shell::preview_confirmation()` behavior.
- Native shell high-risk workspace-internal command has the expected Chat behavior.
- external paths remain denied in every mode.
- WASM medium/high/critical shell remains fail-closed.
- MCP Plan medium/high confirmation behavior is covered or explicitly documented as deferred if no risk classifier exists for MCP.

## Documentation Fixes

Update these documents after code fixes:

- `.alius/workspace/status/development-roadmap.md`
- `.alius/workspace/docs/overview/implementation-gaps.md`
- `.alius/workspace/docs/modules/extensions.md`
- `.alius/workspace/docs/modules/plugin-permissions.md`
- `docs/permissions.md`
- `docs/workflow-schema.md`
- `.alius/workspace/HISTORY.md`

The roadmap must not mark an item as completed while keeping the old "Remaining gaps" and "Required branch" text below it.

## Prohibited Changes

- Do not weaken workspace boundary checks.
- Do not remove confirmation requirements merely to make tests pass.
- Do not rely on external internet services in unit tests.
- Do not put unrelated model routing, memory UX, or config UX work into this branch.
- Do not squash all fixes into one large unstructured commit if the changes are separable.

## Required Verification

Run all commands before resubmitting review:

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace -- --test-threads=1
```

Also run functional checks or equivalent integration tests for:

- plugin denied upgrade preserves the old plugin
- workflow-level timeout works
- workflow mode is propagated to prompt and tool steps
- Native Chat high-risk shell behavior matches `docs/permissions.md`
- durable CoreEvent replay works after reconstructing the runtime/session manager
