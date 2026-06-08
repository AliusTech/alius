//! Capability Bundle management — resolve, install, remove, and verify.
//!
//! Capabilities are groups of tools, permissions, and configurations that
//! are derived from SOUL profiles or user configuration.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A resolved capability with its source and contents.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedCapability {
    /// Capability name.
    pub name: String,
    /// Who owns this capability: "soul" or "user".
    pub owner: String,
    /// Version hash for integrity checks.
    pub version_hash: String,
    /// Whether the capability is enabled.
    pub enabled: bool,
}

/// Installation plan computed from SOUL + user config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityPlan {
    /// Capabilities to install.
    pub to_install: Vec<ResolvedCapability>,
    /// Capabilities to remove.
    pub to_remove: Vec<String>,
}

/// Report from an install operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallReport {
    pub installed: Vec<String>,
    pub removed: Vec<String>,
}

/// Verification report.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifyReport {
    pub valid: bool,
    pub mismatches: Vec<String>,
}

/// Capability Bundle Manager.
pub struct CapabilityManager {
    /// Project .alius directory.
    project_dir: std::path::PathBuf,
}

impl CapabilityManager {
    /// Create a new CapabilityManager.
    pub fn new(project_dir: &Path) -> Self {
        Self {
            project_dir: project_dir.to_path_buf(),
        }
    }

    /// Resolve capabilities from SOUL config and user overrides.
    pub fn resolve_capabilities(
        soul_caps: &[String],
        user_enabled: &[String],
        user_disabled: &[String],
    ) -> CapabilityPlan {
        let mut caps: HashMap<String, ResolvedCapability> = HashMap::new();

        // Add SOUL capabilities.
        for name in soul_caps {
            caps.insert(
                name.clone(),
                ResolvedCapability {
                    name: name.clone(),
                    owner: "soul".to_string(),
                    version_hash: simple_hash(name),
                    enabled: true,
                },
            );
        }

        // Add user capabilities.
        for name in user_enabled {
            caps.insert(
                name.clone(),
                ResolvedCapability {
                    name: name.clone(),
                    owner: "user".to_string(),
                    version_hash: simple_hash(name),
                    enabled: true,
                },
            );
        }

        // Disable specified.
        for name in user_disabled {
            if let Some(cap) = caps.get_mut(name) {
                cap.enabled = false;
            }
        }

        CapabilityPlan {
            to_install: caps.into_values().collect(),
            to_remove: vec![],
        }
    }

    /// Install capabilities from a plan.
    pub fn install_capabilities(&self, plan: &CapabilityPlan) -> Result<InstallReport> {
        let install_dir = self.project_dir.join("capability").join("installed");
        std::fs::create_dir_all(&install_dir)?;

        let mut installed = Vec::new();
        for cap in &plan.to_install {
            let cap_dir = install_dir.join(&cap.name);
            std::fs::create_dir_all(&cap_dir)?;

            // Write metadata.
            let meta = serde_json::json!({
                "name": cap.name,
                "owner": cap.owner,
                "version_hash": cap.version_hash,
                "enabled": cap.enabled,
            });
            std::fs::write(
                cap_dir.join("meta.json"),
                serde_json::to_string_pretty(&meta)?,
            )?;
            installed.push(cap.name.clone());
        }

        // Update lock file.
        let lock = LockFile {
            capabilities: plan.to_install.clone(),
        };
        let lock_path = self.project_dir.join("capability").join("lock.toml");
        std::fs::write(&lock_path, toml::to_string_pretty(&lock)?)?;

        Ok(InstallReport {
            installed,
            removed: plan.to_remove.clone(),
        })
    }

    /// Remove a capability.
    pub fn remove_capability(&self, name: &str) -> Result<()> {
        let cap_dir = self
            .project_dir
            .join("capability")
            .join("installed")
            .join(name);
        if cap_dir.exists() {
            std::fs::remove_dir_all(&cap_dir)?;
        }
        Ok(())
    }

    /// Disable a capability (mark in config).
    pub fn disable_capability(&self, name: &str) -> Result<()> {
        let meta_path = self
            .project_dir
            .join("capability")
            .join("installed")
            .join(name)
            .join("meta.json");
        if meta_path.exists() {
            let mut meta: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
            meta["enabled"] = serde_json::Value::Bool(false);
            std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        }
        Ok(())
    }

    /// Enable a capability.
    pub fn enable_capability(&self, name: &str) -> Result<()> {
        let meta_path = self
            .project_dir
            .join("capability")
            .join("installed")
            .join(name)
            .join("meta.json");
        if meta_path.exists() {
            let mut meta: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
            meta["enabled"] = serde_json::Value::Bool(true);
            std::fs::write(&meta_path, serde_json::to_string_pretty(&meta)?)?;
        }
        Ok(())
    }

