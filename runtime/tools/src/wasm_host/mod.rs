//! Rust WASM tool module host.
//!
//! Provides wasmtime-based execution with module lifecycle management
//! (install, list, find, remove) and tool discovery/call ABI.

pub mod host;
pub mod tool_wrapper;

pub use host::{
    call_plugin_tool, find_plugin, install_plugin, is_path_in_workspace, list_plugin_tools,
    list_plugins, plugin_dir, remove_plugin, validate_permissions, validate_wasm_module, Plugin,
    PluginManifest, PluginPermissions, PluginToolDef, ResolvedPluginPermissions, WasmHostConfig,
};
pub use tool_wrapper::WasmPluginTool;
