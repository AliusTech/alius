# Validation

Use this checklist for documentation-only changes under `.alius/workspace/`.

## Baseline

Confirm workspace package names:

```bash
cargo metadata --format-version=1 --no-deps
```

Expected active package names in this checkout:

- `alius-cli`
- `jsonrpc`
- `protocol-interface`
- `core-runtime`
- `runtime-config`
- `runtime-model`
- `runtime-tools`
- `runtime-store`

## English-Only Check

Check for non-ASCII text in workspace docs:

```bash
LC_ALL=C rg -n "[^\\x00-\\x7F]" .alius/workspace
```

The command should return no results for this English documentation set.

## Stale Reference Check

Check for old package/path claims:

```bash
rg -n "crates/alius[-]|alius[-]interactive|alius[-]store|alius[-]protocol|alius[-]config|alius[-]model|alius[-]tools|alius[-]formula|alius[-]mcp|alius[-]plugin|alius[-]workflow" .alius/workspace
```

If any result is intentional historical context, label it as historical. Otherwise update it to the current path and package names.

## Capability Overclaim Check

Check for features that are often overstated:

```bash
rg -n "production-ready|fully integrated|complete|live Agent|AgentNet|A2A|MCP tools|Plugin tools|Workflow|Google|Shell Gate|permission" .alius/workspace
```

Review every result and make sure the status is accurate.

## Link Check

For each relative Markdown link, verify that the target exists.

Suggested quick scan:

```bash
rg -n "\\[[^\\]]+\\]\\([^)]+\\.md\\)" .alius/workspace
```

## Rust Tests

For documentation-only changes, full Rust tests are not required.

If a documentation task also changes runtime code, use the validation scope appropriate to that code. For broad changes involving shared state, use:

```bash
cargo test -- --test-threads=1
```

For focused CLI package checks in this checkout, use:

```bash
cargo check -p alius-cli
```
