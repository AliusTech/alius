# Documentation History

All entries use the format:

```text
[YYYY-MM-DD HH:MM] [author]: [path] - [summary]
```

## 2026-06-11

```text
[2026-06-11 00:00] Codex: .alius/workspace - Created the English authoritative workspace documentation set from the current Alius code baseline.
[2026-06-11 00:00] Codex: .alius/workspace - Documented Phase 1 Runtime Manager boundary: core-runtime owns local runtime assembly, while runtime-* crates remain managed subsystems.
[2026-06-11 00:00] Codex: .alius/workspace - Documented Phase 2 CLI/TUI compatibility cleanup: REPL no longer retains model client, agent, tool registry, or runtime-model conversation state for default execution.
[2026-06-11 00:00] Codex: .alius/workspace - Documented the in-workspace conversational /config task and its administration boundary.
[2026-06-11 00:00] Codex: .alius/workspace - Documented inline prompt input for /config options, custom values, and checkbox-capable future prompts.
[2026-06-11 00:00] Codex: .alius/workspace - Documented the tool runtime rule that all tools are implemented as Rust WASM modules, plus the long-term ABI, Soul selection, approval, audit, and distribution roadmap.
[2026-06-12 10:11] Codex: .alius/workspace - Documented interactive model-controlled TUI Plan drafting, approval-gated Plans panel visibility, stepwise execution, and completion confirmation.
[2026-06-12 10:22] Codex: .alius/workspace - Documented choice-first confirmation surfaces with optional bottom reply input instead of custom reply as a normal choice.
[2026-06-12 10:34] Codex: .alius/workspace - Corrected clarification prompt documentation: Conversation shows the question, while the interaction surface renders single-select, multi-select, or text controls for answers.
[2026-06-12 14:56] Codex: .alius/workspace/docs/products/tui-workspace.md - Documented responsive Plan drafting and Esc confirmation interrupt behavior while the model drafts clarification questions or plan proposals.
[2026-06-12 15:38] Codex: .alius/workspace - Documented the local tabbed `/config` center, project-local model library, explicit Add Model fetch flow, inline `/model` selector, and Reasoning Notes over light/medium/high router tiers.
[2026-06-12 21:51] Codex: .alius/workspace - Corrected `/config` documentation to the three-section configuration flow, documented `/model` as the dedicated three-level model routing setup, and removed obsolete confirmation/output wording.
[2026-06-13 00:41] Codex: .alius/workspace - Updated TUI/config documentation for Plan/Execute/Review assignment in `/config`, model-pool ownership in `/model`, plaintext API Key input, and `model.toml` compatibility migration.
[2026-06-13 01:10] Codex: .alius/workspace - Documented the three built-in model providers, OpenAI/Anthropic API mode selection in `/model`, and provider-specific Base URLs for BigModel GLM, Xiaomi MiMo, and DeepSeek.
```
