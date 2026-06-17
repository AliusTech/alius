# TUI Workspace

The default interactive Alius experience is a Plan-driven Agent Runtime Workspace. It is not intended to be described as a simple terminal chatbot.

## Entrypoint

`entrypoints/cli/src/repl/mod.rs` exposes `run_repl(settings)`.

Current behavior:

- Without `ALIUS_LEGACY_REPL=1`, Alius enters the Ratatui workspace.
- With `ALIUS_LEGACY_REPL=1`, Alius enters the older `rustyline` REPL.

The workspace implementation lives under `entrypoints/cli/src/tui/workspace/`.

## Interaction Contract

| Key | Behavior |
| --- | --- |
| `Shift+Tab` | Switch between Plan mode and Bypass mode. |
| `Ctrl+Enter` | Submit or execute the current input. |
| `Ctrl+Tab` | Switch Conversation and Agent Team tabs when Agent Team is visible. |
| `Esc` | Clear input or cancel the current prompt or confirmation state. |
| `Ctrl+C` / `Ctrl+D` | Exit. |

Keep these bindings stable unless product design and code are changed together.

During an active configuration task, the input surface stays inside the main workspace:

| Key | Behavior |
| --- | --- |
| `Up` / `Down` | Move within the current inline option picker. |
| `Tab` / `Shift+Tab` | Switch configuration tabs. |
| `Space` | Toggle the highlighted checkbox in multi-select prompt input. |
| `Enter` | Send the selected or typed answer for validation. |
| `Esc` | Exit configuration immediately when unchanged; show an exit confirmation when unsaved changes exist. |

`Shift+Tab` does not switch Plan and Bypass modes while the configuration task is active.

## Main Areas

| Area | Purpose |
| --- | --- |
| Top bar | Version, soul, mode, and network status. |
| Conversation | Workflow blocks for request, understanding, execution, prompt, output, and errors. |
| Plans | Plan nodes, status, acceptance criteria, evidence, and ownership. |
| Interaction surface | Text input, inline single-select, inline multi-select, custom input, or approval controls. |
| Status bar | Current workspace, git status, and runtime state. |

Every workspace launch starts with a dedicated welcome block in the Conversation area. The block is not rendered as an `Output` block and does not show Agent Workspace copy, Protocol details, operation menus, release notes, or help tips.

The welcome block has four responsive layouts:

- `Wide`: `width >= 100 && height >= 14`, full logo, version, `慎始如终`, and a right column for `SOUL`, `Plan`, `Execute`, `Review`, and the Enter hint. Model names include provider wrappers such as `BigModel(glm-4.5-coding)`.
- `Medium`: `72 <= width < 100 && height >= 12`, medium logo and the same information hierarchy. Model names are compacted to the provider-native model name.
- `Compact`: `46 <= width < 72 && height >= 12`, centered small logo card with `SOUL`, `Plan`, `Execute`, `Review`, and the Enter hint stacked below.
- `Tiny`: `width < 46 || height < 12`, plain text without borders to avoid terminal wrapping errors.

All welcome layouts show `Version`, the fixed slogan `慎始如终`, `SOUL`, `Plan`, `Execute`, `Review`, and either `Press Enter to start` or `Press Enter to continue`. Unconfigured values render as `Not selected` or `Not configured`.

Bordered welcome layouts (`Wide`, `Medium`, and `Compact`) use the available conversation width instead of a fixed card width. They keep the same left and right horizontal margin at every terminal size, so resizing wider expands the welcome border and available content area symmetrically.

When the current directory is not inside a Git repository, the status bar shows only `cwd` and does not render placeholder repo or branch fields.

## Modes

| Mode | Purpose |
| --- | --- |
| Plan | Clarify task details with the model, generate a plan only after the task and preconditions are understood, ask for approval, then execute approved plan nodes step by step. |
| Bypass | Send input directly to execution without local plan review. |

Plan mode is not a fixed local sequence such as "understand objective, decompose, approve." The TUI keeps a draft planning state while the model asks clarifying questions. The Plans panel is hidden until the model returns a plan proposal and the user approves it. After approval, the approved nodes appear in the Plans panel and are executed one by one. When every node is complete, the user confirms plan completion and the Plans panel closes.

Clarification prompts should minimize typing. The Conversation area displays the question, such as "What does this interface do?" The interaction surface displays model-proposed answers as single-select or multi-select options, such as "User login" and "Data query." Free-form input is used only when the model cannot safely reduce the question to choices.

While the model is drafting a clarification question or plan proposal, the workspace keeps the TUI event loop responsive and uses the same `Esc` confirmation interrupt flow as execution.

Current plan nodes are still represented in TUI state. Core event reduction through `PlanProposed`, `PlanStepStarted`, `PlanStepCompleted`, and `PlanCompleted` remains the target direction for deeper runtime integration.

