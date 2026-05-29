//! Memory store for persistent notes and context.
//!
//! Stores user-saved memories as JSON files:
//! - Global: ~/.alius/memory/global.json
//! - Project: ./alius/memory/project.json

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// A single memory entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryEntry {
    pub id: String,
    pub text: String,
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
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        let dir = PathBuf::from(home).join(".alius").join("memory");
        std::fs::create_dir_all(&dir)?;
        Self::open(dir.join("global.json"))
    }

    /// Open the project memory store (./alius/memory/project.json).
    pub fn project() -> Result<Self> {
        let dir = PathBuf::from("alius").join("memory");
        std::fs::create_dir_all(&dir)?;
        Self::open(dir.join("project.json"))
    }

    /// Save a new memory entry.
    pub fn save(&mut self, text: &str) -> Result<()> {
        let entry = MemoryEntry {
            id: uuid::Uuid::new_v4().to_string(),
            text: text.to_string(),
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
        self.entries.iter().map(|e| e.text.as_str()).collect::<Vec<_>>().join("\n")
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
