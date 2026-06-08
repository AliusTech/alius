//! Patch diff generation.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A single file change.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileChange {
    /// File path relative to project root.
    pub path: String,
    /// Change type.
    pub change_type: String, // "create", "modify", "delete"
    /// New content (for create/modify).
    pub content: Option<String>,
}

/// A patch containing multiple file changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Patch {
    /// Patch ID.
    pub id: String,
    /// File changes.
    pub changes: Vec<FileChange>,
    /// Trace ID for correlation.
    pub trace_id: String,
    /// ISO 8601 timestamp.
    pub created_at: String,
}

/// Create a patch from file changes.
pub fn create_patch(changes: Vec<FileChange>, trace_id: &str) -> Patch {
    Patch {
        id: uuid::Uuid::new_v4().to_string(),
        changes,
        trace_id: trace_id.to_string(),
        created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    }
}

/// Apply a patch to the project root (write files).
pub fn apply_patch(patch: &Patch, project_root: &Path) -> Result<()> {
    for change in &patch.changes {
        let path = project_root.join(&change.path);
        match change.change_type.as_str() {
            "create" | "modify" => {
                if let Some(ref content) = change.content {
                    if let Some(parent) = path.parent() {
                        std::fs::create_dir_all(parent)?;
                    }
                    std::fs::write(&path, content)?;
                }
            }
            "delete" if path.exists() => {
                std::fs::remove_file(&path)?;
            }
            _ => {}
        }
    }
    Ok(())
}
