//! Rust WASM tool module host.
//!
//! Provides wasmtime-based execution with module lifecycle management
//! (install, list, find, remove) and tool discovery/call ABI.

pub mod audit;
pub mod host;
pub mod imports;
pub mod tool_wrapper;

pub use audit::{audit_event, HostAuditEvent, HostAuditSink, NoopAuditSink, TracingAuditSink};
pub use host::{
    apply_plugin_install, call_plugin_tool, call_plugin_tool_with_state, find_plugin,
    install_plugin, is_path_in_workspace, list_plugin_tools, list_plugins, plan_plugin_install,
    plugin_dir, remove_plugin, validate_permissions, validate_wasm_module, PermissionDecision,
    Plugin, PluginInstallPlan, PluginManifest, PluginPermissions, PluginToolDef, PluginUpgradeInfo,
    ResolvedPluginPermissions, WasmHostConfig,
};
pub use imports::{build_linker, WasmHostState};
pub use tool_wrapper::WasmPluginTool;
