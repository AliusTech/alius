//! Rust WASM tool module host core implementation.
//!
//! Loads and executes Rust WASM tool modules via wasmtime. Each module exports:
//! - `alius_plugin_list_tools()` → JSON array of tool definitions
//! - `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len)` → result ptr

use anyhow::{bail, Result};
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

/// A discovered Rust WASM tool module on disk.
#[derive(Debug, Clone)]
pub struct Plugin {
    pub manifest: PluginManifest,
    pub wasm_path: PathBuf,
}

/// Tool definition returned by a Rust WASM tool module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolDef {
    pub name: String,
    pub description: String,
    #[serde(rename = "inputSchema")]
    pub input_schema: serde_json::Value,
    #[serde(default)]
    pub requires_confirmation: bool,
}

/// Configuration for the Wasm host environment.
#[derive(Debug, Clone)]
pub struct WasmHostConfig {
    pub allow_fs: bool,
    pub allow_net: bool,
    pub workspace_root: PathBuf,
}

impl Default for WasmHostConfig {
    fn default() -> Self {
        Self {
            allow_fs: true,
            allow_net: false,
            workspace_root: PathBuf::from("/tmp/wasm-workspace"),
        }
    }
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
        bail!("plugin.toml not found in {}", source_dir.display());
    }
    let wasm_path = source_dir.join("plugin.wasm");
    if !wasm_path.exists() {
        bail!("plugin.wasm not found in {}", source_dir.display());
    }

    let manifest_content = std::fs::read_to_string(&manifest_path)?;
    let manifest: PluginManifest = toml::from_str(&manifest_content)?;

    let dest = plugin_dir().join(&manifest.id);
    std::fs::create_dir_all(&dest)?;
    std::fs::copy(&manifest_path, dest.join("plugin.toml"))?;
    std::fs::copy(&wasm_path, dest.join("plugin.wasm"))?;

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
        bail!("Plugin '{}' is not installed", id);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

/// Validate a WASM module using wasmtime.
pub fn validate_wasm_module(wasm_bytes: &[u8]) -> Result<()> {
    let engine = wasmtime::Engine::default();
    wasmtime::Module::validate(&engine, wasm_bytes)
        .map_err(|e| anyhow::anyhow!("invalid WASM module: {}", e))
}

/// List tools exposed by a Rust WASM tool module.
pub fn list_plugin_tools(wasm_bytes: &[u8]) -> Result<Vec<PluginToolDef>> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_binary(&engine, wasm_bytes)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[])?;

    let list_fn = instance
        .get_typed_func::<(), i32>(&mut store, "alius_plugin_list_tools")
        .map_err(|_| anyhow::anyhow!("Plugin does not export alius_plugin_list_tools"))?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("Plugin does not export memory"))?;

    let result_ptr = list_fn.call(&mut store, ())?;
    let mem_data = memory.data(&store);
    let ptr = result_ptr as usize;
    if ptr + 4 > mem_data.len() {
        bail!("Plugin returned invalid pointer from alius_plugin_list_tools");
    }
    let len = u32::from_le_bytes(mem_data[ptr..ptr + 4].try_into().unwrap()) as usize;
    if ptr + 4 + len > mem_data.len() {
        bail!("Plugin returned string exceeding memory bounds");
    }
    let json_bytes = &mem_data[ptr + 4..ptr + 4 + len];
    let json_str = std::str::from_utf8(json_bytes)?;
    let tools: Vec<PluginToolDef> = serde_json::from_str(json_str)?;
    Ok(tools)
}

