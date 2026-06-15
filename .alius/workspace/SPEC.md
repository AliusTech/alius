# Alius Workspace Specification

This file is the functional requirements source for the Alius workspace documentation set. It reflects the current code baseline and separates implemented behavior from intended architecture.

## F-001 Project Documentation Authority

Alius must keep project documentation under `.alius/workspace/`.

Acceptance criteria:

- `README.md` identifies `.alius/workspace/` as the authoritative project documentation area.
- `docs/00-reading-path.md` gives a clear reading order.
- `HISTORY.md` records documentation changes.
- Historical `.alius/memory/design/` files are treated as migration input, not the final authority.

## F-002 CLI Entrypoint

The `alius` binary must expose the command surface defined in `entrypoints/cli/src/cli.rs`.

Acceptance criteria:

- Documentation covers `alius`, `alius repl`, `alius run -p`, `alius init`, `alius config`, `alius core`, `alius soul`, `alius plugin`, `alius mcp`, and `alius workflow`.
- Documentation states which root flags are defined and whether they are consumed by the current dispatch path.
- Documentation distinguishes command definitions from runtime wiring.

## F-003 Plan-Driven TUI Workspace

The default interactive experience is the Ratatui workspace unless `ALIUS_LEGACY_REPL=1` is set.

Acceptance criteria:

- Documentation describes Alius as a Plan-driven Agent Runtime Workspace, not a generic chat UI.
- Documentation preserves `Shift+Tab` for mode switching and `Ctrl+Enter` for submit.
- Documentation uses `Plans` terminology for plan state.
- Documentation keeps local Conversation separate from Agent Team and A2A traffic.
- Plan mode does not show the Plans panel before a plan is generated and approved.
- Plan generation is interactive: the model may ask clarifying questions until task details, preconditions, constraints, and success criteria are clear enough to propose an executable plan.
- Approved plan nodes execute step by step; after all approved nodes complete, the user confirms plan completion and the Plans panel closes.
- Clarification prompts put the question text in Conversation and render model-proposed answers in the interaction surface as single-select, multi-select, or text input controls.
- `/config` is a local tabbed configuration center inside the workspace and does not call the model or fetch provider model lists on entry.
- `/config` exposes only `configuration-models`, `configuration-language`, and `configuration-solo`; missing required items are reported in Conversation.
- During `/config`, `Tab` and `Shift+Tab` switch configuration tabs instead of switching Plan/Bypass mode.
- `/model` stays inside the workspace, reads the local model library instead of calling remote model listing, and configures the current `Quick Reasoning`, `Standard Reasoning`, and `Deep Reasoning` model mappings.

## F-004 Protocol Interface Boundary

Product entrypoints should enter Core Runtime through protocol contracts.

Acceptance criteria:

- Documentation covers `ProtocolEnvelope<T>`, `Origin`, `CapabilityScope`, `CoreRequest`, `CoreCommand`, `CoreEvent`, `ProtocolError`, and `CoreRuntimeApi`.
- Documentation states that capability scope is an origin-supplied ceiling, not final authorization.
- Documentation states that all long-running execution is represented as request, command, and event semantics.

## F-005 Core Runtime Main Chain

Core Runtime owns session, run, turn, trace, and loop execution state.

Acceptance criteria:

- Documentation covers `CoreRuntime`, `CoreRuntimeBuilder`, `SessionManager`, `LoopEngine`, and `EventAdapter`.
- Documentation describes the current chain from `ProtocolBridge` to `ProtocolInterface` to `CoreRuntime` to `LoopEngine`.
- Documentation states that Chat/Bypass mode is one user turn with bounded tool-call continuations, tools enabled, and planning disabled; Plan mode is goal-oriented and may use multiple tool-enabled iterations after an executable plan is approved.

## F-006 Configuration System

Alius must document the project configuration model under `.alius/config/`.

Acceptance criteria:

