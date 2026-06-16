# Plugin Permissions and Host Capabilities

Primary paths (target):

- `runtime/tools/src/wasm_host/` — host function registration
- `runtime/tools/src/package.rs` — manifest `permissions` field
- `runtime/tools/src/shell_gate/` — shared shell safety primitive
- `runtime/tools/src/traits.rs` — native `AliusTool` (shell/fs/network)

## Purpose

WASM plugins run sandboxed and cannot touch the OS directly. To let third-party
plugins do useful work that touches the filesystem, network, or shell, Alius
provides a **host capability layer**: plugins declare the permissions they need
in their manifest, and at runtime every host-function call is checked against
that manifest and the shared security primitives (Shell Gate, workspace
boundary) before reaching the real OS.

This lets third-party plugins read project files, query allowlisted APIs, or
run read-only commands — without ever being able to escape the workspace, touch
secrets, or run destructive operations.

This document is the **target architecture**. The current runtime only links
host→wasm exports (`alius_plugin_list_tools`, `alius_plugin_call_tool`); the
wasm→host imports described here are a forward design (see Status).

## Architecture

```
┌─────────────────────────────────────────┐
│  WASM Plugin (third-party, sandboxed)    │
│  calls import: alius_read_file(...)      │
└────────────────┬────────────────────────┘
                 │ wasm → host call
┌────────────────▼────────────────────────┐
│  Host Function Layer (cli built-in)      │
│  1. check manifest permission             │
│  2. validate args (path/url/cmd)          │
│  3. write audit log                       │
│  4. call shared security primitive        │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  Shared Security Primitives (native)     │
│  - Shell Gate (risk / scope)             │
│  - path canonicalize (workspace edge)    │
│  - network allowlist                      │
│  - audit sink                             │
└────────────────┬────────────────────────┘
                 │
┌────────────────▼────────────────────────┐
│  Real OS (fs / process / network)        │
└─────────────────────────────────────────┘
```

The security primitives are implemented **once** in native Rust. Native built-in
tools (shell, fs, network) call them directly; host functions for plugins call
the same primitives after an extra manifest-permission check. No security logic
is duplicated.

## Manifest Permission Schema

Plugins declare permissions in their manifest, grouped by capability domain.
Targets are **relative to the workspace root** (host resolves and canonicalizes
them at load time).

```json
{
  "permissions": {
    "filesystem": [
      "read:project",
      "read:config",
      "write:output",
      "list:project"
    ],
    "network": [
      "fetch:https://crates.io/api/v1",
      "fetch:https://api.github.com/repos"
    ],
    "shell": [
      "exec:readonly",
      "exec:git log"
    ],
    "env": [
      "read:HOME",
      "read:CARGO_HOME"
    ]
  }
}
```

### Format per domain

Each entry is `operation:target`.

- **filesystem** — `read:<path>` / `write:<path>` / `list:<path>`. Path is
  relative to workspace root (e.g. `project`, `output/report.md`). Read and
  write are declared independently for least privilege.
- **network** — `fetch:<url-prefix>`. Prefix-matched at runtime.
- **shell** — `exec:<scope>`. `readonly` = the read-only command set
  (ls/cat/grep/git-log/...). A literal command string allows exactly that
  command. Every shell call additionally passes through Shell Gate regardless
  of manifest — double insurance.
- **env** — `read:<VAR_NAME>`. Read specific environment variables.

## Host Function ABI (wasm → host imports)

Imported by the plugin module, provided by the host. Memory protocol reuses the
length-prefixed JSON convention already used by `alius_plugin_call_tool`:
arguments are `(ptr, len)` pairs; return value is a pointer to length-prefixed
JSON `{ "ok": true, ... } | { "ok": false, "error": "..." }`.

```wat
(import "alius" "read_file"
    (func $read_file (param i32 i32) (result i32)))            ;; path → result
(import "alius" "write_file"
    (func (param i32 i32 i32 i32) (result i32)))                ;; path,content → status
(import "alius" "list_dir"
    (func (param i32 i32) (result i32)))                        ;; path → entries
(import "alius" "fetch"
    (func (param i32 i32) (result i32)))                        ;; url → body
(import "alius" "shell"
    (func (param i32 i32) (result i32)))                        ;; cmd → stdout/err/exit
(import "alius" "env_get"
    (func (param i32 i32) (result i32)))                        ;; name → value
```

