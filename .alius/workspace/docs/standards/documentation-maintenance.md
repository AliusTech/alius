# Documentation Maintenance

This document defines how `.alius/workspace/` should be maintained.

## Authority Rules

- `SPEC.md` is the functional requirements source.
- `docs/` contains the current product, architecture, interface, module, and standards documentation.
- `HISTORY.md` records documentation changes.
- `ROADMAP.md` is planning context only.
- `.alius/memory/design/` is historical input, not the current authority.

## Language

All workspace documentation prose must be English.

Allowed non-English content:

- Code identifiers.
- File paths.
- Command names.
- API names.
- Exact quoted user-facing strings from existing code when needed for accuracy.

## Status Labels

Use these status labels consistently:

- Implemented
- Partially wired
- Dormant scaffold
- Planned

Do not describe partially wired or dormant scaffold behavior as implemented.

## Code-Grounded Writing

Before changing docs, verify relevant code with targeted reads or searches.

Preferred commands:

```bash
cargo metadata --format-version=1 --no-deps
rg -n "<symbol-or-command>" <path>
rg --files <path>
```

## History Entries

Every documentation batch should append `HISTORY.md`.

Format:

```text
[YYYY-MM-DD HH:MM] [author]: [path] - [summary]
```

## Avoiding Drift

Check for:

- stale package names
- stale paths
- old package path references from earlier layouts
- overclaims about Agent Team, A2A, MCP, Plugin, Workflow, Google provider, Shell Gate, and permission enforcement
- README claims that conflict with current code
