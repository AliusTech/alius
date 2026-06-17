//! Alius Plugin — WASM plugin system.
//!
//! Delegates to `runtime_tools::wasm_host` for all plugin operations.

pub use runtime_tools::wasm_host::{Plugin, PluginInstallPlan, PluginManifest, PluginUpgradeInfo};
use std::path::Path;

/// Plan a plugin installation: validate manifest, permissions, and detect upgrades.
///
/// This phase does NOT copy any files. Call [`apply_plugin_install`] after
/// user confirmation to complete the installation.
pub fn plan_plugin_install(source_dir: &Path) -> anyhow::Result<PluginInstallPlan> {
    runtime_tools::wasm_host::plan_plugin_install(source_dir)
}

/// Apply a plugin installation after user confirmation.
pub fn apply_plugin_install(plan: &PluginInstallPlan) -> anyhow::Result<()> {
    runtime_tools::wasm_host::apply_plugin_install(plan)
}

/// Install a plugin from a local directory containing plugin.toml + plugin.wasm.
/// Returns (manifest, permission_summary_lines, upgrade_info).
///
/// **Deprecated**: Use `plan_plugin_install` + `apply_plugin_install` for
/// installations that require user confirmation.
#[allow(dead_code)]
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