## Conversation Blocks

The workspace should use workflow language rather than traditional User/Assistant framing.

Common block kinds:

- Request
- Understanding
- Plan Proposal
- Execution
- Prompt
- Status
- Output
- Error

Core `ToolCallStarted` and `ToolCallCompleted` events render as Status/Error
blocks in the Conversation area. Shell calls use the command as the visible
label, for example `shell: git clone ...`, and completion blocks include a
short sanitized output summary. These tool progress blocks are UI-only and must
not be inserted into the model conversation between assistant `tool_calls` and
tool results.

## Conversation Block Folding

Long conversation blocks (more than 3 logical lines including the title) are automatically folded:

- **Title Line Format**: The title and first content line are merged: `○ Title First line content`
- **Collapsed Display**: Shows title+first-line merged, second line, and third line with fold hint
- **Fold Hint**: Appears at the end of the third line: `… 点击展开 / Ctrl+O 全部展开`
- **Click to Toggle**: Click anywhere on a folded block to expand/collapse it
- **Global Toggle**: `Ctrl+O` expands all foldable blocks; press again to restore default folding
- **Non-Foldable**: Welcome, ConfigOverview, and empty Execution (loading) blocks are never folded
- **New Content**: Defaults to folded unless global expand is active

## Text Selection and Copy

- **Native Selection**: Hold `Shift` to temporarily release mouse capture for terminal native text selection
- **Right-Click Menu**: Use the terminal/system native right-click context menu to copy
- **No Built-in Menu**: Alius does not show its own right-click menu or "copied" status
- **Fallback**: If the terminal doesn't support native right-click, use Shift + system copy shortcut (Cmd+C / Ctrl+Shift+C)

## Workspace Configuration Center

Typing `/config` starts a local tabbed configuration center in the workspace instead of opening a separate full-screen page or calling the model.

The available tabs are:

- `configuration-models`
- `configuration-language`
- `configuration-soul`

The Conversation area does not echo raw configuration commands such as `/config`, `/init`, or `/model`, and it does not echo selected option values as user requests. It records human-readable task status, current prompt context, validation feedback, fetch failures, and final success confirmation. The interaction surface renders the active option picker, multi-select list, text input, or exit confirmation.

Opening `/config` pushes a **configuration overview** block into the Conversation area and lands on the first section with a missing item (or `configuration-models` if all are satisfied). The overview block shows one row per section — `✓`/`○` status marker, the section label, and the current value (Execute model name, locale, role id) — and is **updated in place** (the most recent overview block is replaced, not duplicated) whenever the configuration changes. The interaction surface top renders a **section nav bar** — `配置 · 模型 · 语言 · 灵魂` — where "配置" is a fixed non-selectable title and the three sections are inline with the active one white-background-highlighted. `Tab`/`Shift+Tab` cycles Models → Language → Soul → Models and moves the highlight.

**Selections apply immediately.** There is no Save or Cancel button: choosing a model role, a language, or a role writes settings + `.alius/config/providers.toml` + `.alius/config/model.toml`, rebuilds the runtime bridge, and refreshes the overview — all at once, then keeps the task open for further edits. Esc exits silently (nothing is ever unsaved). The required-state list below is not gated by a save; it is satisfied progressively as the user picks values:

- At least one enabled model in the local model library.
- `Plan Model`, `Execute Model`, and `Review Model` each map to an enabled model.
- Selected models reference providers with a Base URL.
- Selected providers have a direct API key or API key environment source.
- A role is selected.
- A language is selected.

The `configuration-models` section assigns `Plan Model`, `Execute Model`, and `Review Model` from enabled entries in the local model library. It does not allow manual model-name, Base URL, or API key entry.

The `configuration-language` section selects the interface language. The `configuration-soul` section selects the active role.

`/model` (model pool) works the same apply-on-select way: Add/View/Delete persist immediately; there is no "Save Model Pool"/"Cancel" button, and Esc exits.

## Init Wizard

Typing `/init` **clears the Conversation area** (including the welcome block) — it is a reset flow — then starts an inline `InitWizard` state machine in the workspace instead of reusing the `/config` tab surface. The pure state machine lives in `runtime/config/src/init_wizard.rs`; project filesystem effects and resumable state persistence live in `runtime/config/src/project_init.rs`.

The visible flow is:

```text
INIT_START (skipped on fresh workspace — no Start confirmation)
  -> RESUME (only when .alius/runtime/init-state.toml exists)
  -> CHECK_WORKSPACE
  -> CREATE_PROJECT (skipped on fresh workspace; only Reinitialize/Exit when .alius exists)
  -> SELECT_LANGUAGE
  -> CONFIGURE_MODEL_POOL
  -> CONFIGURE_ASSIGNMENT
  -> CONFIGURE_ROLE
  -> COMPLETE
```

