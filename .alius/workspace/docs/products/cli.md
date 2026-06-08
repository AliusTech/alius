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
