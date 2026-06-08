//! SOUL source management — add, update, list, and install SOUL profiles.
//!
//! SOUL sources are git repositories containing SOUL profile definitions.
//! Installed SOULs provide role definitions, capability bundles, and workspace templates.

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// An entry in the source index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceEntry {
    /// Source name.
    pub name: String,
    /// Git URL.
    pub url: String,
}

/// A SOUL available in a source's index.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulIndexEntry {
    /// SOUL name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Source name.
    pub source: String,
}

/// SOUL source manager.
pub struct SoulSourceManager {
    /// Base directory for source repos (~/.alius/souls/).
    base_dir: std::path::PathBuf,
}

impl SoulSourceManager {
    /// Create a new SoulSourceManager.
    pub fn new(base_dir: &Path) -> Self {
        Self {
            base_dir: base_dir.to_path_buf(),
        }
    }

    /// Add a SOUL source (clones the git repo).
    pub fn add_source(&self, name: &str, url: &str) -> Result<()> {
        let repo_dir = self.base_dir.join("repos").join(name);
        if repo_dir.exists() {
            bail!("source '{}' already exists", name);
        }
        std::fs::create_dir_all(repo_dir.parent().unwrap())?;

        let status = std::process::Command::new("git")
            .args(["clone", "--depth", "1", url, &repo_dir.to_string_lossy()])
            .output()?;

        if !status.status.success() {
            let stderr = String::from_utf8_lossy(&status.stderr);
            // Clean up failed clone.
            let _ = std::fs::remove_dir_all(&repo_dir);
            bail!("failed to clone '{}': {}", url, stderr);
        }

        // Update index.
        self.update_index_entry(name, url)?;
        Ok(())
    }

    /// Update all sources (git pull).
    pub fn update_sources(&self) -> Result<Vec<String>> {
        let repos_dir = self.base_dir.join("repos");
        if !repos_dir.exists() {
            return Ok(vec![]);
        }

        let mut updated = Vec::new();
        for entry in std::fs::read_dir(&repos_dir)? {
            let entry = entry?;
            if entry.file_type()?.is_dir() {
                let name = entry.file_name().to_string_lossy().to_string();
                let status = std::process::Command::new("git")
                    .args(["pull", "--ff-only"])
                    .current_dir(entry.path())
                    .output();

                if let Ok(out) = status {
                    if out.status.success() {
                        updated.push(name);
                    }
                }
            }
        }
        Ok(updated)
    }

    /// List configured sources.
    pub fn list_sources(&self) -> Result<Vec<SourceEntry>> {
        let index_path = self.base_dir.join("index.toml");
        if !index_path.exists() {
            return Ok(vec![]);
        }
        let content = std::fs::read_to_string(&index_path)?;
        let table: toml::Value = match toml::from_str(&content) {
            Ok(t) => t,
            Err(_) => return Ok(vec![]),
        };
        let mut sources = Vec::new();
        if let Some(arr) = table.get("sources").and_then(|v| v.as_array()) {
            for item in arr {
                if let Some(t) = item.as_table() {
                    sources.push(SourceEntry {
                        name: t
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                        url: t
                            .get("url")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string(),
                    });
                }
            }
        }
        Ok(sources)
    }

    /// Remove a source by name.
    pub fn remove_source(&self, name: &str) -> Result<()> {
        let repo_dir = self.base_dir.join("repos").join(name);
        if repo_dir.exists() {
            std::fs::remove_dir_all(&repo_dir)?;
        }
        self.remove_index_entry(name)?;
        Ok(())
    }

