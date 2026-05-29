//! Alius Formula — Repository management for Soul/Plugin formulas.
//!
//! Manages the alius-core formula repository:
//! - Clone/fetch from remote git repo
//! - Parse TOML formula definitions
//! - List and query available formulas

use anyhow::Result;
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Default remote URL for the official alius-core repository.
pub const OFFICIAL_REMOTE: &str = "git@github.com:AliusTech/alius-core.git";

/// A parsed formula definition from TOML.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Deserialize)]
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
