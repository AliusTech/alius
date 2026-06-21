//! Loader for permissions.toml.
//!
//! Supports two schema formats:
//! - **New format** (simplified): `[filesystem] mode`, `[[filesystem.roots]]`, `[network]`
//! - **Old format** (full): all sections explicitly defined
//!
//! The new format uses defaults for missing sections. The old format requires all fields.

use crate::error::ConfigResult;
use crate::views::{
    FilesystemPermission, MemoryPermission, NetworkPermission, PermissionConfig,
    ProjectDocumentPermission, RemoteA2APermission, ShellPermission, ShellScopeConfig,
};
use std::path::Path;

/// Load permissions.toml from the given path.
///
/// Tries the new simplified schema first, then falls back to the old full schema.
pub fn load_permissions(path: &Path) -> ConfigResult<PermissionConfig> {
    let content =
        std::fs::read_to_string(path).map_err(|e| crate::error::ConfigError::io(path, e))?;

    // Try new simplified schema first (must have roots)
    if let Ok(raw) = toml::from_str::<NewPermissionsToml>(&content) {
        // Validate that the new schema has roots defined
        if !raw.filesystem.roots.is_empty() {
            return Ok(raw.into());
        }
    }

    // Fall back to old full schema
    let raw: OldPermissionsToml =
        toml::from_str(&content).map_err(|e| crate::error::ConfigError::parse(path, e))?;
    Ok(raw.into())
}

/// Load permissions.toml from the given path (for testing).
///
/// Returns the raw content and which schema was used.
#[cfg(test)]
#[allow(dead_code)]
pub fn load_permissions_debug(path: &Path) -> ConfigResult<(PermissionConfig, &'static str)> {
    let content =
        std::fs::read_to_string(path).map_err(|e| crate::error::ConfigError::io(path, e))?;

    // Try new simplified schema first
    if let Ok(raw) = toml::from_str::<NewPermissionsToml>(&content) {
        return Ok((raw.into(), "new"));
    }

    // Fall back to old full schema
    let raw: OldPermissionsToml =
        toml::from_str(&content).map_err(|e| crate::error::ConfigError::parse(path, e))?;
    Ok((raw.into(), "old"))
}

/// New simplified permissions.toml structure.
///
/// Only requires `[filesystem]` and optionally `[network]`. Other sections use defaults.
#[derive(Debug, Clone, serde::Deserialize)]
struct NewPermissionsToml {
    filesystem: NewFilesystemConfig,
    #[serde(default)]
    network: Option<NewNetworkConfig>,
}

/// New filesystem configuration with mode and roots.
#[derive(Debug, Clone, serde::Deserialize)]
#[allow(dead_code)]
struct NewFilesystemConfig {
    #[serde(default = "default_fs_mode")]
    mode: String,
    #[serde(default)]
    roots: Vec<FilesystemRoot>,
}

/// Filesystem root entry.
#[derive(Debug, Clone, serde::Deserialize)]
struct FilesystemRoot {
    root: String,
    #[serde(default = "default_true")]
    read: bool,
    #[serde(default)]
    write: bool,
}

/// New network configuration.
#[derive(Debug, Clone, serde::Deserialize)]
struct NewNetworkConfig {
    #[serde(default)]
    enabled: bool,
}

fn default_fs_mode() -> String {
    "workspace".to_string()
}

fn default_true() -> bool {
    true
}

impl From<NewPermissionsToml> for PermissionConfig {
    fn from(raw: NewPermissionsToml) -> Self {
        let fs = raw.filesystem;
        let workspace_root = fs
            .roots
            .first()
            .map(|r| r.root.clone())
            .unwrap_or_else(|| ".".to_string());
        let allow_read = fs.roots.first().map(|r| r.read).unwrap_or(true);
        let allow_write = fs.roots.first().map(|r| r.write).unwrap_or(false);

        let network = raw.network.unwrap_or(NewNetworkConfig { enabled: false });

        Self {
            filesystem: FilesystemPermission {
                workspace_root,
                allow_read,
                allow_write,
                allow_delete: false,
                require_confirmation_for_write: true,
                require_confirmation_for_delete: true,
            },
            shell: ShellPermission::default(),
            network: NetworkPermission {
                enabled: network.enabled,
                require_confirmation: true,
                allowlist: vec![],
                denylist: vec![],
            },
            memory: MemoryPermission::default(),
            project_documents: ProjectDocumentPermission::default(),
            remote_a2a: RemoteA2APermission::default(),
        }
    }
}

/// Old full permissions.toml structure (requires all sections).
#[derive(Debug, Clone, serde::Deserialize)]
struct OldPermissionsToml {
    filesystem: FilesystemPermission,
    shell: ShellPermission,
    network: NetworkPermission,
    memory: MemoryPermission,
    project_documents: ProjectDocumentPermission,
    remote_a2a: RemoteA2APermission,
}

impl From<OldPermissionsToml> for PermissionConfig {
    fn from(raw: OldPermissionsToml) -> Self {
        Self {
            filesystem: raw.filesystem,
            shell: raw.shell,
            network: raw.network,
            memory: raw.memory,
            project_documents: raw.project_documents,
            remote_a2a: raw.remote_a2a,
        }
    }
}

impl Default for PermissionConfig {
    fn default() -> Self {
        Self {
            filesystem: FilesystemPermission {
                workspace_root: ".".to_string(),
                allow_read: true,
                allow_write: true,
                allow_delete: false,
                require_confirmation_for_write: true,
                require_confirmation_for_delete: true,
            },
            shell: ShellPermission {
                enabled: true,
                require_confirmation: true,
                workspace_scoped: true,
                deny_unknown_scope: true,
                require_confirmation_for_outside_workspace: true,
                allowlist: vec![],
                denylist: vec![
                    "rm -rf /".to_string(),
                    "rm -rf ~".to_string(),
                    "rm -rf .".to_string(),
                    "rm -rf *".to_string(),
                    "git reset --hard".to_string(),
                    "git checkout --".to_string(),
                    "sudo".to_string(),
                    "dd".to_string(),
                ],
                scope: ShellScopeConfig {
                    allow_read_outside_workspace: false,
                    allow_write_outside_workspace: false,
                    allow_delete_workspace_root: false,
                    allow_delete_outside_workspace: false,
                    follow_symlink_outside_workspace: false,
                    allow_redirection_outside_workspace: false,
                    allow_shell_eval_without_inspection: false,
                },
            },
            network: NetworkPermission {
                enabled: true,
                require_confirmation: true,
                allowlist: vec![],
                denylist: vec![],
            },
            memory: MemoryPermission {
                allow_read: true,
                allow_write: true,
                allow_semantic_index_rebuild: true,
            },
            project_documents: ProjectDocumentPermission {
                allow_update: true,
                require_history_entry: true,
                root: ".alius/workspace".to_string(),
            },
            remote_a2a: RemoteA2APermission {
                enabled: false,
                allow_filesystem: false,
                allow_shell: false,
                allow_network: false,
                allowed_tools: vec![],
            },
        }
    }
}