    /// List available SOULs from all sources.
    pub fn list_available_souls(&self) -> Result<Vec<SoulIndexEntry>> {
        let repos_dir = self.base_dir.join("repos");
        if !repos_dir.exists() {
            return Ok(vec![]);
        }

        let mut souls = Vec::new();
        for entry in std::fs::read_dir(&repos_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let source_name = entry.file_name().to_string_lossy().to_string();
            let souls_dir = entry.path().join("souls");
            if !souls_dir.exists() {
                continue;
            }
            for soul_entry in std::fs::read_dir(&souls_dir)? {
                let soul_entry = soul_entry?;
                if soul_entry.file_type()?.is_dir() {
                    let name = soul_entry.file_name().to_string_lossy().to_string();
                    let desc_path = soul_entry.path().join("description.txt");
                    let description = std::fs::read_to_string(&desc_path)
                        .unwrap_or_else(|_| "No description".to_string());
                    souls.push(SoulIndexEntry {
                        name,
                        description: description.trim().to_string(),
                        source: source_name.clone(),
                    });
                }
            }
        }
        Ok(souls)
    }

    fn update_index_entry(&self, name: &str, url: &str) -> Result<()> {
        let mut sources = self.list_sources().unwrap_or_default();
        if !sources.iter().any(|s| s.name == name) {
            sources.push(SourceEntry {
                name: name.to_string(),
                url: url.to_string(),
            });
        }
        let index_path = self.base_dir.join("index.toml");
        std::fs::create_dir_all(&self.base_dir)?;
        // TOML requires a table at top level.
        let mut table = toml::value::Table::new();
        let mut arr = toml::value::Array::new();
        for s in &sources {
            let mut entry = toml::value::Table::new();
            entry.insert("name".to_string(), toml::Value::String(s.name.clone()));
            entry.insert("url".to_string(), toml::Value::String(s.url.clone()));
            arr.push(toml::Value::Table(entry));
        }
        table.insert("sources".to_string(), toml::Value::Array(arr));
        std::fs::write(&index_path, toml::to_string_pretty(&table)?)?;
        Ok(())
    }

    fn remove_index_entry(&self, name: &str) -> Result<()> {
        let index_path = self.base_dir.join("index.toml");
        if !index_path.exists() {
            return Ok(());
        }
        let mut sources = self.list_sources().unwrap_or_default();
        sources.retain(|s| s.name != name);
        let mut table = toml::value::Table::new();
        let mut arr = toml::value::Array::new();
        for s in &sources {
            let mut entry = toml::value::Table::new();
            entry.insert("name".to_string(), toml::Value::String(s.name.clone()));
            entry.insert("url".to_string(), toml::Value::String(s.url.clone()));
            arr.push(toml::Value::Table(entry));
        }
        table.insert("sources".to_string(), toml::Value::Array(arr));
        std::fs::write(&index_path, toml::to_string_pretty(&table)?)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_list_sources_empty() {
        let dir = TempDir::new().unwrap();
        let mgr = SoulSourceManager::new(dir.path());
        let sources = mgr.list_sources().unwrap();
        assert!(sources.is_empty());
    }

    #[test]
    fn test_add_source_invalid_url() {
        let dir = TempDir::new().unwrap();
        let mgr = SoulSourceManager::new(dir.path());
        let result = mgr.add_source("test", "https://nonexistent.invalid/repo.git");
        assert!(result.is_err());
    }

    #[test]
    fn test_list_available_souls_empty() {
        let dir = TempDir::new().unwrap();
        let mgr = SoulSourceManager::new(dir.path());
        let souls = mgr.list_available_souls().unwrap();
        assert!(souls.is_empty());
    }

    #[test]
    fn test_remove_nonexistent_source_ok() {
        let dir = TempDir::new().unwrap();
        let mgr = SoulSourceManager::new(dir.path());
        // Should not error — removing nonexistent is ok.
        assert!(mgr.remove_source("nonexistent").is_ok());
    }

    #[test]
    fn test_update_sources_empty() {
        let dir = TempDir::new().unwrap();
        let mgr = SoulSourceManager::new(dir.path());
        let updated = mgr.update_sources().unwrap();
        assert!(updated.is_empty());
    }

    #[test]
    fn test_index_persistence() {
        let dir = TempDir::new().unwrap();
        let mgr = SoulSourceManager::new(dir.path());

        // Write an index entry via the manager.
        mgr.update_index_entry("official", "https://example.com/souls")
            .unwrap();

        let loaded = mgr.list_sources().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].name, "official");
    }
}
