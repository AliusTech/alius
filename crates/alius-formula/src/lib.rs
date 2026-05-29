//! Alius Formula — Repository management for Soul/Plugin formulas.
//!
//! Manages the alius-core formula repository:
//! - Clone/fetch from remote git repo
//! - Parse TOML formula definitions
//! - List and query available formulas

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Default remote URL for the official alius-core repository.
pub const OFFICIAL_REMOTE: &str = "git@github.com:AliusTech/alius-core.git";

/// A parsed formula definition from TOML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FormulaDef {
    pub id: String,
    pub name: String,
    pub version: String,
    #[serde(rename = "type")]
    pub formula_type: String,
    pub description: String,
    #[serde(default)]
    pub license: Option<String>,
    #[serde(default)]
    pub model: Option<ModelPrefs>,
}

/// Model preferences in a formula.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelPrefs {
    #[serde(default)]
    pub preferred_provider: Option<String>,
    #[serde(default)]
    pub preferred_main_model: Option<String>,
    #[serde(default)]
    pub preferred_review_model: Option<String>,
}

/// Get the local path for the official formula repository.
pub fn official_repo_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".alius").join("core").join("official")
}

/// Clone or update the official alius-core repository.
pub fn update_repo() -> Result<PathBuf> {
    let path = official_repo_path();
    if path.join(".git").exists() {
        // Fetch latest
        let status = std::process::Command::new("git")
            .args(["-C", path.to_str().unwrap(), "fetch", "--all"])
            .status()?;
        if !status.success() {
            anyhow::bail!("git fetch failed");
        }
        let status = std::process::Command::new("git")
            .args(["-C", path.to_str().unwrap(), "reset", "--hard", "origin/main"])
            .status()?;
        if !status.success() {
            anyhow::bail!("git reset failed");
        }
    } else {
        // Clone
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let status = std::process::Command::new("git")
            .args(["clone", OFFICIAL_REMOTE, path.to_str().unwrap()])
            .status()?;
        if !status.success() {
            anyhow::bail!("git clone failed");
        }
    }
    Ok(path)
}

/// List all formulas in a directory (e.g. Formula/souls/).
pub fn list_formulas(repo_path: &Path, sub_dir: &str) -> Result<Vec<FormulaDef>> {
    let dir = repo_path.join("Formula").join(sub_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut formulas = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            let content = std::fs::read_to_string(&path)?;
            match toml::from_str::<FormulaDef>(&content) {
                Ok(f) => formulas.push(f),
                Err(e) => eprintln!("Warning: failed to parse {}: {}", path.display(), e),
            }
        }
    }

    formulas.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(formulas)
}

/// Find a formula by ID in a directory.
pub fn find_formula(repo_path: &Path, sub_dir: &str, id: &str) -> Result<Option<FormulaDef>> {
    let dir = repo_path.join("Formula").join(sub_dir);
    if !dir.exists() {
        return Ok(None);
    }

    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("toml") {
            let content = std::fs::read_to_string(&path)?;
            if let Ok(f) = toml::from_str::<FormulaDef>(&content) {
                if f.id == id {
                    return Ok(Some(f));
                }
            }
        }
    }

    Ok(None)
}

/// Get the global soul installation directory (~/.alius/soul/).
pub fn soul_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".alius").join("soul")
}

/// Install a soul formula from the repo to the global directory.
///
/// Copies the Formula TOML to ~/.alius/soul/<id>/versions/<version>/formula.toml
pub fn install_soul(formula: &FormulaDef) -> Result<PathBuf> {
    let dest = soul_dir().join(&formula.id).join("versions").join(&formula.version);
    std::fs::create_dir_all(&dest)?;

    let toml_content = toml::to_string_pretty(formula)?;
    std::fs::write(dest.join("formula.toml"), toml_content)?;

    Ok(dest)
}

/// List all installed souls.
pub fn list_installed_souls() -> Result<Vec<FormulaDef>> {
    let dir = soul_dir();
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut souls = Vec::new();
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let soul_path = entry.path();
        if !soul_path.is_dir() {
            continue;
        }
        // Find the latest version
        let versions_dir = soul_path.join("versions");
        if !versions_dir.exists() {
            continue;
        }
        let mut versions: Vec<String> = Vec::new();
        for ve in std::fs::read_dir(&versions_dir)? {
            let ve = ve?;
            if ve.path().is_dir() {
                if let Some(name) = ve.file_name().to_str() {
                    versions.push(name.to_string());
                }
            }
        }
        versions.sort();
        if let Some(latest) = versions.last() {
            let formula_path = versions_dir.join(latest).join("formula.toml");
            if formula_path.exists() {
                let content = std::fs::read_to_string(&formula_path)?;
                if let Ok(f) = toml::from_str::<FormulaDef>(&content) {
                    souls.push(f);
                }
            }
        }
    }

    souls.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(souls)
}

/// Activate a soul for the current project.
///
/// Creates ./alius/soul/<id>/ with a reference to the installed formula.
pub fn activate_soul(id: &str) -> Result<PathBuf> {
    let installed = soul_dir().join(id);
    if !installed.exists() {
        anyhow::bail!("Soul '{}' is not installed. Run: alius soul install {}", id, id);
    }

    let project_dir = PathBuf::from("alius").join("soul").join(id);
    std::fs::create_dir_all(&project_dir)?;

    // Write an activation marker
    std::fs::write(
        project_dir.join(".active"),
        format!("{}\n", id),
    )?;

    Ok(project_dir)
}

/// Get the currently activated soul ID for the project.
pub fn current_project_soul() -> Option<String> {
    let dir = PathBuf::from("alius").join("soul");
    if !dir.exists() {
        return None;
    }
    for entry in std::fs::read_dir(&dir).ok()? {
        let entry = entry.ok()?;
        let active_file = entry.path().join(".active");
        if active_file.exists() {
            let content = std::fs::read_to_string(&active_file).ok()?;
            let id = content.trim().to_string();
            if !id.is_empty() {
                return Some(id);
            }
        }
    }
    None
}

/// Remove an installed soul from the global directory.
pub fn remove_soul(id: &str) -> Result<()> {
    let dir = soul_dir().join(id);
    if !dir.exists() {
        anyhow::bail!("Soul '{}' is not installed", id);
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}