The right-side workflow panel renders the current `/init` state, configuration checklist, summaries, and import selections. It does not append cwd/git/footer metadata; that information belongs to the workspace status area. The Conversation area does not echo the `/init` command; it records a human-readable initialization status, errors, and the final check-mark completion message. Successful step progress stays in the right-side workflow panel instead of producing repeated `Output` blocks. The interaction surface renders the current state's action panel and uses operation-specific scopes such as `init-start`, `select-language`, `configure-model-pool`, `configure-assignment`, and `complete`.

On a fresh workspace (no `.alius`), `/init` skips the `INIT_START` confirmation and runs `CHECK_WORKSPACE` + `CREATE_PROJECT` (reset=false) automatically to reach `SELECT_LANGUAGE`, **without writing `.alius/runtime/init-state.toml`**. So if the user Escapes before submitting any answer, `.alius` exists (the skeleton) but no resume state is left — the next `/init` sees `.alius` and offers Reinitialize/Exit. Persistence of init-state begins only on the first real submit. The `CREATE_PROJECT` choice panel (Reinitialize / Exit) is reached only when `.alius` already exists.

The interaction surface top renders an **init stage nav bar** — `初始化 · 语言 · 模型池 · 分配 · 角色` — mirroring `/config`. "初始化" is a fixed non-selectable title; the four configurable stages are inline, the active one is white-background-highlighted, and stages whose data is already set get a `✓` prefix. `Shift+Tab` steps the wizard back one stage (reuse `InitWizard::back()`); walking forward again auto-skips stages that are already complete (`next_unfinished_state`). If a precondition is unmet moving forward (e.g. model pool emptied, then reaching Assignment), the existing per-stage guard surfaces the issue.

Language selection during `/init` immediately updates the right-side workflow panel, action panel labels, and step feedback to the selected locale while keeping the internal operation scopes stable. The role step loads installed role formulas for selection, preserves saved role progress only when resuming init-state, and treats activation/installation failure as an `/init` error with recovery options instead of silently advancing.

Slash commands are handled before plan-draft continuation input. This keeps `/init`, `/config`, and `/model` reachable even after an uninitialized workspace reports missing model or role requirements during a plan-draft attempt.

When `.alius/runtime/init-state.toml` exists, `/init` starts with `Continue Previous`, `Restart`, and `Exit`. Successful transitions save progress to that file. `Complete` and `Cancelled` clear it.

Model import inside `/init` uses the same provider catalog as `/model`: provider, API mode, Base URL, plaintext API Key input with paste support, remote model fetch, and model multi-select import. The CLI adapter executes the model fetch with `runtime-model`; `runtime-config` does not depend on `runtime-model`. Imported models are written to `.alius/config/providers.toml` immediately so a later `/config` task can read the same model pool even if initialization is resumed or interrupted. A successful `/init` model fetch also writes the entered API Key into the active runtime settings so Bypass/Direct chat can start without reporting a missing `api_key`. `/init` then assigns Plan/Execute/Review from the imported model pool.

Fresh `/init` starts from an empty wizard context instead of pre-filling language, role, or model assignment from existing runtime settings. Choosing reinitialization resets project config defaults, clears the selected role, clears language overrides, and clears model assignment before the user selects new values. After role configuration succeeds, initialization saves and exits automatically. The workspace defaults to Copilot mode. Team mode is entered through a separate workspace operation, not as a mode choice during initialization.

## Model Pool

Typing `/model` starts the inline model-pool manager in the workspace. It does not suspend into the old full-screen selector and does not change the active model directly.

The pool reads entries from `.alius/config/providers.toml` and displays concrete model entries with their provider and Base URL, for example:

```text
GLM-5-Turbo    BigModel GLM (Coding Plan)    OpenAI API
```

`Add Model` is the explicit remote operation. It asks for provider, API mode, Base URL, and API Key, then fetches models from that provider. Provider choices are limited to `BigModel GLM (Coding Plan)`, `Xiaomi MiMo (Token Plan)`, and `DeepSeek`. API mode choices are `OpenAI API` and `Anthropic API`.

API Key input is plaintext, accepts keyboard input and paste, and is not masked while the user edits it. Saved keys are still not shown in session output or model details.

Returned models are shown as a multi-select list. Manual model-name entry is not part of the add flow. Imported and deleted model-pool entries are written to `.alius/config/providers.toml` immediately, so reopening `/config` sees the current model pool. Deleting a model is blocked while it is assigned to `Plan Model`, `Execute Model`, or `Review Model`; the user must change the assignment in `/config` first.

