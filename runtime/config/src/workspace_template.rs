//! Workspace template migration — parse SOUL workspace.dirs and apply to project.
//!
//! Templates are additive only: existing directories are never deleted.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// A workspace template parsed from SOUL configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceTemplate {
    /// Required directories.
    pub dirs: Vec<String>,
}

/// Report from template application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateApplyReport {
    /// Directories that were created.
    pub created: Vec<String>,
    /// Directories that already existed.
    pub existed: Vec<String>,
}

/// Parse a workspace template from SOUL config directories list.
pub fn parse_workspace_template(dirs: &[String]) -> WorkspaceTemplate {
    WorkspaceTemplate {
        dirs: dirs.to_vec(),
    }
}

/// Check which template directories are missing from the project.
pub fn check_missing_directories(project_root: &Path, template: &WorkspaceTemplate) -> Vec<String> {
    template
        .dirs
        .iter()
        .filter(|d| !project_root.join(d).exists())
        .cloned()
        .collect()
}

/// Apply a workspace template — create missing directories only.
pub fn apply_template(
    project_root: &Path,
    template: &WorkspaceTemplate,
) -> Result<TemplateApplyReport> {
    let mut created = Vec::new();
    let mut existed = Vec::new();

    for dir in &template.dirs {
        let path = project_root.join(dir);
        if path.exists() {
            existed.push(dir.clone());
        } else {
            std::fs::create_dir_all(&path)?;
            created.push(dir.clone());
        }
    }

    // Record application history.
    let history_path = project_root
        .join(".alius")
        .join("workspace")
        .join(".workspace.toml");
    if let Some(parent) = history_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let history = serde_json::json!({
        "last_applied": format!("{:?}", std::time::SystemTime::now()),
        "created": created,
        "existed": existed,
    });
    std::fs::write(
        &history_path,
        toml::to_string_pretty(&history).unwrap_or_default(),
    )?;

    Ok(TemplateApplyReport { created, existed })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_parse_template_from_dirs() {
        let dirs = vec![
            "src".to_string(),
            "docs".to_string(),
            ".alius/memory".to_string(),
        ];
        let template = parse_workspace_template(&dirs);
        assert_eq!(template.dirs.len(), 3);
    }

    #[test]
    fn test_apply_creates_missing_dirs() {
        let dir = TempDir::new().unwrap();
        let template = parse_workspace_template(&["src".to_string(), "docs".to_string()]);
        let report = apply_template(dir.path(), &template).unwrap();
        assert_eq!(report.created.len(), 2);
        assert!(dir.path().join("src").exists());
        assert!(dir.path().join("docs").exists());
    }

    #[test]
    fn test_apply_does_not_delete_existing() {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/main.rs"), "fn main() {}").unwrap();

        let template = parse_workspace_template(&["src".to_string(), "new_dir".to_string()]);
        let report = apply_template(dir.path(), &template).unwrap();

        assert_eq!(report.existed.len(), 1);
        assert_eq!(report.created.len(), 1);
        // Existing file preserved.
        assert!(dir.path().join("src/main.rs").exists());
    }

    #[test]
    fn test_workspace_toml_records_history() {
        let dir = TempDir::new().unwrap();
        let template = parse_workspace_template(&["logs".to_string()]);
        apply_template(dir.path(), &template).unwrap();

        let history_path = dir.path().join(".alius/workspace/.workspace.toml");
        assert!(history_path.exists());
    }
}
