# Security Audit Exceptions

This document records known security audit advisories that are temporarily
ignored in CI. Each entry includes the reason, impact assessment, responsible
owner, and mandatory review date.

All entries must be reviewed at least quarterly. An advisory must be removed
from the ignore list as soon as a patched dependency version is available and
tested.

## Active Exceptions

### RUSTSEC-2025-0012 — `backoff` (unmaintained)

- **Crate:** `backoff` 0.4.0
- **Advisory:** https://rustsec.org/advisories/RUSTSEC-2025-0012
- **Type:** Unmaintained
- **Impact:** Low — `backoff` is used for retry logic in provider transport.
  No known vulnerability; the crate is simply no longer maintained.
- **Mitigation:** Evaluate `backon` or `exponential-backoff` as replacements.
  The retry surface is small and well-isolated.
- **Owner:** Platform team
- **Review date:** 2026-09-01

### RUSTSEC-2024-0384 — `instant` (unmaintained)

- **Crate:** `instant` 0.1.13
- **Advisory:** https://rustsec.org/advisories/RUSTSEC-2024-0384
- **Type:** Unmaintained
- **Impact:** Low — transitive dependency via `backoff`. Not used directly.
  The `web-time` crate is the recommended replacement but requires upstream
  updates.
- **Mitigation:** Resolves when `backoff` is replaced (see RUSTSEC-2025-0012).
- **Owner:** Platform team
- **Review date:** 2026-09-01

### RUSTSEC-2024-0436 — `paste` (unmaintained)

- **Crate:** `paste` 1.0.15
- **Advisory:** https://rustsec.org/advisories/RUSTSEC-2024-0436
- **Type:** Unmaintained
- **Impact:** Low — `paste` is a proc-macro for identifier concatenation,
  used in macro definitions. No runtime vulnerability. The functionality
  can be replaced with `macro_metavar_expr` (nightly) or manual expansion.
- **Mitigation:** Monitor stabilization of `macro_metavar_expr`. Replace
  when stabilized or when a maintained fork appears.
- **Owner:** Platform team
- **Review date:** 2026-09-01

### RUSTSEC-2026-0002 — `lru` (unsound)

- **Crate:** `lru` 0.12.5
- **Advisory:** https://rustsec.org/advisories/RUSTSEC-2026-0002
- **Type:** Unsound — `IterMut` violates Stacked Borrows by invalidating
  internal pointer.
- **Impact:** Medium — used for LLM client response caching. The unsound
  code path is `IterMut`, which is not called in our usage (we only use
  `get`, `put`, `pop`). However, the unsoundness could cause UB under
  future compiler optimizations.
- **Mitigation:** Evaluate `lru` 0.13+ (if fixed) or switch to
  `scc`/`quick_cache`. The cache surface is isolated in
  `runtime/model/src/client_cache.rs`.
- **Owner:** Runtime team
- **Review date:** 2026-07-15

## Process

1. When a new advisory appears, check if it affects our direct or transitive
   dependencies.
2. If a fix is available (upgrade path), apply it immediately.
3. If no fix is available, add an entry to this document with:
   - Crate name, version, advisory URL
   - Type (unmaintained / unsound / vulnerability)
   - Impact assessment specific to our usage
   - Mitigation plan
   - Owner and review date
4. Add the advisory ID to the `--ignore` list in `ci.yml` and `release.yml`.
5. Review all exceptions quarterly; remove entries when resolved.
