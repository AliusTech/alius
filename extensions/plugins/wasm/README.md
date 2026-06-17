# WASM Plugins

Official WASM plugin source catalog for Alius.

## Building Plugins

Each plugin is a Rust `cdylib` targeting `wasm32-wasip1`:

```bash
# Install the WASM target (if not already installed)
rustup target add wasm32-wasip1

# Build a plugin
cd extensions/plugins/wasm/hello-world
cargo build --target wasm32-wasip1 --release

# The output is at:
# target/wasm32-wasip1/release/hello_world_plugin.wasm
```

## Installing a Plugin

After building, copy the `.wasm` file and `plugin.toml` to `~/.alius/plugins/<id>/`:

```bash
mkdir -p ~/.alius/plugins/hello-world
cp target/wasm32-wasip1/release/hello_world_plugin.wasm ~/.alius/plugins/hello-world/plugin.wasm
cp plugin.toml ~/.alius/plugins/hello-world/plugin.toml
```

## Plugin ABI

Each WASM module must export:
- `alius_plugin_list_tools() -> i32` — returns pointer to length-prefixed JSON array of tool definitions
- `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len) -> i32` — returns pointer to length-prefixed JSON result

Memory layout:
- 4-byte LE length prefix followed by UTF-8 JSON bytes
- Tool definitions: `{name, description, inputSchema, requires_confirmation}`
- Call result: `{success: bool, output: string}`

## Host Imports

Plugins can import functions from the `"alius_host"` namespace:
- `read_file(ptr, len) -> i64` — read a file (permission-gated)
- `write_file(ptr, len) -> i64` — write a file (permission-gated)
- `list_dir(ptr, len) -> i64` — list directory (permission-gated)
- `env_get(ptr, len) -> i64` — read env var (permission-gated)
- `shell(ptr, len) -> i64` — execute shell command (permission-gated + Shell Gate)
- `fetch(ptr, len) -> i64` — HTTP fetch (deny-by-default, not yet implemented)

Each import accepts a JSON string via WASM memory and returns a packed i64 (high 32 = ptr, low 32 = len).

## Status

**Current**: Source catalog. CI binary distribution is not yet implemented.
Plugins must be compiled locally and installed manually.
