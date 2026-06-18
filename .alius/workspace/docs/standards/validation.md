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

## Testing Feature Isolation

Test-only helpers must be isolated from release binaries.

Unit tests that do not need shared helpers should stay behind `#[cfg(test)]`. Shared helpers may be exposed only through test-gated modules:

```rust
#[cfg(any(test, feature = "testing"))]
pub mod testing;
```

Allowed helper module locations:

- `runtime/model/src/testing.rs`
- `runtime/tools/src/testing.rs`
- `runtime/core/src/testing.rs`
- `entrypoints/cli/src/testing.rs`
- `entrypoints/cli/src/tui/testing.rs`

Product code must not import testing helpers from normal code paths. Any test-helper import must be gated:

```rust
#[cfg(any(test, feature = "testing"))]
use crate::testing::FakeProvider;
```

The standard test command for helper-backed tests is:

```bash
cargo test --workspace --features testing --locked
```

The standard release build command is:

```bash
cargo build -p alius-cli --bin alius --release --locked
```

Release builds must not use:

```bash
cargo build --release --all-features
cargo build --release --features testing
```

After the release build, CI must fail if test-only symbols are found in the final binary:

```bash
if strings target/release/alius | grep -E "FakeProvider|FakeTool|CoreRuntimeHarness|TuiTestHarness|VecEventSource|testing::|testkit"; then
  echo "::error::Test-only symbols found in release binary"
  exit 1
fi
```

## CI-Native Test And Coverage Reports

Alius CI must keep test and coverage reports inside the CI system's native logs, job summaries, and artifacts. Do not add Codecov, Coveralls, SonarCloud, third-party PR comment actions, third-party test result actions, or third-party coverage badge services.

Allowed reporting surfaces:

- CI logs.
- Native job summary pages.
- CI workflow artifacts.
- Official checkout actions for the CI platform.
- Official artifact upload actions for the CI platform.

The main CI workflow should run a single test-and-coverage job before release build smoke:

1. Check out the repository.
2. Install the stable Rust toolchain with `rustfmt`, `clippy`, and `llvm-tools-preview`.
3. Install `cargo-llvm-cov` with `--locked`.
4. Run `cargo fmt --all -- --check`.
5. Run `cargo clippy --workspace --all-targets --features testing -- -D warnings`.
6. Run `cargo test --workspace --features testing --locked -- --nocapture` and capture the full log under `target/ci-reports/test.log`.
7. Parse `test result:` lines into `target/ci-reports/test-summary.env`.
8. Generate coverage reports with `cargo llvm-cov --workspace --features testing`, excluding `tests/`, `testing.rs`, and `testkit` paths via `--ignore-filename-regex`. The same regex must be passed to every `cargo llvm-cov report` invocation.
9. Write a Markdown summary to `GITHUB_STEP_SUMMARY`.
10. Upload test logs, summary files, LCOV, coverage summary, and HTML coverage as GitHub artifacts.
11. Enforce the staged coverage threshold (see below).

Recommended report artifacts:

- `target/ci-reports/test.log`
- `target/ci-reports/test-result-lines.txt`
- `target/ci-reports/test-summary.env`
- `target/llvm-cov/summary.txt`
- `target/llvm-cov/lcov.info`
- `target/llvm-cov/html`

CI artifacts should be retained for 14 days. Release test-gate artifacts should be retained for 30 days.

The release workflow must run a `test-gate` job before creating a GitHub Release. The `test-gate` job must include version consistency checks, formatting, clippy, helper-backed tests, coverage report generation, coverage threshold enforcement, release build smoke, and the test-only symbol scan. The GitHub Release creation job must depend on both version resolution and `test-gate`.

Release build smoke must use:

```bash
cargo build -p alius-cli --bin alius --release --locked
```

It must not use `--all-features` or `--features testing`.

### Staged Coverage Threshold

The final coverage target is `--fail-under-lines 85`. Because the codebase was not built with coverage instrumentation from the start, reaching 85% requires staged effort. The threshold is enforced at each stage as follows:

| Stage | Threshold | Baseline | Target date | Criteria |
|-------|-----------|----------|-------------|----------|
| 0 (current) | 65% | 67.5% | 2026-06 | Established with TUI TestKit and initial state-machine tests |
| 1 | 70% | — | 2026-07 | Cover remaining CLI dispatch, config loader, plugin install/remove |
| 2 | 75% | — | 2026-08 | Cover Core Runtime loop engine, session manager, tool execution |
| 3 | 80% | — | 2026-09 | Cover MCP protocol, WASM host imports, workflow engine |
| 4 | 85% | — | 2026-10 | Cover JSON-RPC surface, provider error mapping, TUI full state machine |

