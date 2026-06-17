# CLI Product Surface

The `alius` binary is defined in `entrypoints/cli`. Command definitions live in `entrypoints/cli/src/cli.rs`, and dispatch lives in `entrypoints/cli/src/main.rs`.

## Top-Level Commands

| Command | Current behavior |
| --- | --- |
| `alius` | Starts the interactive workspace. |
| `alius repl` | Explicitly starts the interactive workspace. |
| `alius run -p <prompt>` | Runs a single prompt in non-interactive mode. |
| `alius config` | Shows, validates, and updates config-related state. |
| `alius version` | Prints the compiled `ALIUS_VERSION`. |
| `alius init` | Resets project config defaults and opens the init wizard. |
| `alius core` | Manages the official Soul repository cache. |
| `alius soul` | Manages installed soul cache entries and current project soul display. |
| `alius plugin` | Lists, installs, inspects, and removes Rust WASM tool modules. |
| `alius mcp` | Lists, starts, and inspects MCP server tools. |
| `alius workflow` | Lists, validates, and runs workflow definitions. |

## Global Flags

The CLI type defines these root flags:

| Flag | Current status |
| --- | --- |
| `--model` | Defined at root level; `run --model` is the clearly consumed one-shot override. |
| `--provider` | Defined at root level; do not assume it affects every dispatch path without checking code. |
| `--workspace` | Defined at root level; do not assume it changes current directory or tool workspace on every path. |
| `--config` | Defined at root level; main dispatch currently calls `Settings::load()` directly. |
| `--verbose` | Defined at root level; logging behavior should be verified before documenting it as active. |

## Project Initialization

`alius init`:

1. Loads settings.
2. Checks whether project config exists.
3. Calls `core_runtime::config::reset_project_config`.
4. Opens the TUI init wizard.
5. Saves provider, model, locale, and Agent Card related settings through project config paths.

Project initialization is project-local. It is not a global user profile wizard.

## Single Prompt Execution

`alius run -p <prompt>`:

1. Loads settings.
2. Applies `run --model` if provided.
3. Builds `CoreRuntimeManager`.
4. Sends the prompt through `CoreRuntimeManager -> ProtocolInterface<CoreRuntime> -> CoreRuntime` using Chat mode.
5. Streams model deltas to stdout.

Do not describe this path as a full workspace session UI. It is a one-shot command.

## Config Commands

`alius config` includes:

- `show`
- `validate`
- `soul --role <role>`
- `credential`

The config command can display both legacy settings and the newer project config snapshot when available.

## Soul and Core Commands

`alius core` manages the official Soul repository cache. `alius soul` manages installed soul cache entries.

Important distinction:

- `alius core update` updates or clones the official repository cache.
- `alius soul update` syncs official souls into the local soul cache.
- There is no `alius soul use` command in the current CLI command definition.

## Extension Commands

`alius plugin`, `alius mcp`, and `alius workflow` expose management surfaces. They should not be documented as fully integrated runtime capabilities unless the code path being described explicitly connects them to Core Runtime execution.

## Functional Test Surface

Every CLI command family must have a functional acceptance test. The goal is to catch dispatch, config, filesystem, output, and runtime wiring regressions, not only unit-level parser mistakes.

Required command coverage:

- `alius version` prints a non-empty version and exits successfully.
- `alius --help` and each command-family help page render without panic.
- `alius init` reaches the project-local initialization path inside an isolated temporary workspace and does not write to the developer or CI user's real home directory.
- `alius run -p <prompt>` works through deterministic test providers in normal CI and through a selected-provider smoke test only when provider-test secrets are present.
- `alius config show` and `alius config validate` work against a generated project config fixture.
- `alius config soul --role <role>` validates role selection behavior against a fixture role.
- `alius core` compatibility commands must be tested without making the legacy remote repository the official extension path.
- `alius soul` commands must test bundled-soul sync and local cache behavior without requiring network access.
- `alius plugin` commands must test list, inspect, install, remove, invalid package handling, permission display, and duplicate-name rejection.
- `alius mcp` commands must test list/start/inspect behavior against local fixture servers.
- `alius workflow` commands must test list, validate, dry-run or run behavior, confirmation requirements, and runtime-backed tool execution where applicable.
- `alius repl` and plain `alius` must have non-interactive smoke coverage through the TUI or legacy REPL test harness rather than unbounded terminal sessions.

Functional tests must isolate filesystem state by setting temporary workspace, home, cache, and config paths. They must not mutate `~/.alius`, the repository working tree outside the fixture workspace, or real global credentials.

Network-facing command behavior should be covered in two layers:

- deterministic functional tests against local mock HTTP or MCP servers, required on every CI run;
- selected-provider smoke tests against real model APIs, required only when the configured CI secrets are available and the workflow context is trusted.
