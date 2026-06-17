//! Rust WASM tool module host core implementation.
//!
//! Loads and executes Rust WASM tool modules via wasmtime. Each module exports:
//! - `alius_plugin_list_tools()` → JSON array of tool definitions
//! - `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len)` → result ptr

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::{Component, Path, PathBuf};

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
    /// Env target contains wildcard or is not a valid env var name.
    InvalidEnvVarName { entry: String },
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
            Self::InvalidEnvVarName { entry } => {
                write!(
                    f,
                    "invalid env var name (must be [A-Za-z_][A-Za-z0-9_]*): {entry}"
                )
            }
        }
    }
}

/// Allowed operations per domain.
const FS_OPS: &[&str] = &["read", "write", "list"];
const NET_OPS: &[&str] = &["fetch"];
const SHELL_OPS: &[&str] = &["exec"];
const ENV_OPS: &[&str] = &["read"];

/// Validate all permission entries in a manifest.
/// Returns Ok(warnings) if all entries are valid, Err with all errors otherwise.
pub fn validate_permissions(permissions: &PluginPermissions) -> Result<Vec<String>> {
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
    Ok(Vec::new())
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
    // Reject wildcards and validate env var name format.
    // Valid: HOME, CARGO_HOME, MY_VAR
    // Invalid: *, HOME*, *HOME, my-var, 123
    if parsed.target.contains('*') || !is_valid_env_var_name(&parsed.target) {
        return Err(PermissionValidationError::InvalidEnvVarName {
            entry: entry.to_string(),
        });
    }
    Ok(())
}

/// Validate that a string is a valid environment variable name.
/// Must start with [A-Za-z_] and contain only [A-Za-z0-9_].
fn is_valid_env_var_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() && first != '_' {
        return false;
    }
    chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// A plugin with its resolved permissions (from manifest or default).
