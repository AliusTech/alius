# Alius Extensions

Official extension assets bundled with the Alius CLI repository.

## Structure

```
extensions/
  registry.toml          # Extension registry (id, type, path, version)
  souls/                 # Soul definitions
    Formula/souls/
      <id>.toml          # Soul formula definition
      <id>/              # Soul prompt files
        identity.md
        style.md
        rules.md
  plugins/
    wasm/                # WASM plugin source catalog
      <plugin-id>/
        plugin.toml      # Plugin manifest with permissions
        Cargo.toml
        src/lib.rs
```

## Souls

Souls are persona definitions that shape how Alius responds. They include:
- **identity.md** — Who the persona is
- **style.md** — How the persona communicates
- **rules.md** — Behavioral constraints and guidelines

`alius soul update` syncs bundled souls to `~/.alius/soul/` without requiring network access.

## WASM Plugins

WASM plugins are sandboxed extensions that run in a wasmtime runtime. Each plugin declares
its required host capabilities (filesystem, network, shell, env) in `plugin.toml`.

**Current status**: Source catalog only. CI binary distribution is not yet implemented.

## Registry

`registry.toml` describes all official extensions with:
- `id` — unique identifier
- `type` — `soul`, `wasm_plugin`, or `workflow`
- `path` — relative path to the extension entry point
- `version` — semver version
- `description` — human-readable description

## Migration from alius-souls

Previously, official souls were hosted in a separate repository (`AliusTech/alius-souls`).
As of P4, all official extension assets are bundled in this repository under `extensions/`.
The `alius-souls` repository is considered legacy.