Each stage must update the `--fail-under-lines` value in both `ci.yml` and `release.yml`. A stage cannot be skipped. If a stage target is missed, the threshold stays at the previous stage value until the target is met.

Coverage exclusion regex (applied to all `cargo llvm-cov report` commands):

```
--ignore-filename-regex '(/tests/|/testing\.rs$|/testkit/|state_machine_tests\.rs$)'
```

This excludes integration test files, shared testing modules, testkit code, and state-machine test files from line coverage calculations, since those files are test infrastructure, not production logic.

This policy forbids third-party test report services, but it does not forbid normal CI dependency downloads such as `rustup`, crates.io dependencies, or `cargo install cargo-llvm-cov`. If the project later requires offline CI, the workflow must move to self-hosted runners, preinstalled tools, `cargo vendor`, and offline Cargo commands.

## Functional And Provider Network Tests

Alius CI must separate deterministic functional coverage from selected-provider network smoke coverage.

### Required On Every CI Run

Every pull request and branch push must run the stable test gate before any release build:

1. Parser and unit tests.
2. Core Runtime, Protocol Interface, Session Manager, Loop Engine, tool, plugin, MCP, workflow, and JSON-RPC tests.
3. CLI command functional tests for every command family documented in `docs/products/cli.md`.
4. TUI state-machine tests through `TuiTestHarness`.
5. Network-facing behavior through local mock HTTP servers or local fixture MCP servers.
6. Shell, filesystem, network, env, confirmation, and permission-denial tests.
7. Release build smoke without the `testing` feature.

These tests must be deterministic. They may bind to loopback addresses, spawn local fixture processes, and use temporary directories, but they must not depend on public network availability or a selected external provider.

### Selected Provider Smoke Tests

Selected-provider smoke tests are allowed when CI secrets are configured. They are not a separate product mode. They are a small CI verification step that uses the configured provider, such as the project-default DeepSeek provider, to prove that configuration loading, credential lookup, transport, provider response parsing, and streaming compatibility still work.

Recommended CI secrets:

- `ALIUS_PROVIDER_SMOKE`
- `ALIUS_TEST_PROVIDER`
- `ALIUS_TEST_API_MODE`
- `ALIUS_TEST_BASE_URL`
- `ALIUS_TEST_API_KEY`
- `ALIUS_TEST_MODEL`

`ALIUS_PROVIDER_SMOKE` must be set to `1` before selected-provider tests run. If it is absent, normal PR CI may report provider smoke tests as skipped, but release workflows may treat missing provider-test configuration as a release-blocking condition once provider smoke is part of the release policy.

Provider smoke coverage should include:

- default-provider configuration loading, with DeepSeek covered when it is the configured project default;
- environment-backed credential resolution;
- representative configuration flow tests that create a temporary project config, set provider/model assignment, validate the config, and run one command through that config;
- one minimal non-streaming chat request with a low token cap;
- one streaming chat request if the provider and API mode support streaming;
- provider error handling for an intentionally invalid model or invalid endpoint against a mock server, not by wasting live provider quota;
- timeout and cancellation behavior through deterministic harnesses.

Provider smoke tests must use short prompts that do not require semantic judgment. The assertion should check transport success, response shape, non-empty text, and correct error mapping. Do not assert exact natural-language text.

### Secret And Log Safety

Provider network tests must never print API keys, authorization headers, raw provider request bodies with secrets, or full provider error payloads if they can include credentials. CI logs and artifacts must redact:

- `Authorization` headers;
- API key query parameters;
- provider-specific key fields;
- generated config files that contain secret values;
- full environment dumps.

All provider requests must have explicit timeouts, low token limits, and bounded retries. A provider outage should fail the provider smoke job clearly, but it must not hide unit or deterministic functional test results.

### Workflow Policy

Recommended workflow split:

- `test`: mandatory deterministic unit and functional gate.
- `coverage`: mandatory coverage generation and threshold enforcement.
- `release-build-smoke`: mandatory release binary build and test-symbol scan, depending on `test`.
- `provider-smoke`: selected-provider smoke, enabled only in trusted contexts with secrets.
- `release-test-gate`: release workflow gate that requires deterministic tests, coverage, release build smoke, and selected-provider smoke when the release policy enables provider tests.

Secrets are not available to untrusted fork pull requests. Do not design CI so that an external fork PR can exfiltrate `ALIUS_TEST_API_KEY`. For public or fork-based workflows, provider smoke tests must run only on trusted branch pushes, scheduled jobs, manual dispatch, or protected CI environments.