Each import maps to exactly one capability domain, so the host always knows
which permission set to consult.

## Runtime Verification

Every host-function call passes three gates before reaching the OS:

1. **Manifest declaration** — the plugin's manifest must declare a matching
   permission in the relevant domain. Undeclared → deny.
2. **Argument within declared scope** — the concrete argument must fall inside
   a declared target:
   - filesystem: target path joined to workspace root, canonicalized (resolves
     symlinks, collapses `..`), then `startswith` a declared canonical prefix.
     This single check defeats `../` traversal, symlink escape, and absolute-path
     injection.
   - network: URL `startswith` a declared `fetch:` prefix.
   - shell: command matches a declared `exec:` scope (or is in the `readonly`
     set when `exec:readonly` is declared).
   - env: variable name exactly matches a declared `read:`.
3. **Domain security primitive** — even when manifest-permitted:
   - shell still passes through Shell Gate (risk + scope);
   - network still checked against size / TLS policy;
   - filesystem still canonicalized at the boundary.

Only after all three gates does the call reach the real OS.

### Worked example

Plugin with `filesystem: ["write:output"]` calls
`alius_write_file("output/report.md", content)`:

- join → `$workspace_root/output/report.md`, canonicalize;
- `startswith $workspace_root/output` → ✓;
- audit: `{plugin, action: "write_file", target, allowed: true, trace_id, ts}`;
- `std::fs::write`.

Calls to `alius_write_file("../etc/passwd", ...)`:
- canonicalize → outside workspace → deny + audit `allowed: false`.

Calls to `alius_write_file("secret/key", ...)`:
- canonical `$workspace_root/secret/key`, no `write:` prefix matches → deny.

## Install-time Authorization and Audit

**Install time** — when a plugin is installed, the cli renders the full
permission list and asks the user to confirm:

```
Plugin rust-project v0.1.0 requests:
  filesystem:
    read   project/, config/
    write  output/
    list   project/
  network:
    fetch  https://crates.io/api/v1
  shell:
    exec   readonly (ls/cat/grep/git log/...)
Install? [y/n]
```

Installing = authorizing the declared permission set. Permissions are fixed per
version; upgrading a plugin with new permissions re-prompts.

**Run time** — no permission prompts (the user already authorized at install).
The only run-time prompt is for high-risk shell commands, surfaced through the
normal confirmation flow (see tools-and-shell-gate.md).

**Audit** — every host-function call is logged:
`{trace_id, plugin_id, action, target, allowed, ts}`. Denied calls are logged
with the denial reason. Audit records feed the trace system and can be reviewed
per session.

## Design Principles

1. **Paths relative to workspace root.** Host canonicalizes; manifests stay
   portable across machines.
2. **Read/write declared independently.** Least privilege — a plugin that only
   generates reports declares `write:output` and never gets `write:project`.
3. **Capability domains isolated.** filesystem / network / shell / env do not
   bleed into each other; each host function consults exactly one domain.
4. **Shell is double-checked.** Manifest `exec:` permission **and** Shell Gate;
   neither alone is sufficient.
5. **Authorize at install, verify at run.** Users see the full permission bill
   up front; runtime is deterministic (check + audit, no surprises).

## Relationship to Native Tools

Native built-in tools and plugin host functions share the same security
primitives:

| Entry | Manifest | Security primitive |
|---|---|---|
| Native `shell` tool (cli built-in) | — (user trusts the cli) | Shell Gate |
| Plugin `alius_shell` import | `shell: exec:*` | manifest check → Shell Gate |
| Native `read_file` tool (future) | — | path canonicalize |
| Plugin `alius_read_file` import | `filesystem: read:*` | manifest check → path canonicalize |

Native tools skip the manifest check (the cli itself is trusted by the user);
plugins always go through it. The Shell Gate / path-boundary logic is identical
on both paths.

## Status

Forward design — **not yet implemented**:

- `wasm_host/host.rs` links only host→wasm exports today; no `Linker` imports
  are registered.
- `package.rs` manifest has no `permissions` field.
- No audit sink for host-function calls.

The native `shell` tool (this work) is the first concrete use of the shared
security primitives (Shell Gate). When host imports land, they will reuse the
same primitives rather than re-implementing safety checks.