/// Call a tool in a Rust WASM tool module.
pub fn call_plugin_tool(
    wasm_bytes: &[u8],
    tool_name: &str,
    args: &serde_json::Value,
) -> Result<serde_json::Value> {
    let engine = wasmtime::Engine::default();
    let module = wasmtime::Module::from_binary(&engine, wasm_bytes)?;
    let mut store = wasmtime::Store::new(&engine, ());
    let instance = wasmtime::Instance::new(&mut store, &module, &[])?;

    let call_fn = instance
        .get_typed_func::<(i32, i32, i32, i32), i32>(&mut store, "alius_plugin_call_tool")
        .map_err(|_| anyhow::anyhow!("Plugin does not export alius_plugin_call_tool"))?;

    let memory = instance
        .get_memory(&mut store, "memory")
        .ok_or_else(|| anyhow::anyhow!("Plugin does not export memory"))?;

    let name_bytes = tool_name.as_bytes();
    let args_str = serde_json::to_string(args)?;
    let args_bytes = args_str.as_bytes();

    let name_ptr: usize = 0;
    let args_ptr: usize = name_bytes.len() + 16;

    let mem = memory.data_mut(&mut store);
    if args_ptr + args_bytes.len() > mem.len() {
        bail!("Plugin memory too small for input data");
    }
    mem[name_ptr..name_ptr + name_bytes.len()].copy_from_slice(name_bytes);
    mem[args_ptr..args_ptr + args_bytes.len()].copy_from_slice(args_bytes);

    let result_ptr = call_fn.call(
        &mut store,
        (
            name_ptr as i32,
            name_bytes.len() as i32,
            args_ptr as i32,
            args_bytes.len() as i32,
        ),
    )?;

    let mem_data = memory.data(&store);
    let ptr = result_ptr as usize;
    if ptr + 4 > mem_data.len() {
        bail!("Plugin returned invalid result pointer");
    }
    let len = u32::from_le_bytes(mem_data[ptr..ptr + 4].try_into().unwrap()) as usize;
    if ptr + 4 + len > mem_data.len() {
        bail!("Plugin returned result exceeding memory bounds");
    }
    let result_bytes = &mem_data[ptr + 4..ptr + 4 + len];
    let result_str = std::str::from_utf8(result_bytes)?;
    let result: serde_json::Value = serde_json::from_str(result_str)?;
    Ok(result)
}

/// Check if a file path is within the workspace sandbox.
pub fn is_path_in_workspace(path: &Path, workspace_root: &Path) -> bool {
    match (path.canonicalize(), workspace_root.canonicalize()) {
        (Ok(p), Ok(root)) => p.starts_with(&root),
        _ => {
            let path_str = path.to_string_lossy();
            let root_str = workspace_root.to_string_lossy();
            path_str.starts_with(root_str.as_ref())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_valid_module() {
        let wasm = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap();
        assert!(validate_wasm_module(&wasm).is_ok());
    }

    #[test]
    fn test_validate_invalid_bytes() {
        assert!(validate_wasm_module(&[0x00, 0x11, 0x22, 0x33]).is_err());
    }

    #[test]
    fn test_validate_too_short() {
        assert!(validate_wasm_module(&[0x00, 0x61]).is_err());
    }

    #[test]
    fn test_path_in_workspace() {
        assert!(is_path_in_workspace(
            Path::new("/workspace/src/main.rs"),
            Path::new("/workspace"),
        ));
        assert!(!is_path_in_workspace(
            Path::new("/etc/passwd"),
            Path::new("/workspace"),
        ));
    }

    #[test]
    fn test_plugin_missing_exports_is_safe() {
        let wasm = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap();
        assert!(validate_wasm_module(&wasm).is_ok());
        assert!(list_plugin_tools(&wasm).is_err());
        assert!(call_plugin_tool(&wasm, "test", &serde_json::json!({})).is_err());
        // Validation still works after failed calls
        assert!(validate_wasm_module(&wasm).is_ok());
    }

    #[test]
    fn test_config_defaults() {
        let config = WasmHostConfig::default();
        assert!(config.allow_fs);
        assert!(!config.allow_net);
    }

    /// Load the hello test plugin WASM from the test data directory.
    fn hello_plugin_wasm() -> Vec<u8> {
        let wat_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("plugins")
            .join("hello")
            .join("hello.wat");
        let wat_source = std::fs::read_to_string(&wat_path)
            .unwrap_or_else(|e| panic!("Failed to read hello.wat: {}", e));
        wat::parse_str(&wat_source).expect("Failed to parse hello.wat")
    }

    #[test]
    fn test_hello_plugin_loads_and_lists_tools() {
        let wasm = hello_plugin_wasm();
        assert!(validate_wasm_module(&wasm).is_ok());

        let tools = list_plugin_tools(&wasm).expect("Failed to list tools");
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].name, "hello");
        assert!(tools[0].description.contains("greeting"));
    }

    #[test]
    fn test_hello_plugin_call_tool() {
        let wasm = hello_plugin_wasm();
        let result = call_plugin_tool(&wasm, "hello", &serde_json::json!({"name": "world"}))
            .expect("Failed to call tool");

        assert_eq!(result["output"].as_str(), Some("Hello, world!"));
        assert_eq!(result["success"].as_bool(), Some(true));
    }
}