#[derive(Debug, Clone, Default, PartialEq)]
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

    /// Check whether a filesystem operation is allowed by this permission set.
    ///
    /// `operation` is one of `"read"`, `"write"`, `"list"`.
    /// `path` is the requested path **relative to workspace root**.
    /// `workspace_root` is the absolute workspace root on the host filesystem.
    ///
    /// The path is canonicalized (resolving symlinks and `..`) and checked to
    /// ensure it stays within the workspace boundary. A declared permission
    /// prefix matches if the canonical path equals the prefix or is a child of it.
    pub fn check_filesystem(
        &self,
        operation: &str,
        path: &Path,
        workspace_root: &Path,
    ) -> PermissionDecision {
        // Absolute paths are always rejected.
        if path.is_absolute() {
            return PermissionDecision::deny("absolute paths are not allowed");
        }

        if self.filesystem.is_empty() {
            return PermissionDecision::deny("no filesystem permissions declared");
        }

        let full_path = workspace_root.join(path);

        // Canonicalize to resolve symlinks and `..`. If canonicalization fails
        // (path doesn't exist yet), fall back to lexical normalization.
        let (canonical, ws_canonical) =
            match (full_path.canonicalize(), workspace_root.canonicalize()) {
                (Ok(canon), Ok(ws)) => (canon, ws),
                _ => (
                    normalize_lexical(&full_path),
                    normalize_lexical(workspace_root),
                ),
            };

        // Reject paths that escape the workspace boundary.
        if !canonical.starts_with(&ws_canonical) {
            return PermissionDecision::deny("path escapes workspace boundary");
        }

        // Check each declared permission for a prefix match.
        for entry in &self.filesystem {
            let (op, target) = match entry.split_once(':') {
                Some(pair) => pair,
                None => continue,
            };
            if op != operation {
                continue;
            }

            let prefix_canonical = match workspace_root.join(target).canonicalize() {
                Ok(p) => p,
                _ => normalize_lexical(&workspace_root.join(target)),
            };

            if canonical == prefix_canonical
                || canonical.starts_with(
                    prefix_canonical
                        .join("__placeholder__")
                        .parent()
                        .unwrap_or(&prefix_canonical),
                )
            {
                // More precise check: path must start with prefix + "/" or equal prefix.
                if canonical == prefix_canonical
                    || canonical.starts_with(format!(
                        "{}{}",
                        prefix_canonical.display(),
                        std::path::MAIN_SEPARATOR
                    ))
                {
                    return PermissionDecision::Allow {
                        resolved_path: canonical,
                    };
                }
            }
        }

        PermissionDecision::deny(&format!(
            "filesystem {operation} not permitted for path '{}'",
            path.display()
        ))
    }

    /// Check whether a network request is allowed by this permission set.
    ///
    /// The URL is prefix-matched against declared `fetch:<prefix>` entries.
    pub fn check_network(&self, url: &str) -> PermissionDecision {
        if self.network.is_empty() {
            return PermissionDecision::deny("no network permissions declared");
        }

        for entry in &self.network {
            let (op, prefix) = match entry.split_once(':') {
                Some(pair) => pair,
                None => continue,
            };
            if op != "fetch" {
                continue;
            }
            // Exact match, or prefix followed by a path/query separator.
            // This prevents "https://api.example.com.evil.com" from matching
            // a declared prefix of "https://api.example.com".
            if url == prefix
                || url.starts_with(prefix)
                    && url
                        .as_bytes()
                        .get(prefix.len())
                        .map(|&b| b == b'/' || b == b'?')
                        .unwrap_or(false)
            {
                return PermissionDecision::allow_no_path();
            }
        }

        PermissionDecision::deny(&format!("network fetch not permitted for URL '{url}'"))
    }

    /// Check whether a shell command is allowed by this permission set.
    ///
    /// `exec:readonly` allows commands in the read-only set (ls, cat, grep, git, etc.).
    /// `exec:<literal>` allows the exact command string only.
    pub fn check_shell(&self, command: &str) -> PermissionDecision {
        if self.shell.is_empty() {
            return PermissionDecision::deny("no shell permissions declared");
        }

        let trimmed = command.trim();
        let base_command = trimmed.split_whitespace().next().unwrap_or("");

        for entry in &self.shell {
            let (op, target) = match entry.split_once(':') {
                Some(pair) => pair,
                None => continue,
            };
            if op != "exec" {
                continue;
            }

            if target == "readonly" {
                if READONLY_SHELL_COMMANDS.contains(&base_command) {
                    return PermissionDecision::allow_no_path();
                }
            } else if trimmed == target {
                return PermissionDecision::allow_no_path();
            }
        }

        PermissionDecision::deny(&format!("shell command not permitted: '{command}'"))
    }

    /// Check whether reading an environment variable is allowed by this permission set.
    ///
    /// Variable names must match exactly — no wildcards, no prefixes.
    pub fn check_env(&self, var_name: &str) -> PermissionDecision {
        if var_name.is_empty() {
            return PermissionDecision::deny("empty environment variable name");
        }

        if self.env.is_empty() {
            return PermissionDecision::deny("no env permissions declared");
        }

        for entry in &self.env {
            let (op, target) = match entry.split_once(':') {
                Some(pair) => pair,
                None => continue,
            };
            if op != "read" {
                continue;
            }
            if target == var_name {
                return PermissionDecision::allow_no_path();
            }
        }

        PermissionDecision::deny(&format!("env read not permitted for '{var_name}'"))
    }
}

/// Result of a runtime permission check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PermissionDecision {
    /// The operation is allowed.
    ///
    /// For filesystem checks, `resolved_path` contains the canonicalized
    /// absolute path that was verified against the workspace boundary.
    /// Callers MUST use this path for execution instead of re-joining,
    /// to avoid TOCTOU between check and execution.
    Allow { resolved_path: std::path::PathBuf },
    /// The operation is denied with a human-readable reason.
    Deny { reason: String },
}

impl PermissionDecision {
    /// Create a `Deny` decision with the given reason.
    pub fn deny(reason: &str) -> Self {
        Self::Deny {
            reason: reason.to_string(),
        }
    }

    /// Create an `Allow` decision with an empty resolved path (for non-fs checks).
    pub fn allow_no_path() -> Self {
        Self::Allow {
            resolved_path: std::path::PathBuf::new(),
        }
    }