Saving `/config` synchronizes compatibility fields: `Plan Model` maps to `tiers.light`, `Execute Model` maps to `tiers.medium` and the active legacy runtime model, and `Review Model` maps to `tiers.high` and `Settings.llm.review_model`.

## Agent Team Boundary

Agent Team and A2A traffic must remain distinct from the local Conversation workflow.

Current state:

- Agent Team state and view concepts exist.
- Plan nodes carry owner-like concepts for future assignment.
- Agent Team is not live by default.
- Do not claim live AgentNet or A2A traffic until runtime plumbing populates the Agent Team state.

## TUI Test Design

The TUI workspace must have deterministic tests for state transitions, interaction surfaces, rendering reducers, keyboard and mouse behavior, and runtime event reduction. Test helpers must not leak into the release `alius` binary.

### Test Placement

Unit tests that only validate private module behavior should stay behind `#[cfg(test)]`:

```rust
#[cfg(test)]
mod tests {
    #[test]
    fn test_state_transition() {}
}
```

Shared test helpers are allowed only in test-gated modules:

```rust
#[cfg(any(test, feature = "testing"))]
pub mod testing;
```

Allowed helper locations:

- `runtime/model/src/testing.rs`
- `runtime/tools/src/testing.rs`
- `runtime/core/src/testing.rs`
- `entrypoints/cli/src/testing.rs`
- `entrypoints/cli/src/tui/testing.rs`

Integration tests should live under package-local `tests/` directories, for example:

- `entrypoints/cli/tests/cli_parse.rs`
- `entrypoints/cli/tests/cli_dispatch_config.rs`
- `entrypoints/cli/tests/tui_state_machine.rs`
- `runtime/core/tests/chat_run.rs`
- `runtime/model/tests/client_cache.rs`

Files under `tests/` compile only as test binaries and must not be linked into the release `alius` binary.

### TestKit Scope

The TUI TestKit should provide only deterministic harnesses and fakes:

- `TuiTestHarness` for workspace state setup, synthetic key and mouse events, rendered block inspection, and terminal-size variants.
- `CoreRuntimeHarness` for deterministic Core event streams and cancellation or confirmation scenarios.
- `VecEventSource` for ordered event replay without background networking.
- `FakeProvider` for model output fixtures.
- `FakeTool` for controlled tool success, failure, and confirmation behavior.

Production modules must not import `crate::testing::*` or package testing modules from normal code paths. Any import of a test helper must itself be gated with `#[cfg(any(test, feature = "testing"))]`.

### TUI State-Machine Coverage

TUI tests should prioritize state-machine coverage over fragile terminal screenshots. Required coverage:

- workspace launch state, welcome block presence, and status bar rendering for Git and non-Git directories;
- Plan and Bypass mode switching, including the rule that `Shift+Tab` does not change mode while a configuration task is active;
- plan drafting, clarification prompt rendering, plan proposal, approval, per-node execution, completion confirmation, and Plans panel close;
- cancellation and interrupt flow through `Esc` while the model is drafting, while a tool confirmation is pending, and while execution is active;
- `/init` wizard state transitions, resume/restart/exit behavior, progress persistence, and cleanup of completed or cancelled init state;
- `/config` tab navigation, immediate apply-on-select behavior, validation feedback, and silent exit when nothing is unsaved;
- `/model` add/view/delete flows with local provider fixtures and no real network dependency;
- interaction-surface variants: text input, single select, multi select, approval controls, and validation errors;
- folding behavior, global expand/collapse, and click-to-toggle for long conversation blocks;
- mouse capture release for native text selection and absence of custom right-click copy UI;
- responsive welcome layouts for wide, medium, compact, and tiny terminal sizes;
- Core event reduction for tool start/completion/error, plan events, status events, and output streaming.

Snapshot or screenshot-style assertions may be used only for stable, layout-critical fragments. The primary assertions should inspect state, block kinds, visible labels, selected options, active operation scope, and emitted commands.

### Test and Release Commands

Test runs that need shared harnesses use the explicit `testing` feature:

```bash
cargo test --workspace --features testing --locked
```

Release builds must not enable the `testing` feature:

```bash
cargo build -p alius-cli --bin alius --release --locked
```

Forbidden release commands:

```bash
cargo build --release --all-features
cargo build --release --features testing
```

Release CI must scan the final binary for test-only symbols and fail if any are present:

```bash
strings target/release/alius | grep -E "FakeProvider|FakeTool|CoreRuntimeHarness|TuiTestHarness|VecEventSource|testing::|testkit"
```

## Legacy REPL

The legacy REPL remains useful for fallback and debugging:

```bash
ALIUS_LEGACY_REPL=1 alius
```

The legacy path supports slash commands and terminal streaming through the same manager-backed compatibility bridge used by the TUI workspace, but it is not the main product surface.
