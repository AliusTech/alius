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
    /// Declared permissions for host capability access.
    /// Defaults to empty (no permissions) for backward compatibility.
    #[serde(default)]
    pub permissions: Option<PluginPermissions>,
}

/// Structured permission declaration for a plugin.
///
/// Each domain lists allowed operations as `"operation:target"` strings.
/// Empty lists mean no access for that domain.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PluginPermissions {
    #[serde(default)]
    pub filesystem: Vec<String>,
    #[serde(default)]
    pub network: Vec<String>,
    #[serde(default)]
    pub shell: Vec<String>,
    #[serde(default)]
    pub env: Vec<String>,
}

/// A single parsed permission entry: `"operation:target"`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedPermission {
    pub operation: String,
    pub target: String,
}

/// Validation errors for a single permission entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionValidationError {
    /// Entry has wrong format (missing colon separator).
    InvalidFormat { entry: String },
    /// Operation is not recognized for this domain.
    UnknownOperation { domain: String, entry: String },
    /// Filesystem target contains `..` traversal.
    PathTraversal { entry: String },
    /// Filesystem target is an absolute path.
    AbsolutePath { entry: String },
    /// Env entry is empty or has empty target.
    EmptyTarget { entry: String },
}

impl std::fmt::Display for PermissionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat { entry } => {
                write!(f, "invalid format (expected 'op:target'): {entry}")
            }
            Self::UnknownOperation { domain, entry } => {
                write!(f, "unknown operation for {domain}: {entry}")
            }
            Self::PathTraversal { entry } => write!(f, "path traversal (..) not allowed: {entry}"),
            Self::AbsolutePath { entry } => write!(f, "absolute paths not allowed: {entry}"),
            Self::EmptyTarget { entry } => write!(f, "empty target not allowed: {entry}"),
        }
    }
}

/// Allowed operations per domain.
const FS_OPS: &[&str] = &["read", "write", "list"];
const NET_OPS: &[&str] = &["fetch"];
const SHELL_OPS: &[&str] = &["exec"];
const ENV_OPS: &[&str] = &["read"];

/// Validate all permission entries in a manifest.
/// Returns Ok(()) if all entries are valid, Err with all errors otherwise.
pub fn validate_permissions(permissions: &PluginPermissions) -> Result<()> {
    let mut errors = Vec::new();

    for entry in &permissions.filesystem {
        if let Err(e) = validate_fs_permission(entry) {
            errors.push(e);
        }
    }
    for entry in &permissions.network {
        if let Err(e) = validate_net_permission(entry) {
            errors.push(e);
        }
    }
    for entry in &permissions.shell {
        if let Err(e) = validate_shell_permission(entry) {
            errors.push(e);
        }
    }
    for entry in &permissions.env {
        if let Err(e) = validate_env_permission(entry) {
            errors.push(e);
        }
    }

    if !errors.is_empty() {
        let messages: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        bail!(
            "Invalid plugin permissions:\n  - {}",
            messages.join("\n  - ")
        );
    }
    Ok(())
}

fn parse_permission(entry: &str) -> Result<ParsedPermission, PermissionValidationError> {
    let (op, target) =
        entry
            .split_once(':')
            .ok_or_else(|| PermissionValidationError::InvalidFormat {
                entry: entry.to_string(),
            })?;
    if target.is_empty() {
        return Err(PermissionValidationError::EmptyTarget {
            entry: entry.to_string(),
        });
    }
    Ok(ParsedPermission {
        operation: op.to_string(),
        target: target.to_string(),
    })
}

fn validate_fs_permission(entry: &str) -> Result<(), PermissionValidationError> {
    let parsed = parse_permission(entry)?;
    if !FS_OPS.contains(&parsed.operation.as_str()) {
        return Err(PermissionValidationError::UnknownOperation {
            domain: "filesystem".to_string(),
            entry: entry.to_string(),
        });
    }
    if parsed.target.contains("..") {
        return Err(PermissionValidationError::PathTraversal {
            entry: entry.to_string(),
        });
    }
    let p = Path::new(&parsed.target);
    if p.is_absolute() {
        return Err(PermissionValidationError::AbsolutePath {
            entry: entry.to_string(),
        });
    }
    Ok(())
}

