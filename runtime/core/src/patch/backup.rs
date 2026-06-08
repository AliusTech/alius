//! Backup and rollback for patches.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Reference to a backup snapshot.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupRef {
    /// Backup ID.
    pub id: String,
    /// Patch ID this backup is for.
    pub patch_id: String,
    /// Backup directory path.
    pub path: String,
}

/// Create a backup of files that will be affected by a patch.
pub fn create_backup(
    patch_id: &str,
    files: &[String],
    project_root: &Path,
    snapshots_dir: &Path,
) -> Result<BackupRef> {
    let backup_id = uuid::Uuid::new_v4().to_string();
    let backup_dir = snapshots_dir.join(&backup_id);
    std::fs::create_dir_all(&backup_dir)?;

    for file in files {
        let src = project_root.join(file);
        if src.exists() {
            let dst = backup_dir.join(file);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dst)?;
        }
    }

    Ok(BackupRef {
        id: backup_id,
        patch_id: patch_id.to_string(),
        path: backup_dir.to_string_lossy().to_string(),
    })
}

/// Restore files from a backup.
pub fn restore_backup(backup: &BackupRef, project_root: &Path) -> Result<()> {
    let backup_dir = Path::new(&backup.path);
    if !backup_dir.exists() {
        anyhow::bail!("backup directory not found: {}", backup.path);
    }
    copy_dir_recursive(backup_dir, project_root)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::patch::diff::{create_patch, FileChange};
    use tempfile::TempDir;

    #[test]
    fn test_create_patch_generates_diff() {
        let changes = vec![FileChange {
            path: "src/main.rs".to_string(),
            change_type: "create".to_string(),
            content: Some("fn main() {}".to_string()),
        }];
        let patch = create_patch(changes, "trace-1");
        assert!(!patch.id.is_empty());
        assert_eq!(patch.trace_id, "trace-1");
        assert_eq!(patch.changes.len(), 1);
    }

    #[test]
    fn test_create_backup_stores_snapshot() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("file.txt"), "original").unwrap();

        let snapshots = dir.path().join("snapshots");
        let backup =
            create_backup("patch-1", &["file.txt".to_string()], dir.path(), &snapshots).unwrap();

        assert!(Path::new(&backup.path).join("file.txt").exists());
    }

    #[test]
    fn test_restore_backup_reverts_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("data.txt"), "original").unwrap();

        let snapshots = dir.path().join("snapshots");
        let backup =
            create_backup("patch-1", &["data.txt".to_string()], dir.path(), &snapshots).unwrap();

        // Modify file.
        std::fs::write(dir.path().join("data.txt"), "modified").unwrap();

        // Restore.
        restore_backup(&backup, dir.path()).unwrap();

        let content = std::fs::read_to_string(dir.path().join("data.txt")).unwrap();
        assert_eq!(content, "original");
    }

    #[test]
    fn test_patch_metadata_includes_trace_id() {
        let patch = create_patch(vec![], "trace-abc");
        assert_eq!(patch.trace_id, "trace-abc");
        assert!(!patch.created_at.is_empty());
    }
}
