//! Alius Plugin — WASM plugin system.
//!
//! Loads and executes WASM plugins that extend the tool system.
//! Each plugin exposes tools via a simple ABI:
//! - `alius_plugin_list_tools()` → JSON array of tool definitions
//! - `alius_plugin_call_tool(name, args_json)` → JSON result

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Plugin metadata from plugin.toml.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    #[serde(default)]
    pub author: Option<String>,
}

/// A loaded WASM plugin.
pub struct Plugin {
    pub manifest: PluginManifest,
    pub wasm_path: PathBuf,
}

/// Get the plugin installation directory (~/.alius/plugins/).
pub fn plugin_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".alius").join("plugins")
}

/// Install a plugin from a local directory containing plugin.toml + plugin.wasm.
pub fn install_plugin(source_dir: &Path) -> Result<PluginManifest> {
    let manifest_path = source_dir.join("plugin.toml");
    if !manifest_path.exists() {
        anyhow::bail!("plugin.toml not found in {}", source_dir.display());
    }
    let wasm_path = source_dir.join("plugin.wasm");
    if !wasm_path.exists() {
        anyhow::bail!("plugin.wasm not found in {}", source_dir.display());
    }

    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    let manifest: PluginManifest = toml::from_str(&manifest_content)?;

    let dest = plugin_dir().join(&manifest.id);
    std::fs::create_dir_all(&dest)?;
    std::fs::copy(&manifest_path, dest.join("plugin.toml"))?;
    std::fs::copy(&wasm_path, dest.join("plugin.wasm"))?;

    // Copy schemas/ if present
    let schemas_dir = source_dir.join("schemas");
    if schemas_dir.exists() {
        let dest_schemas = dest.join("schemas");
        std::fs::create_dir_all(&dest_schemas)?;
        for entry in std::fs::read_dir(&schemas_dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                std::fs::copy(entry.path(), dest_schemas.join(entry.file_name()))?;
            }
        }
    }

    Ok(manifest)
}

/// List all installed plugins.
pub fn list_plugins() -> Result<Vec<Plugin>> {
    let dir = plugin_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut plugins = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let manifest_path = path.join("plugin.toml");
        let wasm_path = path.join("plugin.wasm");
        if manifest_path.exists() && wasm_path.exists() {
            let content = std::fs::read_to_string(&manifest_path)?;
            if let Ok(manifest) = toml::from_str::<PluginManifest>(&content) {
                plugins.push(Plugin {
                    manifest,
                    wasm_path,
                });
            }
        }
    }

    plugins.sort_by(|a, b| a.manifest.id.cmp(&b.manifest.id));
    Ok(plugins)
}

/// Find a plugin by ID.
pub fn find_plugin(id: &str) -> Result<Option<Plugin>> {
    let plugins = list_plugins()?;
    Ok(plugins.into_iter().find(|p| p.manifest.id == id))
}

/// Remove an installed plugin.
pub fn remove_plugin(id: &str) -> Result<()> {
    let dir = plugin_dir().join(id);
    if !dir.exists() {
        anyhow::bail!("Plugin '{}' is not installed", id);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

/// Load a WASM plugin and call its functions.
///
/// This is the runtime that executes WASM plugins.
pub fn call_plugin_tool(
    plugin: &Plugin,
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_file(&engine, &plugin.wasm_path)?;

    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[])?;

    // Call alius_plugin_call_tool(name, args_json)
    let call_tool = instance
        .get_typed_func::<(i32, i32, i32, i32), i32>(&mut store, "alius_plugin_call_tool")
        .map_err(|_| anyhow::anyhow!("Plugin does not export alius_plugin_call_tool"))?;

    // For now, use a simple approach: pass args as JSON string via memory
    // In a real implementation, we'd use WASI or shared memory
    let args_str = serde_json::to_string(args)?;
    let name_bytes = tool_name.as_bytes();
    let args_bytes = args_str.as_bytes();

    // Allocate memory for the call
    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("Plugin does not export memory"))?;

    let name_ptr = 0;
    let args_ptr = name_bytes.len() as i32 + 16; // offset

    // Write name
    memory.data_mut(&mut store)[name_ptr..name_ptr + name_bytes.len()].copy_from_slice(name_bytes);
    // Write args
    memory.data_mut(&mut store)[args_ptr as usize..args_ptr as usize + args_bytes.len()]
        .copy_from_slice(args_bytes);

    let result_ptr = call_tool.call(
        &mut store,
        (
            name_ptr as i32,
            name_bytes.len() as i32,
            args_ptr,
            args_bytes.len() as i32,
        ),
    )?;

    // Read result from memory
    let result_len = memory.data(&store)[result_ptr as usize] as usize;
    let result_bytes =
        &memory.data(&store)[result_ptr as usize + 4..result_ptr as usize + 4 + result_len];
    let result_str = std::str::from_utf8(result_bytes)?;
    let result: serde_json::Value = serde_json::from_str(result_str)?;

    Ok(result)
}