fn validate_net_permission(entry: &str) -> Result<(), PermissionValidationError> {
    let parsed = parse_permission(entry)?;
    if !NET_OPS.contains(&parsed.operation.as_str()) {
        return Err(PermissionValidationError::UnknownOperation {
            domain: "network".to_string(),
            entry: entry.to_string(),
        });
    }
    Ok(())
}

fn validate_shell_permission(entry: &str) -> Result<(), PermissionValidationError> {
    let parsed = parse_permission(entry)?;
    if !SHELL_OPS.contains(&parsed.operation.as_str()) {
        return Err(PermissionValidationError::UnknownOperation {
            domain: "shell".to_string(),
            entry: entry.to_string(),
        });
    }
    Ok(())
}

fn validate_env_permission(entry: &str) -> Result<(), PermissionValidationError> {
    let parsed = parse_permission(entry)?;
    if !ENV_OPS.contains(&parsed.operation.as_str()) {
        return Err(PermissionValidationError::UnknownOperation {
            domain: "env".to_string(),
            entry: entry.to_string(),
        });
    }
    Ok(())
}

/// A plugin with its resolved permissions (from manifest or default).
#[derive(Debug, Clone, Default)]
pub struct ResolvedPluginPermissions {
    pub filesystem: Vec<String>,
    pub network: Vec<String>,
    pub shell: Vec<String>,
    pub env: Vec<String>,
}

impl From<Option<PluginPermissions>> for ResolvedPluginPermissions {
    fn from(permissions: Option<PluginPermissions>) -> Self {
        match permissions {
            Some(p) => Self {
                filesystem: p.filesystem,
                network: p.network,
                shell: p.shell,
                env: p.env,
            },
            None => Self::default(),
        }
    }
}

impl ResolvedPluginPermissions {
    /// Check if this permission set has any entries.
    pub fn is_empty(&self) -> bool {
        self.filesystem.is_empty()
            && self.network.is_empty()
            && self.shell.is_empty()
            && self.env.is_empty()
    }

    /// Get a human-readable summary of the permissions for install-time display.
    pub fn summary_lines(&self) -> Vec<String> {
        let mut lines = Vec::new();
        if !self.filesystem.is_empty() {
            lines.push(format!("  filesystem: {}", self.filesystem.join(", ")));
        }
        if !self.network.is_empty() {
            lines.push(format!("  network: {}", self.network.join(", ")));
        }
        if !self.shell.is_empty() {
            lines.push(format!("  shell: {}", self.shell.join(", ")));
        }
        if !self.env.is_empty() {
            lines.push(format!("  env: {}", self.env.join(", ")));
        }
        lines
    }
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
///
/// Validates the manifest including permission declarations before copying.
/// Returns an error if the manifest is malformed or permissions are invalid.
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

    // Validate permissions if declared
    if let Some(ref permissions) = manifest.permissions {
        validate_permissions(permissions)?;
    }

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

    // ===== Permission Validation Tests =====

    #[test]
    fn test_empty_permissions_valid() {
        let permissions = PluginPermissions::default();
        assert!(validate_permissions(&permissions).is_ok());
    }

    #[test]
    fn test_valid_filesystem_permissions() {
        let permissions = PluginPermissions {
            filesystem: vec![
                "read:project".to_string(),
                "write:output".to_string(),
                "list:src".to_string(),
            ],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        assert!(validate_permissions(&permissions).is_ok());
    }

    #[test]
    fn test_valid_network_permission() {
        let permissions = PluginPermissions {
            filesystem: vec![],
            network: vec!["fetch:https://crates.io/api/v1".to_string()],
            shell: vec![],
            env: vec![],
        };
        assert!(validate_permissions(&permissions).is_ok());
    }

    #[test]
    fn test_valid_shell_permission() {
        let permissions = PluginPermissions {
            filesystem: vec![],
            network: vec![],
            shell: vec!["exec:readonly".to_string()],
            env: vec![],
        };
        assert!(validate_permissions(&permissions).is_ok());
    }

    #[test]
    fn test_valid_env_permission() {
        let permissions = PluginPermissions {
            filesystem: vec![],
            network: vec![],
            shell: vec![],
            env: vec!["read:HOME".to_string(), "read:CARGO_HOME".to_string()],
        };
        assert!(validate_permissions(&permissions).is_ok());
    }

    #[test]
    fn test_malformed_entry_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec!["no-colon-here".to_string()],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid format"));
    }

