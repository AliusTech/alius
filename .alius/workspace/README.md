# Alius Workspace Documentation

This directory is the authoritative project documentation area for Alius. It describes the current codebase, the intended architecture, the public contracts between layers, and the operational rules for maintaining the project.

The documentation is organized from broad to detailed. Start with the reading path, then move into product behavior, architecture, interfaces, module internals, and standards.

## Authority

Implementation decisions should be grounded in:

1. `SPEC.md` for functional requirements and acceptance criteria.
2. `docs/` for product, architecture, interface, module, and engineering details.
3. `HISTORY.md` for documentation changes and decision history.

`ROADMAP.md` is planning context. It is useful for prioritization, but it is not an implementation contract.

`.alius/memory/design/` is historical input. It may contain useful migration notes, but it is not the final source of truth for the current workspace documentation.

## Reading Order

1. `docs/00-reading-path.md`
2. `docs/01-current-state.md`
3. `docs/products/cli.md`
4. `docs/products/tui-workspace.md`
5. `docs/overview/architecture.md`
6. `docs/interfaces/protocol-interface.md`
7. `docs/modules/core-runtime.md`
8. `SPEC.md`
9. `ROADMAP.md`
10. `HISTORY.md`

## Current Package Baseline

The current Rust workspace has these active packages:

| Package | Path | Role |
| --- | --- | --- |
| `alius-cli` | `entrypoints/cli` | CLI binary, command parsing, TUI, REPL, and command dispatch |
| `jsonrpc` | `entrypoints/jsonrpc` | Lightweight JSON-RPC entrypoint |
| `protocol-interface` | `protocol` | Shared protocol types and Direct Rust API gateway |
| `core-runtime` | `runtime/core` | Core Runtime implementation, Session Manager, Loop Engine, logging, patch helpers |
| `runtime-config` | `runtime/config` | Project config loading, schema views, migration, Agent Card, workspace templates |
| `runtime-model` | `runtime/model` | LLM client, providers, conversation state, agent loop, model router |
| `runtime-tools` | `runtime/tools` | Tool trait, registry, Rust WASM module loading, Shell Gate, WASM host |
| `runtime-store` | `runtime/memory` | Memory, conversation, session, retrieval, and layered stores |

## Status Language

This documentation uses four status labels:

| Status | Meaning |
| --- | --- |
| Implemented | Present in code and usable on a normal path. |
| Partially wired | Present in code, but not fully connected to the default workflow. |
| Dormant scaffold | Types or UI exist, but no live runtime plumbing is connected yet. |
| Planned | Architectural direction only. |
