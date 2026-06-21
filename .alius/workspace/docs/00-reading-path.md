# Reading Path

Use this guide to choose the shortest path through the documentation.

## First-Time Reader

1. `../README.md`
2. `01-current-state.md`
3. `terms/GLOSSARY.md`
4. `products/cli.md`
5. `products/tui-workspace.md`
6. `overview/architecture.md`
7. `overview/runtime-flow.md`

Goal: understand what Alius is, how to run it, and which parts are implemented today.

## Contributor

1. `01-current-state.md`
2. `overview/architecture.md`
3. `interfaces/protocol-interface.md`
4. `interfaces/core-runtime-api.md`
5. `modules/<target-module>.md`
6. `standards/validation.md`

Goal: make a scoped code change without crossing product, protocol, and runtime boundaries.

## Architecture Maintainer

1. `../SPEC.md`
2. `overview/architecture.md`
3. `overview/data-flow.md`
4. `interfaces/protocol-interface.md`
5. `interfaces/events-and-tracing.md`
6. `overview/implementation-gaps.md`
7. `../ROADMAP.md`

Goal: decide whether a change belongs in product code, protocol interface code, Core Runtime, or an extension subsystem.

## Agent Team Maintainer

1. `../SPEC.md`
2. `modules/agent-team.md`
3. `products/tui-workspace.md`
4. `interfaces/protocol-interface.md`
5. `interfaces/events-and-tracing.md`
6. `overview/implementation-gaps.md`

Goal: design or review Agent CLI long-lived connections, Agent presence, work status, task leases, backend authorization, and TUI Agent Team event population.

## Documentation Maintainer

1. `standards/documentation-maintenance.md`
2. `standards/validation.md`
3. `../HISTORY.md`

Goal: keep `.alius/workspace/` accurate as code evolves.