    #[test]
    fn test_unknown_fs_operation_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec!["delete:project".to_string()],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("unknown operation"));
    }

    #[test]
    fn test_path_traversal_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec!["read:../etc".to_string()],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("path traversal"));
    }

    #[test]
    fn test_absolute_path_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec!["read:/etc/passwd".to_string()],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("absolute paths"));
    }

    #[test]
    fn test_empty_target_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec!["read:".to_string()],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty target"));
    }

    #[test]
    fn test_wildcard_env_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec![],
            network: vec![],
            shell: vec![],
            env: vec!["read:*".to_string()],
        };
        // Wildcards are allowed in env - only empty targets are rejected
        assert!(validate_permissions(&permissions).is_ok());
    }

    #[test]
    fn test_empty_env_rejected() {
        let permissions = PluginPermissions {
            filesystem: vec![],
            network: vec![],
            shell: vec![],
            env: vec!["read:".to_string()],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("empty target"));
    }

    #[test]
    fn test_multiple_errors_collected() {
        let permissions = PluginPermissions {
            filesystem: vec!["read:../etc".to_string(), "delete:src".to_string()],
            network: vec![],
            shell: vec![],
            env: vec![],
        };
        let result = validate_permissions(&permissions);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("path traversal"));
        assert!(err_msg.contains("unknown operation"));
    }

    #[test]
    fn test_old_manifest_without_permissions_parses() {
        let toml_str = r#"
id = "old-plugin"
name = "Old Plugin"
version = "1.0.0"
description = "An old plugin without permissions"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert_eq!(manifest.id, "old-plugin");
        assert!(manifest.permissions.is_none());
        let resolved: ResolvedPluginPermissions = manifest.permissions.into();
        assert!(resolved.is_empty());
    }

    #[test]
    fn test_new_manifest_with_permissions_parses() {
        let toml_str = r#"
id = "new-plugin"
name = "New Plugin"
version = "2.0.0"
description = "A plugin with permissions"

[permissions]
filesystem = ["read:project", "write:output"]
network = ["fetch:https://api.example.com"]
shell = ["exec:readonly"]
env = ["read:HOME"]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.permissions.is_some());
        let perms = manifest.permissions.unwrap();
        assert_eq!(perms.filesystem.len(), 2);
        assert_eq!(perms.network.len(), 1);
        assert_eq!(perms.shell.len(), 1);
        assert_eq!(perms.env.len(), 1);
        assert!(validate_permissions(&perms).is_ok());
    }

    #[test]
    fn test_malformed_toml_permissions_rejected() {
        let toml_str = r#"
id = "bad-plugin"
name = "Bad Plugin"
version = "1.0.0"
description = "Invalid permissions"

[permissions]
filesystem = "not-a-list"
"#;
        let result: std::result::Result<PluginManifest, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_permission_field_rejected() {
        let toml_str = r#"
id = "bad-plugin"
name = "Bad Plugin"
version = "1.0.0"
description = "Unknown field"

[permissions]
filesystem = ["read:project"]
unknown_domain = ["something"]
"#;
        let result: std::result::Result<PluginManifest, _> = toml::from_str(toml_str);
        assert!(result.is_err());
    }

    #[test]
    fn test_resolved_permissions_summary_lines() {
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string(), "write:output".to_string()],
            network: vec!["fetch:https://api.example.com".to_string()],
            shell: vec![],
            env: vec!["read:HOME".to_string()],
        };
        let lines = perms.summary_lines();
        assert!(lines.len() == 3); // filesystem, network, env (shell empty)
        assert!(lines[0].contains("filesystem"));
        assert!(lines[1].contains("network"));
        assert!(lines[2].contains("env"));
    }
}
