//! Workspace archive — compare, confirm, and manage .archive/ directory.
//!
//! The archive holds the last-known-good state of workspace configuration.
//! `compare_archive` detects drift; `confirm_archive` syncs current state.

use anyhow::Result;
use std::path::Path;

/// Difference report between workspace and archive.
#[derive(Debug, Clone)]
pub struct ArchiveDiffReport {
    /// Files present in workspace but not in archive.
    pub added: Vec<String>,
    /// Files present in archive but not in workspace.
    pub removed: Vec<String>,
    /// Files with different content.
    pub modified: Vec<String>,
}

/// Compare the current workspace config with the .archive/ directory.
pub fn compare_archive(workspace_dir: &Path) -> Result<ArchiveDiffReport> {
    let archive_dir = workspace_dir.join(".archive");
    if !archive_dir.exists() {
        return Ok(ArchiveDiffReport {
            added: list_all_files(workspace_dir)?,
            removed: vec![],
            modified: vec![],
        });
    }

    let workspace_files = list_all_files(workspace_dir)?;
    let archive_files = list_all_files(&archive_dir)?;

    let added: Vec<String> = workspace_files
        .iter()
        .filter(|f| !archive_files.contains(f))
        .cloned()
        .collect();

    let removed: Vec<String> = archive_files
        .iter()
        .filter(|f| !workspace_files.contains(f))
        .cloned()
        .collect();

    let modified: Vec<String> = workspace_files
        .iter()
        .filter(|f| {
            archive_files.contains(f)
                && file_content(workspace_dir, f) != file_content(&archive_dir, f)
        })
        .cloned()
        .collect();

    Ok(ArchiveDiffReport {
        added,
        removed,
        modified,
    })
}

/// Confirm archive — copy current workspace state into .archive/.
pub fn confirm_archive(workspace_dir: &Path) -> Result<()> {
    let archive_dir = workspace_dir.join(".archive");
    std::fs::create_dir_all(&archive_dir)?;

    let files = list_all_files(workspace_dir)?;
    for file in &files {
        let src = workspace_dir.join(file);
        let dst = archive_dir.join(file);
        if let Some(parent) = dst.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(&src, &dst)?;
    }
    Ok(())
}

fn list_all_files(dir: &Path) -> Result<Vec<String>> {
    let mut files = Vec::new();
    if !dir.exists() {
        return Ok(files);
    }
    walk_dir(dir, dir, &mut files)?;
    Ok(files)
}

fn walk_dir(base: &Path, current: &Path, files: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            // Skip .archive itself to avoid recursion.
            if path.file_name().map(|n| n == ".archive").unwrap_or(false) {
                continue;
            }
            walk_dir(base, &path, files)?;
        } else {
            let relative = path.strip_prefix(base)?.to_string_lossy().to_string();
            files.push(relative);
        }
    }
    Ok(())
}

fn file_content(base: &Path, relative: &str) -> Option<String> {
    std::fs::read_to_string(base.join(relative)).ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_compare_detects_added_files() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("config.toml"), "test").unwrap();
        let report = compare_archive(dir.path()).unwrap();
        assert!(report.added.contains(&"config.toml".to_string()));
    }

    #[test]
    fn test_compare_detects_modified_files() {
        let dir = TempDir::new().unwrap();
        let archive_dir = dir.path().join(".archive");
        std::fs::create_dir_all(&archive_dir).unwrap();

        std::fs::write(dir.path().join("config.toml"), "new content").unwrap();
        std::fs::write(archive_dir.join("config.toml"), "old content").unwrap();

        let report = compare_archive(dir.path()).unwrap();
        assert!(report.modified.contains(&"config.toml".to_string()));
    }

    #[test]
    fn test_confirm_syncs_archive() {
        let dir = TempDir::new().unwrap();
        std::fs::write(dir.path().join("data.txt"), "hello").unwrap();
        confirm_archive(dir.path()).unwrap();

        let archive_content =
            std::fs::read_to_string(dir.path().join(".archive/data.txt")).unwrap();
        assert_eq!(archive_content, "hello");
    }
}