- Documentation covers `ProjectConfigSnapshot` and `RuntimeConfigView`.
- Documentation lists `config.toml`, `providers.toml`, `soul.toml`, `tools.toml`, `permissions.toml`, `protocol.toml`, and `mcp.json`.
- Documentation identifies legacy flat paths as compatibility paths where applicable.
- Documentation covers the local model library stored in `providers.toml`.
- Documentation maps `Quick Reasoning`, `Standard Reasoning`, and `Deep Reasoning` to the existing `light`, `medium`, and `high` router tiers.
- Documentation states that Add Model is the explicit remote-fetch flow and falls back to manual model-name entry on fetch failure.

## F-007 Model Runtime

Alius must document provider support according to current code.

Acceptance criteria:

- OpenAI-compatible providers, BigModel, and Custom are documented as OpenAI-compatible paths.
- Anthropic is documented as a native provider.
- Google is documented as present in code but not production-complete unless current code proves otherwise.
- Model routing is documented as a runtime module, not as a guarantee that all product paths use tier routing.

## F-008 Tools and Shell Gate

Alius must document the tool model without overstating enforcement.

Acceptance criteria:

- Documentation covers `AliusTool`, `ToolRegistry`, `ToolContext`, Rust WASM module loading, and Shell Gate modules.
- Documentation states that all tools are implemented as Rust WASM modules.
- Documentation states that Core Runtime owns loading, validation, permission, scheduling, events, and audit boundaries, but not concrete tool business logic.
- Documentation states that some permission structures exist but are not fully enforced on every default execution path.

## F-009 Memory and Store

Alius must document layered memory and persistence behavior.

Acceptance criteria:

- Documentation covers global and project memory concepts.
- Documentation covers `ConversationStore`, `SessionStore`, `MemoryStore`, `EpisodicStore`, `SemanticStore`, `ProceduralStore`, and `RetrievalEngine`.
- Documentation distinguishes documentation memory from runtime memory data.

## F-010 Extensions

Alius must document extension systems by maturity.

Acceptance criteria:

- Documentation covers MCP, WASM Plugin, Workflow, Agent Team, and A2A.
- Documentation clearly labels MCP tools, Plugin tools, Workflow prompt/tool execution, and Agent Team/A2A live traffic as not fully connected to the default workspace unless current code shows otherwise.

## F-011 JSON-RPC Entrypoint

Alius must document the current JSON-RPC entrypoint accurately.

Acceptance criteria:

- Documentation covers `dispatch` and `serve`.
- Documentation lists implemented methods such as `health_check`, `config_read`, and `version`.
- Documentation states that the current implementation is lightweight and does not expose the full Core Runtime protocol surface.

## F-012 Documentation Maintenance

Every documentation update should keep implementation state and planned state separate.

Acceptance criteria:

- New docs avoid stale historical package paths unless they are quoted as historical notes.
- New docs avoid describing dormant scaffolds as implemented runtime features.
- `HISTORY.md` receives an entry for each documentation batch.

## F-013 Runtime Manager Boundary

`core-runtime` must act as the local Runtime Manager for default product execution while `runtime-config`, `runtime-model`, `runtime-tools`, and `runtime-store` remain independent managed subsystems.

Acceptance criteria:

- `core-runtime` exports `CoreRuntimeManager`.
- CLI, TUI, and JSON-RPC default execution enters through `CoreRuntimeManager`, then `ProtocolInterface<CoreRuntime>`, then `CoreRuntime`.
- Product entrypoints do not directly assemble `LlmClient`, `ToolRegistry`, and `CoreRuntime` for default conversation, Plan, one-shot run, memory, tool listing, review, or health flows.
- Product compatibility wrappers do not retain their own `LlmClient`, `AliusAgent`, `ToolRegistry`, or runtime-model conversation container for default execution.
- `CoreRuntimeManager` does not replace `CoreRuntimeApi`; it is a local facade over the existing protocol contract.
- Bootstrap and administration surfaces may temporarily call subsystem crates directly only for init, config management, credential management, and plugin management.
- Temporary subsystem direct calls are documented in `docs/overview/implementation-gaps.md`.
