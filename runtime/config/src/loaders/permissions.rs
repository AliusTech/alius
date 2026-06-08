//! Loader for permissions.toml.

use crate::error::ConfigResult;
use crate::views::{
    EmbeddedSdkPermission, FilesystemPermission, MemoryPermission, NetworkPermission,
    PermissionConfig, ProjectDocumentPermission, RemoteA2APermission, ShellPermission,
    ShellScopeConfig,
};
use std::path::Path;

/// Load permissions.toml from the given path.
pub fn load_permissions(path: &Path) -> ConfigResult<PermissionConfig> {
    let raw: PermissionsToml = super::load_toml(path)?;
    Ok(raw.into())
}

/// permissions.toml raw structure.
#[derive(Debug, Clone, serde::Deserialize)]
struct PermissionsToml {
    filesystem: FilesystemPermission,
    shell: ShellPermission,
    network: NetworkPermission,
    memory: MemoryPermission,
    project_documents: ProjectDocumentPermission,
    remote_a2a: RemoteA2APermission,
    embedded_sdk: EmbeddedSdkPermission,
}

impl From<PermissionsToml> for PermissionConfig {
    fn from(raw: PermissionsToml) -> Self {
        Self {
            filesystem: raw.filesystem,
            shell: raw.shell,
            network: raw.network,
            memory: raw.memory,
            project_documents: raw.project_documents,
            remote_a2a: raw.remote_a2a,
            embedded_sdk: raw.embedded_sdk,
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
            embedded_sdk: EmbeddedSdkPermission {
                allow_shell: false,
                allow_local_tools: false,
                allow_lancedb: false,
                allow_local_embedding: false,
                allow_plugin_runtime: false,
            },
        }
    }
}