    /// Returns `true` if the decision is `Allow`.
    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allow { .. })
    }

    /// Returns the resolved path if this is an `Allow` decision.
    pub fn resolved_path(&self) -> Option<&std::path::Path> {
        match self {
            Self::Allow { resolved_path } => Some(resolved_path),
            Self::Deny { .. } => None,
        }
    }
}

/// Shell commands allowed under `exec:readonly` permission.
///
/// Aligns with `LOW_RISK_COMMANDS` in shell_gate/inspector.rs plus `git`
/// (which has subcommand-level risk classification in Shell Gate).
const READONLY_SHELL_COMMANDS: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "find", "wc", "sort", "uniq", "diff", "echo", "pwd",
    "whoami", "which", "type", "stat", "file", "tree", "rg", "ag", "fd", "bat", "git",
];

/// Resolve `.` and `..` components in a path without filesystem access.
fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
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

/// Result of the planning phase for plugin installation.
///
/// Contains all information needed to display a confirmation prompt
/// and to execute the installation if approved.
#[derive(Debug)]
pub struct PluginInstallPlan {
    pub manifest: PluginManifest,
    pub summary: Vec<String>,
    pub upgrade_info: Option<PluginUpgradeInfo>,
    /// Resolved source paths for the apply phase.
    source_manifest: std::path::PathBuf,
    source_wasm: std::path::PathBuf,
}

/// Plan a plugin installation: validate manifest, permissions, and detect upgrades.
///
/// This phase does NOT copy any files. Call [`apply_plugin_install`] after
/// user confirmation to complete the installation.
pub fn plan_plugin_install(source_dir: &Path) -> Result<PluginInstallPlan> {
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

    // Validate WASM module is valid (non-empty, reasonable size, valid structure)
    let wasm_metadata = std::fs::metadata(&wasm_path)?;
    if wasm_metadata.len() == 0 {
        bail!("plugin.wasm is empty");
    }
    if wasm_metadata.len() > 100 * 1024 * 1024 {
        bail!(
            "plugin.wasm is too large ({}MB, max 100MB)",
            wasm_metadata.len() / (1024 * 1024)
        );
    }

    // Validate WASM module structure using wasmtime
    let wasm_bytes = std::fs::read(&wasm_path)?;
    validate_wasm_module(&wasm_bytes)?;

    // Validate permissions if declared, collect warnings
    let warnings = if let Some(ref permissions) = manifest.permissions {
        validate_permissions(permissions)?
    } else {
        Vec::new()
    };

    // Build permission summary for install-time display
    let resolved: ResolvedPluginPermissions = manifest.permissions.clone().into();
    let mut summary = resolved.summary_lines();
    // Append warnings to summary
    for w in &warnings {
        summary.push(format!("  WARNING: {}", w));
    }

    // Check for existing installation (upgrade detection)
    let dest = plugin_dir().join(&manifest.id);
    let upgrade_info = if dest.exists() {
        let existing_manifest_path = dest.join("plugin.toml");
        if existing_manifest_path.exists() {
            let existing_content = std::fs::read_to_string(&existing_manifest_path)?;
            let existing: PluginManifest = toml::from_str(&existing_content)?;
            let existing_resolved: ResolvedPluginPermissions = existing.permissions.into();
            let new_resolved: ResolvedPluginPermissions = manifest.permissions.clone().into();

            let permissions_changed = existing_resolved != new_resolved;
            Some(PluginUpgradeInfo {
                old_version: existing.version,
                new_version: manifest.version.clone(),
                permissions_changed,
            })
        } else {
            None
        }
    } else {
        None
    };

    Ok(PluginInstallPlan {
        manifest,
        summary,
        upgrade_info,
        source_manifest: manifest_path,
        source_wasm: wasm_path,
    })
}

/// Apply a plugin installation after user confirmation.
///
/// Copies `plugin.toml` and `plugin.wasm` from the source directory
/// to the plugin installation directory. For upgrades, the old plugin
/// is overwritten atomically (new files replace old files).
pub fn apply_plugin_install(plan: &PluginInstallPlan) -> Result<()> {
    let dest = plugin_dir().join(&plan.manifest.id);
    std::fs::create_dir_all(&dest)?;
    std::fs::copy(&plan.source_manifest, dest.join("plugin.toml"))?;
    std::fs::copy(&plan.source_wasm, dest.join("plugin.wasm"))?;
    Ok(())
}

