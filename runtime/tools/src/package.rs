//! Rust WASM tool package resolution.
//!
//! A tool package is an installed Rust WASM module plus its manifest. The
//! resolver builds registries from installed packages; concrete tool business
//! logic stays inside those Rust WASM modules.

use std::path::{Path, PathBuf};

use anyhow::Result;

use crate::wasm_host::{self, PluginManifest};
use crate::{ToolRegistry, WasmPluginTool};

/// Tool package manifest used by the runtime loader.
#[derive(Debug, Clone)]
pub struct ToolPackageManifest {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: String,
    pub author: Option<String>,
    /// Resolved permissions from the plugin manifest.
    pub permissions: wasm_host::ResolvedPluginPermissions,
}

impl From<PluginManifest> for ToolPackageManifest {
    fn from(manifest: PluginManifest) -> Self {
        Self {
            id: manifest.id,
            name: manifest.name,
            version: manifest.version,
            description: manifest.description,
            author: manifest.author,
            permissions: manifest.permissions.into(),
        }
    }
}

/// Installed Rust WASM tool package.
#[derive(Debug, Clone)]
pub struct ToolPackage {
    pub manifest: ToolPackageManifest,
    pub wasm_path: PathBuf,
}

/// Host capability classes that Rust WASM tools may request through the runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolHostCapability {
    Filesystem,
    Shell,
    Network,
    Git,
    Memory,
}

/// Runtime host boundary for Rust WASM tool execution.
#[derive(Debug, Clone)]
pub struct ToolRuntimeHost {
    workspace_root: PathBuf,
}

impl ToolRuntimeHost {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

/// Resolves installed Rust WASM tool packages and builds active registries.
#[derive(Debug, Clone)]
pub struct ToolPackageResolver {
    host: ToolRuntimeHost,
}

impl ToolPackageResolver {
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            host: ToolRuntimeHost::new(workspace_root),
        }
    }

    pub fn host(&self) -> &ToolRuntimeHost {
        &self.host
    }

    pub fn list_installed_packages(&self) -> Result<Vec<ToolPackage>> {
        wasm_host::list_plugins().map(|plugins| {
            plugins
                .into_iter()
                .map(|plugin| ToolPackage {
                    manifest: plugin.manifest.into(),
                    wasm_path: plugin.wasm_path,
                })
                .collect()
        })
    }

    pub fn build_registry(&self) -> Result<ToolRegistry> {
        let registry = ToolRegistry::new();
        // Always register native tools first — they own the built-in names
        crate::native::register_native_tools(&registry);
        for package in self.list_installed_packages()? {
            let wasm_bytes = std::fs::read(&package.wasm_path)?;
            let tools = WasmPluginTool::from_wasm_bytes(&wasm_bytes)?;
            for tool in tools {
                if let Err(conflict) = registry.register(tool) {
                    // WASM tool name conflicts with an already-registered tool
                    // (typically a native built-in). Log and skip.
                    eprintln!("[warn] {conflict} — skipping WASM tool");
                }
            }
        }
        Ok(registry)
    }

    pub fn build_registry_lossy(&self) -> ToolRegistry {
        match self.build_registry() {
            Ok(registry) => registry,
            Err(err) => {
                eprintln!("[warn] Failed to load Rust WASM tools: {err}");
                // Still register native tools even if WASM loading fails
                let registry = ToolRegistry::new();
                crate::native::register_native_tools(&registry);
                registry
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wasm_host::{PluginManifest, PluginPermissions};

    #[test]
    fn test_manifest_with_permissions_converts() {
        let manifest = PluginManifest {
            id: "test-plugin".to_string(),
            name: "Test Plugin".to_string(),
            version: "1.0.0".to_string(),
            description: "A test plugin".to_string(),
            author: Some("tester".to_string()),
            permissions: Some(PluginPermissions {
                filesystem: vec!["read:project".to_string(), "write:output".to_string()],
                network: vec!["fetch:https://api.example.com".to_string()],
                shell: vec!["exec:readonly".to_string()],
                env: vec!["read:HOME".to_string(), "read:PATH".to_string()],
            }),
        };
        let pkg: ToolPackageManifest = manifest.into();
        assert_eq!(pkg.id, "test-plugin");
        assert_eq!(pkg.name, "Test Plugin");
        assert_eq!(pkg.version, "1.0.0");
        assert_eq!(pkg.author, Some("tester".to_string()));
        assert_eq!(
            pkg.permissions.filesystem,
            vec!["read:project", "write:output"]
        );
        assert_eq!(
            pkg.permissions.network,
            vec!["fetch:https://api.example.com"]
        );
        assert_eq!(pkg.permissions.shell, vec!["exec:readonly"]);
        assert_eq!(pkg.permissions.env, vec!["read:HOME", "read:PATH"]);
    }

    #[test]
    fn test_manifest_without_permissions_converts_empty() {
        let manifest = PluginManifest {
            id: "old-plugin".to_string(),
            name: "Old Plugin".to_string(),
            version: "0.1.0".to_string(),
            description: "An old plugin".to_string(),
            author: None,
            permissions: None,
        };
        let pkg: ToolPackageManifest = manifest.into();
        assert_eq!(pkg.id, "old-plugin");
        assert!(pkg.author.is_none());
        assert!(pkg.permissions.is_empty());
    }
}
