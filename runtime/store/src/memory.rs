//! Memory store for persistent notes and context.
//!
//! Stores user-saved memories as JSON files:
//! - Global: ~/.alius/memory/global.json
//! - Project: ./.alius/memory/project.json

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::paths;

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub text: String,
    #[serde(default)]
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Memory store backed by a JSON file.
pub struct MemoryStore {
    path: PathBuf,
    entries: Vec<MemoryEntry>,
}

impl MemoryStore {
    /// Open or create a memory store at the given path.
    pub fn open(path: PathBuf) -> Result<Self> {
        let entries = if path.exists() {
            let content = std::fs::read_to_string(&path)?;
            serde_json::from_str(&content).unwrap_or_default()
        } else {
            Vec::new()
        };
        Ok(Self { path, entries })
    }

    /// Open the global memory store (~/.alius/memory/global.json).
    pub fn global() -> Result<Self> {
        let dir = paths::global_alius_dir().join("memory");
        std::fs::create_dir_all(&dir)?;
        Self::open(dir.join("global.json"))
    }

    /// Open the project memory store (./.alius/memory/project.json).
    pub fn project() -> Result<Self> {
        let dir = paths::project_memory_dir();
        let path = dir.join("project.json");
        let legacy_dot_alius_path = paths::project_alius_dir()
            .join("memory")
            .join("project.json");
        let legacy_path = PathBuf::from("alius").join("memory").join("project.json");

        if !path.exists() && legacy_dot_alius_path.exists() {
            return Self::open(legacy_dot_alius_path);
        }

        if !path.exists() && !legacy_dot_alius_path.exists() && legacy_path.exists() {
            return Self::open(legacy_path);
        }

        std::fs::create_dir_all(&dir)?;
        Self::open(path)
    }

    /// Save a new memory entry.
    pub fn save(&mut self, text: &str, tags: Vec<String>) -> Result<()> {
        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.to_string(),
            tags,
            created_at: Utc::now(),
        };
        self.entries.push(entry);
        self.flush()
    }

    /// List all memory entries.
    pub fn list(&self) -> &[MemoryEntry] {
        &self.entries
    }

    /// Get all memory text as a single string.
    pub fn all_text(&self) -> String {
        self.entries
            .iter()
            .map(|e| e.text.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Clear all entries.
    pub fn clear(&mut self) -> Result<()> {
        self.entries.clear();
        self.flush()
    }

    /// Write entries to disk.
    fn flush(&self) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(&self.entries)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn project_memory_uses_memory_directory() {
        let _lock = crate::test_util::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let mut store = MemoryStore::project().unwrap();
        store.save("project note", vec![]).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(project_dir
            .join(".alius")
            .join("memory")
            .join("project.json")
            .exists());
    }
}