/// Install a plugin from a local directory containing plugin.toml + plugin.wasm.
///
/// Validates the manifest including permission declarations before copying.
/// Returns an error if the manifest is malformed or permissions are invalid.
///
/// **Deprecated**: Use `plan_plugin_install` + `apply_plugin_install` for
/// installations that require user confirmation.
pub fn install_plugin(
    source_dir: &Path,
) -> Result<(PluginManifest, Vec<String>, Option<PluginUpgradeInfo>)> {
    let plan = plan_plugin_install(source_dir)?;
    apply_plugin_install(&plan)?;
    Ok((plan.manifest, plan.summary, plan.upgrade_info))
}

/// Information about a plugin upgrade.
#[derive(Debug, Clone)]
pub struct PluginUpgradeInfo {
    pub old_version: String,
    pub new_version: String,
    pub permissions_changed: bool,
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

    // ===== Plugin Install WASM Validation Tests =====

    #[test]
    fn test_plan_plugin_install_rejects_invalid_wasm() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path();

        // Write valid plugin.toml
        let toml = r#"
id = "test-invalid"
name = "Test Invalid"
version = "1.0.0"
description = "Plugin with invalid wasm"
"#;
        std::fs::write(source.join("plugin.toml"), toml).unwrap();

        // Write invalid WASM bytes
        std::fs::write(source.join("plugin.wasm"), b"not-a-valid-wasm-module").unwrap();

