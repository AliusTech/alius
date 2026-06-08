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
- Output
- Error

## Workspace Configuration Center

Typing `/config` starts a local tabbed configuration center in the workspace instead of opening a separate full-screen page or calling the model.

The available tabs are:

- `configuration-models`
- `configuration-language`
- `configuration-solo`

The Conversation area records the request, current prompt, validation feedback, fetch failures, and save confirmation. It does not render option lists. The interaction surface renders the active option picker, multi-select list, text input, or exit confirmation.

Opening `/config` validates required local state and jumps to the first missing item. Required state is:

- At least one enabled model in the local model library.
- `Plan Model`, `Execute Model`, and `Review Model` each map to an enabled model.
- Selected models reference providers with a Base URL.
- Selected providers have a direct API key or API key environment source.
- A Soul is selected.
- A language is selected.

The `configuration-models` section assigns `Plan Model`, `Execute Model`, and `Review Model` from enabled entries in the local model library. It does not allow manual model-name, Base URL, or API key entry.

The `configuration-language` section selects the interface language. The `configuration-soul` section selects the active Solo/Soul role.

Saving the configuration center writes settings, writes `.alius/config/providers.toml`, writes `.alius/config/model.toml`, and rebuilds the local runtime bridge. This is an administration surface, not a default model execution path.

## Model Pool

Typing `/model` starts the inline model-pool manager in the workspace. It does not suspend into the old full-screen selector and does not change the active model directly.

The pool reads entries from `.alius/config/providers.toml` and displays concrete model entries with their provider and Base URL, for example:

```text
GLM-5-Turbo    BigModel GLM (Coding Plan)    OpenAI API
```

`Add Model` is the explicit remote operation. It asks for provider, API mode, Base URL, and API Key, then fetches models from that provider. Provider choices are limited to `BigModel GLM (Coding Plan)`, `Xiaomi MiMo (Token Plan)`, and `DeepSeek`. API mode choices are `OpenAI API` and `Anthropic API`.

API Key input is plaintext, accepts keyboard input and paste, and is not masked while the user edits it. Saved keys are still not shown in session output or model details.

Returned models are shown as a multi-select list. Manual model-name entry is not part of the add flow. Deleting a model is blocked while it is assigned to `Plan Model`, `Execute Model`, or `Review Model`; the user must change the assignment in `/config` first.

Saving `/config` synchronizes compatibility fields: `Plan Model` maps to `tiers.light`, `Execute Model` maps to `tiers.medium` and the active legacy runtime model, and `Review Model` maps to `tiers.high` and `Settings.llm.review_model`.

## Agent Team Boundary

Agent Team and A2A traffic must remain distinct from the local Conversation workflow.

Current state:

- Agent Team state and view concepts exist.
- Plan nodes carry owner-like concepts for future assignment.
- Agent Team is not live by default.
- Do not claim live AgentNet or A2A traffic until runtime plumbing populates the Agent Team state.

## Legacy REPL

The legacy REPL remains useful for fallback and debugging:

```bash
ALIUS_LEGACY_REPL=1 alius
```

The legacy path supports slash commands and terminal streaming through the same manager-backed compatibility bridge used by the TUI workspace, but it is not the main product surface.
