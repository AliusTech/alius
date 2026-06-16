# Current State

This file summarizes the current implementation state of Alius in this checkout.

## Implemented

| Area | State |
| --- | --- |
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
| Tools | Rust WASM module loading, `ToolRegistry`, `AliusTool`, and Shell Gate modules exist under `runtime-tools`. |
| JSON-RPC | A lightweight `jsonrpc` package exposes a small method dispatcher and TCP line server. |

## Partially Wired

| Area | State |
| --- | --- |
| Plan mode tool execution | Plan mode can use a tool registry through `LoopEngine`, but end-to-end tool approval and evidence handling are still developing. |
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
| MCP tools | MCP server config, connection, and tool listing exist. MCP auto-initialization requires: (1) `mcp` Cargo feature enabled, (2) project config `.alius/config/tools.toml` has `registry.mcp_tools = true`, `mcp.load_on_workspace_start = true`, and `mcp.register_as_tools = true`, (3) user-level `~/.alius/mcp/servers.toml` exists. When all conditions are met, MCP tools register into the shared `ToolRegistry` and are visible via `tool_list` / JSON-RPC `tool_list` with `ToolSource::Mcp`. Native/WASM tools take priority on name conflicts. MCP initialization failure does not block runtime startup. |
| Rust WASM module tools | Tool module loading and registration code exists, but capability policy and production ABI behavior need more hardening. |
| Workflow runtime | Workflow commands and parsing exist, but prompt and tool steps should not be described as a complete automation engine. |

## Planned

- Event-driven TUI reduction from Core events.
- Stronger Shell Gate and permission enforcement.
- Deeper structured logging and trace persistence.
- Full project memory retrieval and writeback.
- A2A and Agent Team runtime integration.
- More complete JSON-RPC mapping to Core Runtime.
