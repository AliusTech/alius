# npm Distribution

Alius has npm package wrapper files under `npm-packages/`. This documentation describes the intended distribution model, not a guarantee that every release artifact is currently synchronized.

## Package Model

The distribution model uses:

- A main package, typically `@alius-tech/alius`.
- Platform packages for operating system and architecture specific binaries.
- A Node wrapper that resolves the correct native binary and forwards process arguments and stdio.

## Responsibilities

The npm wrapper should:

- Detect `process.platform` and `process.arch`.
- Resolve the matching platform package or local development binary.
- Spawn the native `alius` binary.
- Forward stdio and process signals.

## Version Caution

Always verify release versions before publishing. The Rust workspace version, npm package version, generated platform package versions, tags, and changelog can drift if release automation is incomplete.

## Documentation Boundary

The npm wrapper is a product distribution surface. It does not own Core Runtime behavior, Protocol Interface contracts, TUI behavior, or tool permissions.