    /// Verify lock file consistency with installed capabilities.
    pub fn verify_lock(&self) -> Result<VerifyReport> {
        let lock_path = self.project_dir.join("capability").join("lock.toml");
        if !lock_path.exists() {
            return Ok(VerifyReport {
                valid: true,
                mismatches: vec![],
            });
        }

        let lock: LockFile = toml::from_str(&std::fs::read_to_string(&lock_path)?)?;
        let mut mismatches = Vec::new();

        for cap in &lock.capabilities {
            let meta_path = self
                .project_dir
                .join("capability")
                .join("installed")
                .join(&cap.name)
                .join("meta.json");
            if !meta_path.exists() {
                mismatches.push(format!("{}: installed directory missing", cap.name));
                continue;
            }
            let meta: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
            let actual_hash = meta["version_hash"].as_str().unwrap_or("");
            if actual_hash != cap.version_hash {
                mismatches.push(format!(
                    "{}: hash mismatch (lock={}, actual={})",
                    cap.name, cap.version_hash, actual_hash
                ));
            }
        }

        Ok(VerifyReport {
            valid: mismatches.is_empty(),
            mismatches,
        })
    }

    /// List installed capabilities.
    pub fn list_capabilities(&self) -> Result<Vec<ResolvedCapability>> {
        let install_dir = self.project_dir.join("capability").join("installed");
        if !install_dir.exists() {
            return Ok(vec![]);
        }

        let mut caps = Vec::new();
        for entry in std::fs::read_dir(&install_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name().to_string_lossy().to_string();
            let meta_path = entry.path().join("meta.json");
            if meta_path.exists() {
                let meta: serde_json::Value =
                    serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
                caps.push(ResolvedCapability {
                    name,
                    owner: meta["owner"].as_str().unwrap_or("unknown").to_string(),
                    version_hash: meta["version_hash"].as_str().unwrap_or("").to_string(),
                    enabled: meta["enabled"].as_bool().unwrap_or(true),
                });
            }
        }
        Ok(caps)
    }
}

fn simple_hash(input: &str) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    input.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct LockFile {
    capabilities: Vec<ResolvedCapability>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_mgr() -> (TempDir, CapabilityManager) {
        let dir = TempDir::new().unwrap();
        let mgr = CapabilityManager::new(dir.path());
        (dir, mgr)
    }

    #[test]
    fn test_resolve_from_soul_config() {
        let plan = CapabilityManager::resolve_capabilities(
            &["code_review".to_string(), "testing".to_string()],
            &[],
            &[],
        );
        assert_eq!(plan.to_install.len(), 2);
        assert_eq!(plan.to_install[0].owner, "soul");
    }

    #[test]
    fn test_install_creates_directories() {
        let (_dir, mgr) = make_mgr();
        let plan = CapabilityManager::resolve_capabilities(&["cap_a".to_string()], &[], &[]);
        let report = mgr.install_capabilities(&plan).unwrap();
        assert!(report.installed.contains(&"cap_a".to_string()));
    }

    #[test]
    fn test_lock_toml_consistent_with_installed() {
        let (_dir, mgr) = make_mgr();
        let plan = CapabilityManager::resolve_capabilities(&["cap_b".to_string()], &[], &[]);
        mgr.install_capabilities(&plan).unwrap();

        let report = mgr.verify_lock().unwrap();
        assert!(report.valid);
    }

    #[test]
    fn test_remove_capability_cleans_up() {
        let (_dir, mgr) = make_mgr();
        let plan = CapabilityManager::resolve_capabilities(&["cap_c".to_string()], &[], &[]);
        mgr.install_capabilities(&plan).unwrap();
        mgr.remove_capability("cap_c").unwrap();

        let caps = mgr.list_capabilities().unwrap();
        assert!(caps.is_empty());
    }

    #[test]
    fn test_disable_marks_in_config() {
        let (_dir, mgr) = make_mgr();
        let plan = CapabilityManager::resolve_capabilities(&["cap_d".to_string()], &[], &[]);
        mgr.install_capabilities(&plan).unwrap();
        mgr.disable_capability("cap_d").unwrap();

        let caps = mgr.list_capabilities().unwrap();
        assert!(!caps[0].enabled);
    }

    #[test]
    fn test_enable_restores() {
        let (_dir, mgr) = make_mgr();
        let plan = CapabilityManager::resolve_capabilities(&["cap_e".to_string()], &[], &[]);
        mgr.install_capabilities(&plan).unwrap();
        mgr.disable_capability("cap_e").unwrap();
        mgr.enable_capability("cap_e").unwrap();

        let caps = mgr.list_capabilities().unwrap();
        assert!(caps[0].enabled);
    }

    #[test]
    fn test_verify_detects_tampering() {
        let (dir, mgr) = make_mgr();
        let plan = CapabilityManager::resolve_capabilities(&["cap_f".to_string()], &[], &[]);
        mgr.install_capabilities(&plan).unwrap();

        // Tamper with the meta.json.
        let meta_path = dir.path().join("capability/installed/cap_f/meta.json");
        std::fs::write(
            &meta_path,
            r#"{"name":"cap_f","owner":"soul","version_hash":"tampered","enabled":true}"#,
        )
        .unwrap();

        let report = mgr.verify_lock().unwrap();
        assert!(!report.valid);
        assert!(!report.mismatches.is_empty());
    }
}
