# CLI Entrypoint Module

Primary paths:

- `entrypoints/cli/src/cli.rs`
- `entrypoints/cli/src/main.rs`
- `entrypoints/cli/src/repl/`
- `entrypoints/cli/src/tui/`

## Responsibilities

- Define the `alius` command-line interface with `clap`.
- Load settings.
- Apply locale.
- Dispatch commands.
- Start the TUI workspace or legacy REPL.
- Expose init, config, model, soul, core, plugin, MCP, and workflow command surfaces.

## Important Types and Functions

- `Cli`
- `Command`
- `ConfigCommand`
- `CredentialCommand`
- `CoreCommand`
- `SoulCommand`
- `PluginCommand`
- `McpCommand`
- `WorkflowCommand`
- `run()`
- `run_repl(settings)`
- `run_workspace(session, initial_missing)`

## Runtime Boundary

CLI code should not own model provider internals, storage layouts, tool implementations, or Core Runtime internals.

When CLI needs default model execution, model listing, runtime memory, tool listing, review, or health checks, it should go through:

```text
CLI / TUI
  -> ProtocolBridge compatibility wrapper
  -> CoreRuntimeManager
  -> ProtocolInterface<CoreRuntime>
  -> CoreRuntime
```

The REPL and TUI compatibility layer should not retain its own `LlmClient`, `AliusAgent`, or `ToolRegistry` for default execution. Local REPL history state should use protocol `Message` values rather than runtime-model conversation types.

## Known Gaps

- Some root flags are defined but not fully consumed across all dispatch paths.
- Extension command management does not imply default workspace runtime integration.
- Legacy REPL remains available but should not drive the main product model.
- Local session and conversation stores are still owned by the CLI/TUI layer for history display and compatibility.
