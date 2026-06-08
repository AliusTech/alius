# Runtime Flow

This document describes the main runtime flows in the current codebase.

## Default Interactive Flow

```text
alius
  -> entrypoints/cli/src/main.rs
  -> run_repl(settings)
  -> Ratatui workspace unless ALIUS_LEGACY_REPL=1
  -> ReplSession
  -> ProtocolBridge compatibility wrapper
  -> CoreRuntimeManager
  -> ProtocolInterface<CoreRuntime>
  -> CoreRuntime
  -> SessionManager
  -> LoopEngine
  -> LlmClient and optional ToolRegistry
```

The interactive workspace is the main product surface. The legacy `rustyline` REPL remains available for fallback:

```bash
ALIUS_LEGACY_REPL=1 alius
```

## One-Shot Prompt Flow

```text
alius run -p "<prompt>"
  -> load Settings
  -> apply run-level model override
  -> CoreRuntimeManager::new_local
  -> CoreRuntimeManager::start_streaming
  -> ProtocolInterface<CoreRuntime>
  -> send Chat-mode RunLoop request through CoreRuntime
  -> print ModelDelta events
```

This is a one-shot command. It should not be described as the full interactive workspace.

## Protocol Start Flow

```text
Product entrypoint
  -> CoreRuntimeManager
  -> ProtocolInterface<CoreRuntime>
  -> CoreRequest::run_loop
  -> ProtocolEnvelope<CoreRequest>
  -> CoreRuntime::start or start_streaming
  -> SessionManager::create_session if needed
  -> SessionManager::create_turn
  -> LoopEngine::run
```

## Chat Mode Flow

Chat mode uses `LoopPolicy::chat()`:

- `max_iterations = 1`
- `tools_enabled = false`
- `planning_enabled = false`
- convergence check enabled

The loop executes a single streaming model call and emits final result or error events.

## Plan Mode Flow

The TUI Plan mode has a product-level drafting phase before execution:

1. User submits a goal in Plan mode.
2. The model acts as the plan controller and asks clarifying questions when task details, preconditions, constraints, or success criteria are incomplete.
3. The Plans panel stays hidden while the plan is still a draft.
4. When the model returns a plan proposal, the Conversation area shows it and the interaction surface asks for approval.
5. Only after approval does the TUI create visible plan nodes.
6. Approved nodes execute one by one.
7. When all approved nodes complete, the user confirms completion and the Plans panel closes.

Runtime execution still uses `LoopPolicy::plan()`:

- `max_iterations = 20`
- `tools_enabled = true`
- `planning_enabled = true`
- convergence check enabled
- tool approval required by policy

Plan mode can iterate through model calls and tool results when a tool registry is present. The TUI currently owns part of the visible plan drafting and panel lifecycle; deeper reduction from Core plan events remains the target architecture.

## Project Initialization Flow

```text
alius init
  -> load Settings
  -> reset_project_config(locale)
  -> create .alius/config/*
  -> ensure .alius/memory/*
  -> ensure .alius/workspace/
  -> run init wizard
  -> save selected provider, model, locale, and soul settings
```

Current init code ensures the workspace directory exists. It does not yet generate this full documentation set.

## Extension Command Flow

Extension commands currently live under the CLI product surface:

- `alius mcp`
- `alius plugin`
- `alius workflow`
- `alius soul`
- `alius core`

These commands should be documented as management surfaces unless a specific code path connects them to Core Runtime execution.
