# Alius Documentation Roadmap

This roadmap is planning context. It does not override `SPEC.md` or the module and interface docs under `docs/`.

## Near Term

- Keep `.alius/workspace/` as the authoritative documentation area.
- Keep all workspace docs in English.
- Update docs whenever code changes alter CLI behavior, protocol contracts, runtime flow, config schema, or tool safety behavior.
- Document the difference between implemented runtime behavior and dormant scaffolds.
- Reconcile top-level README files with the workspace documentation once this set stabilizes.

## Medium Term

- Add diagrams for event streams, approval flow, Shell Gate enforcement, and memory retrieval.
- Add acceptance checklists per module.
- Add examples for project configuration, provider setup, Agent Card configuration, and memory usage.
- Expand JSON-RPC documentation when it exposes real Core Runtime requests and event streams.
- Expand extension documentation when MCP tools, Rust WASM module tools, and workflows are connected to the main runtime path.

## Long Term

- Generate a minimal `.alius/workspace/` documentation skeleton from `alius init`.
- Add automated documentation checks for stale package names, broken links, and unsupported capability claims.
- Maintain archived snapshots under `.alius/workspace/.archive/` when major documentation versions are confirmed.
- Publish selected workspace docs as external developer documentation after internal accuracy is stable.
- Complete the tool runtime roadmap: formal Rust WASM tool ABI and SDK, first-party tool migration, Soul-driven tool selection, unified approval and audit, and versioned tool package distribution.