        let result = plan_plugin_install(source);
        assert!(result.is_err(), "Should reject invalid WASM");
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("invalid WASM module"),
            "Error should mention invalid WASM — got: {}",
            err_msg
        );
    }

    #[test]
    fn test_plan_plugin_install_accepts_valid_wasm() {
        let dir = tempfile::TempDir::new().unwrap();
        let source = dir.path();

        // Write valid plugin.toml
        let toml = r#"
id = "test-valid"
name = "Test Valid"
version = "1.0.0"
description = "Plugin with valid wasm"
"#;
        std::fs::write(source.join("plugin.toml"), toml).unwrap();

        // Write valid minimal WASM module
        let valid_wasm = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap();
        std::fs::write(source.join("plugin.wasm"), &valid_wasm).unwrap();

        let result = plan_plugin_install(source);
        assert!(
            result.is_ok(),
            "Should accept valid WASM — got: {:?}",
            result.err()
        );
    }

    /// Helper: create a source directory with plugin.toml and plugin.wasm.
    fn make_plugin_source(toml_content: &str) -> tempfile::TempDir {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("plugin.toml"), toml_content).unwrap();
        let valid_wasm = wat::parse_str("(module (memory (export \"memory\") 1))").unwrap();
        std::fs::write(dir.path().join("plugin.wasm"), &valid_wasm).unwrap();
        dir
    }

    #[test]
    fn test_plan_empty_permissions_no_summary() {
        let dir = make_plugin_source(
            r#"
id = "no-perms"
name = "No Perms"
version = "1.0.0"
description = "Plugin without permissions"
"#,
        );
        let plan = plan_plugin_install(dir.path()).unwrap();
        assert!(
            plan.summary.is_empty(),
            "Empty-permission plugin should have no summary lines"
        );
        assert!(
            plan.upgrade_info.is_none(),
            "Fresh install should have no upgrade info"
        );
    }

    #[test]
    fn test_plan_nonempty_permissions_has_summary() {
        let dir = make_plugin_source(
            r#"
id = "with-perms"
name = "With Perms"
version = "1.0.0"
description = "Plugin with permissions"

[permissions]
filesystem = ["read:project"]
network = ["fetch:https://api.example.com"]
"#,
        );
        let plan = plan_plugin_install(dir.path()).unwrap();
        assert!(
            !plan.summary.is_empty(),
            "Non-empty-permission plugin should have summary lines"
        );
        assert!(plan.summary.iter().any(|l| l.contains("filesystem")));
        assert!(plan.summary.iter().any(|l| l.contains("network")));
    }

    #[test]
    fn test_plan_does_not_copy_files() {
        let dir = make_plugin_source(
            r#"
id = "plan-no-copy"
name = "Plan No Copy"
version = "1.0.0"
description = "Plan should not copy files"
"#,
        );
        let _plan = plan_plugin_install(dir.path()).unwrap();
        // Verify no plugin directory was created
        let dest = plugin_dir().join("plan-no-copy");
        assert!(
            !dest.exists(),
            "plan_plugin_install should NOT create plugin directory"
        );
    }

    #[test]
    fn test_denied_fresh_install_leaves_no_directory() {
        let dir = make_plugin_source(
            r#"
id = "deny-fresh"
name = "Deny Fresh"
version = "1.0.0"
description = "should not be installed"

[permissions]
filesystem = ["read:src"]
"#,
        );
        let _plan = plan_plugin_install(dir.path()).unwrap();
        // User denies — we do NOT call apply_plugin_install

        let dest = plugin_dir().join("deny-fresh");
        assert!(
            !dest.exists(),
            "Denied fresh install should leave no plugin directory"
        );
    }

    #[test]
    fn test_apply_installs_plugin_files() {
        let dir = make_plugin_source(
            r#"
id = "apply-test"
name = "Apply Test"
version = "1.0.0"
description = "Test apply phase"
"#,
        );
        let plan = plan_plugin_install(dir.path()).unwrap();
        apply_plugin_install(&plan).unwrap();

        let dest = plugin_dir().join("apply-test");
        assert!(
            dest.exists(),
            "apply_plugin_install should create plugin dir"
        );
        assert!(dest.join("plugin.toml").exists());
        assert!(dest.join("plugin.wasm").exists());

        // Cleanup
        let _ = std::fs::remove_dir_all(&dest);
    }

    #[test]
    fn test_upgrade_detection() {
        // Install v1
        let dir1 = make_plugin_source(
            r#"
id = "upgrade-test"
name = "Upgrade Test"
version = "1.0.0"
description = "v1"
"#,
        );
        let plan1 = plan_plugin_install(dir1.path()).unwrap();
        assert!(
            plan1.upgrade_info.is_none(),
            "Fresh install has no upgrade info"
        );
        apply_plugin_install(&plan1).unwrap();

        // Plan v2 — should detect upgrade
        let dir2 = make_plugin_source(
            r#"
id = "upgrade-test"
name = "Upgrade Test"
version = "2.0.0"
description = "v2"
"#,
        );
        let plan2 = plan_plugin_install(dir2.path()).unwrap();
        assert!(plan2.upgrade_info.is_some(), "Should detect upgrade");
        let info = plan2.upgrade_info.as_ref().unwrap();
        assert_eq!(info.old_version, "1.0.0");
        assert_eq!(info.new_version, "2.0.0");
        assert!(!info.permissions_changed);

        // Cleanup
        let _ = std::fs::remove_dir_all(plugin_dir().join("upgrade-test"));
    }

    #[test]
    fn test_upgrade_permission_change_detected() {
        // Install v1 with no permissions
        let dir1 = make_plugin_source(
            r#"
id = "perm-change"
name = "Perm Change"
version = "1.0.0"
description = "v1 no perms"
"#,
        );
        let plan1 = plan_plugin_install(dir1.path()).unwrap();
        apply_plugin_install(&plan1).unwrap();

        // Plan v2 with permissions — should detect permission change
        let dir2 = make_plugin_source(
            r#"
id = "perm-change"
name = "Perm Change"
version = "2.0.0"
description = "v2 with perms"

[permissions]
filesystem = ["read:src"]
"#,
        );
        let plan2 = plan_plugin_install(dir2.path()).unwrap();
        assert!(plan2.upgrade_info.is_some());
        let info = plan2.upgrade_info.as_ref().unwrap();
        assert!(info.permissions_changed, "Should detect permission change");

        // Cleanup
        let _ = std::fs::remove_dir_all(plugin_dir().join("perm-change"));
    }

    #[test]
    fn test_denied_upgrade_preserves_old_plugin() {
        // Install v1
        let dir1 = make_plugin_source(
            r#"
id = "deny-upgrade"
name = "Deny Upgrade"
version = "1.0.0"
description = "v1"
"#,
        );
        let plan1 = plan_plugin_install(dir1.path()).unwrap();
        apply_plugin_install(&plan1).unwrap();

        // Read original plugin.toml to verify later
        let dest = plugin_dir().join("deny-upgrade");
        let original_content = std::fs::read_to_string(dest.join("plugin.toml")).unwrap();
        assert!(original_content.contains("1.0.0"));

        // Plan v2 but DON'T apply (simulate user denial)
        let dir2 = make_plugin_source(
            r#"
id = "deny-upgrade"
name = "Deny Upgrade"
version = "2.0.0"
description = "v2 denied"
"#,
        );
        let _plan2 = plan_plugin_install(dir2.path()).unwrap();
        // User denies — we do NOT call apply_plugin_install

        // Verify old plugin is still intact
        let after_content = std::fs::read_to_string(dest.join("plugin.toml")).unwrap();
        assert!(
            after_content.contains("1.0.0"),
            "Denied upgrade should preserve old plugin.toml"
        );

        // Cleanup
        let _ = std::fs::remove_dir_all(&dest);
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
        let result = validate_permissions(&permissions);
        assert!(result.is_err(), "env wildcard should be rejected");
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("invalid env var name"));
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

    // ===== Permission Matcher Tests =====

    fn make_workspace() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("alius_test_{}", std::process::id()));
        std::fs::create_dir_all(dir.join("project/src")).unwrap();
        std::fs::create_dir_all(dir.join("output")).unwrap();
        std::fs::create_dir_all(dir.join("other")).unwrap();
        std::fs::write(dir.join("project/src/main.rs"), "fn main() {}").unwrap();
        std::fs::write(dir.join("output/report.md"), "report").unwrap();
        std::fs::write(dir.join("other/file.txt"), "other").unwrap();
        dir
    }

    fn cleanup_workspace(ws: &Path) {
        let _ = std::fs::remove_dir_all(ws);
    }

    // --- Default deny (no permissions) ---

    #[test]
    fn test_no_permissions_denies_all_filesystem() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions::default();
        let decision = perms.check_filesystem("read", Path::new("project/src/main.rs"), &ws);
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason: "no filesystem permissions declared".to_string()
            }
        );
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_no_permissions_denies_all_network() {
        let perms = ResolvedPluginPermissions::default();
        let decision = perms.check_network("https://api.example.com/data");
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason: "no network permissions declared".to_string()
            }
        );
    }

    #[test]
    fn test_no_permissions_denies_all_shell() {
        let perms = ResolvedPluginPermissions::default();
        let decision = perms.check_shell("ls -la");
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason: "no shell permissions declared".to_string()
            }
        );
    }

    #[test]
    fn test_no_permissions_denies_all_env() {
        let perms = ResolvedPluginPermissions::default();
        let decision = perms.check_env("HOME");
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason: "no env permissions declared".to_string()
            }
        );
    }

    // --- Filesystem matcher ---

    #[test]
    fn test_fs_read_allowed_within_prefix() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        assert!(perms
            .check_filesystem("read", Path::new("project/src/main.rs"), &ws)
            .is_allowed());
        assert!(perms
            .check_filesystem("read", Path::new("project/src"), &ws)
            .is_allowed());
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_write_allowed_within_prefix() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["write:output".to_string()],
            ..Default::default()
        };
        assert!(perms
            .check_filesystem("write", Path::new("output/report.md"), &ws)
            .is_allowed());
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_list_allowed_within_prefix() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["list:project".to_string()],
            ..Default::default()
        };
        assert!(perms
            .check_filesystem("list", Path::new("project/src"), &ws)
            .is_allowed());
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_denies_undeclared_path() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        let decision = perms.check_filesystem("read", Path::new("other/file.txt"), &ws);
        assert!(!decision.is_allowed());
        assert!(match &decision {
            PermissionDecision::Deny { reason } => reason.contains("not permitted"),
            _ => false,
        });
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_denies_path_traversal() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        let decision =
            perms.check_filesystem("read", Path::new("project/../../../etc/passwd"), &ws);
        assert!(!decision.is_allowed());
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_denies_absolute_path() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        let decision = perms.check_filesystem("read", Path::new("/etc/passwd"), &ws);
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason: "absolute paths are not allowed".to_string()
            }
        );
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_denies_symlink_escape() {
        let ws = make_workspace();
        // Create a symlink inside workspace pointing outside.
        let outside = std::env::temp_dir().join(format!("alius_outside_{}", std::process::id()));
        std::fs::create_dir_all(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "secret").unwrap();

        let link_path = ws.join("project/escape_link");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&outside, &link_path).unwrap();

        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        let decision =
            perms.check_filesystem("read", Path::new("project/escape_link/secret.txt"), &ws);
        assert!(!decision.is_allowed());

        cleanup_workspace(&outside);
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_denies_operation_mismatch() {
        let ws = make_workspace();
        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        // Read permission does not grant write.
        let decision = perms.check_filesystem("write", Path::new("project/src/main.rs"), &ws);
        assert!(!decision.is_allowed());
        cleanup_workspace(&ws);
    }

    #[test]
    fn test_fs_exact_prefix_match() {
        let ws = make_workspace();
        // "read:project" must NOT match "project-backup".
        std::fs::create_dir_all(ws.join("project-backup")).unwrap();
        std::fs::write(ws.join("project-backup/file.txt"), "data").unwrap();

        let perms = ResolvedPluginPermissions {
            filesystem: vec!["read:project".to_string()],
            ..Default::default()
        };
        let decision = perms.check_filesystem("read", Path::new("project-backup/file.txt"), &ws);
        assert!(!decision.is_allowed());
        cleanup_workspace(&ws);
    }

    // --- Network matcher ---

    #[test]
    fn test_net_fetch_allowed_prefix() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        assert!(perms
            .check_network("https://api.example.com/data")
            .is_allowed());
        assert!(perms
            .check_network("https://api.example.com/v2/items")
            .is_allowed());
    }

    #[test]
    fn test_net_denies_undeclared_domain() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        let decision = perms.check_network("https://evil.com/steal");
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_net_denies_similar_but_not_matching() {
        let perms = ResolvedPluginPermissions {
            network: vec!["fetch:https://api.example.com".to_string()],
            ..Default::default()
        };
        // Similar domain but not a prefix match.
        let decision = perms.check_network("https://api.example.com.evil.com/data");
        assert!(!decision.is_allowed());
    }

    // --- Shell matcher ---

    #[test]
    fn test_shell_readonly_allows_ls() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:readonly".to_string()],
            ..Default::default()
        };
        assert!(perms.check_shell("ls -la").is_allowed());
        assert!(perms.check_shell("ls").is_allowed());
    }

    #[test]
    fn test_shell_readonly_allows_cat() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:readonly".to_string()],
            ..Default::default()
        };
        assert!(perms.check_shell("cat file.txt").is_allowed());
    }

    #[test]
    fn test_shell_readonly_allows_git_log() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:readonly".to_string()],
            ..Default::default()
        };
        assert!(perms.check_shell("git log --oneline").is_allowed());
        assert!(perms.check_shell("git status").is_allowed());
    }

    #[test]
    fn test_shell_readonly_denies_rm() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:readonly".to_string()],
            ..Default::default()
        };
        let decision = perms.check_shell("rm -rf /");
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_shell_readonly_denies_sudo() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:readonly".to_string()],
            ..Default::default()
        };
        let decision = perms.check_shell("sudo apt install foo");
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_shell_literal_exact_match() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:git log".to_string()],
            ..Default::default()
        };
        assert!(perms.check_shell("git log").is_allowed());
    }

    #[test]
    fn test_shell_literal_denies_different() {
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:git log".to_string()],
            ..Default::default()
        };
        let decision = perms.check_shell("git status");
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_shell_dangerous_command_only_matches_exact_literal() {
        // If someone declares "exec:rm -rf /", it only matches that exact string.
        let perms = ResolvedPluginPermissions {
            shell: vec!["exec:rm -rf /".to_string()],
            ..Default::default()
        };
        assert!(perms.check_shell("rm -rf /").is_allowed());
        // A different rm command does NOT match.
        assert!(!perms.check_shell("rm -rf ~").is_allowed());
        assert!(!perms.check_shell("rm file.txt").is_allowed());
    }

    // --- Env matcher ---

    #[test]
    fn test_env_exact_match_allowed() {
        let perms = ResolvedPluginPermissions {
            env: vec!["read:HOME".to_string(), "read:CARGO_HOME".to_string()],
            ..Default::default()
        };
        assert!(perms.check_env("HOME").is_allowed());
        assert!(perms.check_env("CARGO_HOME").is_allowed());
    }

    #[test]
    fn test_env_empty_denied() {
        let perms = ResolvedPluginPermissions {
            env: vec!["read:HOME".to_string()],
            ..Default::default()
        };
        let decision = perms.check_env("");
        assert_eq!(
            decision,
            PermissionDecision::Deny {
                reason: "empty environment variable name".to_string()
            }
        );
    }

    #[test]
    fn test_env_prefix_denied() {
        let perms = ResolvedPluginPermissions {
            env: vec!["read:HOME".to_string()],
            ..Default::default()
        };
        // "HOME" should NOT match "HOME_DIR".
        let decision = perms.check_env("HOME_DIR");
        assert!(!decision.is_allowed());
    }

    #[test]
    fn test_env_undeclared_denied() {
        let perms = ResolvedPluginPermissions {
            env: vec!["read:HOME".to_string()],
            ..Default::default()
        };
        let decision = perms.check_env("PATH");
        assert!(!decision.is_allowed());
    }

    // --- ToolPackageManifest integration ---

    #[test]
    fn test_package_manifest_permissions_used_for_matching() {
        let ws = make_workspace();

        let toml_str = r#"
id = "test-matcher"
name = "Test Matcher"
version = "1.0.0"
description = "A plugin for testing the matcher"

[permissions]
filesystem = ["read:project", "write:output"]
network = ["fetch:https://api.example.com"]
shell = ["exec:readonly"]
env = ["read:HOME"]
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        let pkg: crate::package::ToolPackageManifest = manifest.into();

        // Use pkg.permissions directly for matching.
        assert!(pkg
            .permissions
            .check_filesystem("read", Path::new("project/src/main.rs"), &ws)
            .is_allowed());
        assert!(pkg
            .permissions
            .check_filesystem("write", Path::new("output/report.md"), &ws)
            .is_allowed());
        assert!(!pkg
            .permissions
            .check_filesystem("read", Path::new("other/file.txt"), &ws)
            .is_allowed());
        assert!(pkg
            .permissions
            .check_network("https://api.example.com/data")
            .is_allowed());
        assert!(!pkg
            .permissions
            .check_network("https://evil.com")
            .is_allowed());
        assert!(pkg.permissions.check_shell("ls -la").is_allowed());
        assert!(!pkg.permissions.check_shell("rm -rf /").is_allowed());
        assert!(pkg.permissions.check_env("HOME").is_allowed());
        assert!(!pkg.permissions.check_env("PATH").is_allowed());

        cleanup_workspace(&ws);
    }

    #[test]
    fn test_permission_decision_is_allowed() {
        assert!(PermissionDecision::allow_no_path().is_allowed());
        assert!(!PermissionDecision::Deny {
            reason: "test".to_string()
        }
        .is_allowed());
    }

    #[test]
    fn test_manifest_without_permissions_denies_all_runtime_checks() {
        let ws = make_workspace();
        // A manifest with no [permissions] section deserializes to None.
        let toml_str = r#"
id = "no-perms"
name = "No Perms"
version = "1.0.0"
description = "Plugin without permissions"
"#;
        let manifest: PluginManifest = toml::from_str(toml_str).unwrap();
        assert!(manifest.permissions.is_none());

        let resolved: ResolvedPluginPermissions = manifest.permissions.into();
        assert!(resolved.is_empty());

        // All runtime checks must deny.
        assert!(!resolved
            .check_filesystem("read", Path::new("project/src/main.rs"), &ws)
            .is_allowed());
        assert!(!resolved
            .check_filesystem("write", Path::new("output/report.md"), &ws)
            .is_allowed());
        assert!(!resolved
            .check_network("https://api.example.com/data")
            .is_allowed());
        assert!(!resolved.check_shell("ls -la").is_allowed());
        assert!(!resolved.check_env("HOME").is_allowed());
        cleanup_workspace(&ws);
    }
}
