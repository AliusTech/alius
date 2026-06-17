# Current State

This file summarizes the current implementation state of Alius in this checkout.

## Implemented

| Area | State |
| --- | --- |
| Testing infrastructure | `testing` feature flag on all 9 crates. Shared testing modules in `protocol`, `runtime-tools`, `runtime-model`, `core-runtime`, `entrypoints/cli`. `FakeProvider`, `FakeTool`, `EchoTool`, `ConfirmationRequiredTool`, `FakeMcpEchoTool`, `FakeMcpToolCallProvider`, `CoreRuntimeHarness`, `CwdGuard`, TUI key helpers. Release binary symbol scan in CI. ~783 tests total. |
| CLI integration tests | 30 integration tests covering all CLI commands: parse, config, core, soul, plugin, mcp, workflow, run. Isolated HOME/workspace via `tempfile::TempDir`. |
| CLI binary | `alius` is built from `entrypoints/cli` and uses `clap` command definitions. |
| Project init | `alius init` resets project config and opens the TUI init wizard. |
| TUI workspace | The default interactive path enters the Ratatui workspace unless `ALIUS_LEGACY_REPL=1` is set. |
| Legacy REPL | A `rustyline` path remains available behind `ALIUS_LEGACY_REPL=1`. |
| Protocol types | `ProtocolEnvelope<T>`, `CoreRequest`, `CoreCommand`, `CoreEvent`, and `CoreRuntimeApi` are defined in `protocol-interface`. |
| Runtime Manager | `CoreRuntimeManager` is exported by `core-runtime` and owns local runtime assembly for default product execution. |
| Direct Rust bridge | `ProtocolBridge` is now a CLI/TUI compatibility wrapper around `CoreRuntimeManager`. |
| Core Runtime | `CoreRuntime` implements `CoreRuntimeApi` and owns session, run, and loop execution state. |
| Loop Engine | Chat and Plan modes enter one `LoopEngine`; behavior is controlled by `LoopPolicy`. |
| TUI Plan drafting | Plan mode asks the model to clarify task details before proposing a plan; the Plans panel appears only after user approval. |
| Model providers | OpenAI-compatible, Anthropic, BigModel, and Custom paths exist through `runtime-model`. |
| Stores | Session, conversation, memory, episodic, semantic, procedural, and retrieval modules exist under `runtime-store`. |
| Tools | Rust WASM module loading, `ToolRegistry`, `AliusTool`, Shell Gate, WASM host imports (read_file/write_file/list_dir/env_get/shell/fetch), and host audit sink exist under `runtime-tools`. |
| JSON-RPC | A lightweight `jsonrpc` package exposes 8 methods (health_check, config_read, model_list, tool_list, run_start, run_subscribe, run_cancel, run_confirm_tool) and TCP line server. |

## Partially Wired

| Area | State |
| --- | --- |
| Plan mode tool execution | Plan mode can use a tool registry through `LoopEngine`. Tool confirmation flow is now implemented: `ToolConfirmationRequired` events trigger UI prompts in the TUI, and user approval/denial is sent back to the runtime via `respond_confirmation()`. |
| Tool permissions | Permission structures and Shell Gate exist, but enforcement is not uniform across every default execution path. |
| Model router | Router types exist, but product paths should be checked before assuming tier routing is active. |
| Config schema | Rich project config views exist, while some older settings paths remain for compatibility. |
| Logging | Logging modules exist in Core Runtime, but documentation should not assume full trace persistence on every path. |
| Admin direct calls | Init, config, credential, and plugin management may still call subsystem crates directly as temporary bootstrap/admin exceptions. |

## Dormant Scaffold

| Area | State |
| --- | --- |
| Agent Team | TUI state and view concepts exist, but live Agent Team or AgentNet traffic is not connected by default. |
| A2A | Protocol concepts and config surfaces exist, but A2A runtime plumbing is not a live default feature. |
| MCP tools | MCP server config, connection, and tool listing exist. MCP auto-initialization requires: (1) `mcp` Cargo feature enabled, (2) project config `.alius/config/tools.toml` has `registry.mcp_tools = true`, `mcp.load_on_workspace_start = true`, and `mcp.register_as_tools = true`, (3) user-level `~/.alius/mcp/servers.toml` exists. When all conditions are met, MCP tools register into the shared `ToolRegistry` and are visible via `tool_list` / JSON-RPC `tool_list` with `ToolSource::Mcp`. Native/WASM tools take priority on name conflicts. MCP initialization failure does not block runtime startup. E2E tests verified with stdio echo server. |
| Rust WASM module tools | Tool module loading, `ToolRegistry` registration, WASM host imports (6 functions with permission matcher → Shell Gate → audit pipeline), and host audit sink are implemented. Remaining gaps (P5): fetch real HTTP execution (deny-by-default), install-time authorization prompt, plugin upgrade re-prompt, audit trace persistence. |
| Workflow runtime | `workflow run` is backed by `CoreRuntimeManager` (LLM via LoopEngine) and `ToolRegistry` (WASM/native/MCP tools). `RuntimeWorkflowHandle` delegates prompt steps to `run_text()` and tool steps to `ToolRegistry::get() + execute()`. 9 tests including integration test with fake provider/tool proving real runtime paths. |

## Planned

- Event-driven TUI reduction from Core events.
- Stronger Shell Gate and permission enforcement.
- Deeper structured logging and trace persistence.
- Full project memory retrieval and writeback.
- A2A and Agent Team runtime integration.
- More complete JSON-RPC mapping to Core Runtime.
