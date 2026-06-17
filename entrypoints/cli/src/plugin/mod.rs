//! Alius Plugin — WASM plugin system.
//!
//! Delegates to `runtime_tools::wasm_host` for all plugin operations.

pub use runtime_tools::wasm_host::{Plugin, PluginManifest, PluginUpgradeInfo};
use std::path::Path;

/// Install a plugin from a local directory containing plugin.toml + plugin.wasm.
/// Returns (manifest, permission_summary_lines, upgrade_info).
pub fn install_plugin(
    source_dir: &Path,
) -> anyhow::Result<(PluginManifest, Vec<String>, Option<PluginUpgradeInfo>)> {
    runtime_tools::wasm_host::install_plugin(source_dir)
}

/// List all installed plugins.
pub fn list_plugins() -> anyhow::Result<Vec<Plugin>> {
    runtime_tools::wasm_host::list_plugins()
}

/// Find a plugin by ID.
pub fn find_plugin(id: &str) -> anyhow::Result<Option<Plugin>> {
    runtime_tools::wasm_host::find_plugin(id)
}

/// Remove an installed plugin.
pub fn remove_plugin(id: &str) -> anyhow::Result<()> {
    runtime_tools::wasm_host::remove_plugin(id)
}
