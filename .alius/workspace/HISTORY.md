# Documentation History

All entries use the format:

```text
[YYYY-MM-DD HH:MM] [author]: [path] - [summary]
```

## 2026-06-15

[2026-06-15 18:30] Alius: runtime/core/src/loop_engine/tool_step.rs + runtime/core/src/loop_engine/engine.rs + runtime/core/src/loop_engine/context.rs + runtime/core/src/runtime.rs - P3-3 structured denial, fail-fast, audit wiring: ToolBatchResult replaces Vec return — engine uses batch_denied field instead of fragile string matching; ConfirmationDecision enum (Approved/Denied/Cancelled/Unavailable); execute_tools aborts batch on first denial; LogWriter wired through LoopContext from CoreRuntime into real Plan/Chat paths; audit_confirmation emits ErrorRaised on audit failure (non-blocking observable policy); engine-level plan_denial_produces_error_and_failed_final test; 30 engine+tool_step tests total.

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
