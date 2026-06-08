# Architecture Overview

Alius is organized as a local-first Agent Runtime Workspace with a layered execution model.

```text
Product Entrypoints
  -> Protocol Interface
  -> Core Runtime Manager
  -> Core Runtime
  -> Runtime subsystems
```

## Active Workspace Packages

| Package | Path | Layer |
| --- | --- | --- |
| `alius-cli` | `entrypoints/cli` | Product Entrypoint |
| `jsonrpc` | `entrypoints/jsonrpc` | Product Entrypoint / adapter |
| `protocol-interface` | `protocol` | Protocol Interface |
| `core-runtime` | `runtime/core` | Core Runtime and local Runtime Manager |
| `runtime-config` | `runtime/config` | Runtime subsystem |
| `runtime-model` | `runtime/model` | Runtime subsystem |
| `runtime-tools` | `runtime/tools` | Runtime subsystem |
| `runtime-store` | `runtime/memory` | Runtime subsystem |

## Product Entrypoints

Product entrypoints define how users or integrations enter Alius.

Current entrypoints:

- CLI binary and TUI workspace through `alius-cli`.
- Lightweight JSON-RPC package through `jsonrpc`.

Planned or partial surfaces:

- Desktop application.
- IDE extension.
- Embedded SDK.
- A2A remote agent integration.

Product code should not own agent loop internals, provider-specific behavior, or storage internals.

For default execution, product entrypoints should not assemble the model client, tool registry, memory stores, and `CoreRuntime` themselves. They should call the local `CoreRuntimeManager`, which owns that assembly and then enters `ProtocolInterface<CoreRuntime>`.

## Protocol Interface

The protocol layer defines the shared boundary between product code and Core Runtime.

Main responsibilities:

- Define request, command, event, error, origin, and capability types.
- Validate protocol envelope versions.
- Enforce origin capability ceilings before delegation.
- Delegate to an implementation of `CoreRuntimeApi`.
- Wrap Core events back into protocol envelopes where needed.

## Core Runtime Manager

`CoreRuntimeManager` is the local Runtime Manager facade exported by `core-runtime`.

Main responsibilities:

- Assemble `LlmClient`, `ToolRegistry`, memory/store paths, settings, and `CoreRuntime` for local product callers.
- Hold `ProtocolInterface<CoreRuntime>` internally.
- Provide product-level methods such as `run_text`, `start_streaming`, `config_read`, `model_list`, `memory_save`, `memory_list`, `memory_clear`, `tool_list`, `review_start`, and `health_check`.
- Preserve the protocol boundary while giving CLI, TUI, and JSON-RPC a stable local entrypoint.

`CoreRuntimeManager` is not a replacement for `CoreRuntimeApi`. The manager is a local facade; `CoreRuntimeApi` remains the protocol contract implemented by `CoreRuntime`.

## Core Runtime

Core Runtime is the managed execution layer.

Main responsibilities:

- Build and own runtime state.
- Manage sessions, turns, runs, and trace ids.
- Adapt requests into loop input.
- Run Chat and Plan modes through `LoopEngine`.
- Emit and store Core events for runs.
- Expose config, memory, session, tool, review, and health operations through `CoreRuntimeApi`.

## Runtime Subsystems

| Subsystem | Package | Purpose |
| --- | --- | --- |
| Config | `runtime-config` | Project config, schema views, Agent Card, migration, workspace templates |
| Model | `runtime-model` | LLM providers, streaming, conversation, agent loop, router |
| Tools | `runtime-tools` | Tool trait, registry, Rust WASM module loading, Shell Gate |
| Store | `runtime-store` | Memory, sessions, conversations, retrieval, layered stores |

## Boundary Rules

- Product entrypoints should use `CoreRuntimeManager` for default execution and should not directly assemble model or tool execution paths.
- Product entrypoints should use protocol contracts instead of directly calling Core internals.
- Protocol Interface should not implement model provider logic or tool behavior.
- Core Runtime should not depend on TUI-private types.
- Shared cross-layer data structures should live in `protocol-interface` when they are protocol contracts.
- Dangerous tool execution should pass through tool policy and Shell Gate mechanisms as those paths mature.

## Temporary Admin Exceptions

The following product surfaces may temporarily call subsystem crates directly because they bootstrap or administer runtime state rather than execute the default conversation, Plan, tool, memory, or review path:

- `alius init`
- config UI and config file management
- credential management
- plugin install, list, and remove commands

These exceptions are implementation gaps, not the target architecture.
