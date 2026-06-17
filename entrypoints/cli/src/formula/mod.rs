//! Alius Formula — Repository management for Soul/Plugin formulas.
//!
//! Manages the official Alius Soul repository:
//! - Clone/fetch from remote git repo
//! - Parse TOML formula definitions
//! - List and query available formulas

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Legacy remote URL for the official Alius Soul repository.
///
/// **Deprecated**: Official souls are now bundled in the main repository under
/// `extensions/souls/`. Use [`bundled_souls_path`] instead. This constant is
/// kept only for backward compatibility and migration.
#[deprecated(note = "Use bundled_souls_path() — official souls are now in extensions/souls/")]
pub const OFFICIAL_REMOTE: &str = "git@github.com:AliusTech/alius-souls.git";

/// Legacy public fallback URL.
///
/// **Deprecated**: See [`OFFICIAL_REMOTE`].
#[deprecated(note = "Use bundled_souls_path() — official souls are now in extensions/souls/")]
pub const OFFICIAL_HTTPS_REMOTE: &str = "https://github.com/AliusTech/alius-souls.git";

/// Get the path to the bundled souls repository root in the main repository.
///
/// Returns the root that contains `Formula/souls/` — i.e. `extensions/souls`.
/// This matches the layout expected by `sync_souls_from_repo()` and `list_formulas()`.
///
/// Looks relative to:
/// 1. The directory containing the current executable (release builds)
/// 2. `CARGO_MANIFEST_DIR` (development builds)
/// 3. Current working directory
///
/// Returns `None` if the bundled directory is not found.
pub fn bundled_souls_path() -> Option<PathBuf> {
    // Try relative to the executable first (release builds)
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let path = exe_dir.join("extensions/souls");
            if path.join("Formula").join("souls").exists() {
                return Some(path);
            }
            if let Some(parent) = exe_dir.parent() {
                let path = parent.join("extensions/souls");
                if path.join("Formula").join("souls").exists() {
                    return Some(path);
                }
            }
        }
    }

    // Try relative to CARGO_MANIFEST_DIR (development builds)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let path = PathBuf::from(manifest_dir)
            .join("..")
            .join("extensions/souls");
        if path.join("Formula").join("souls").exists() {
            return Some(path);
        }
    }

    // Try current working directory
    if let Ok(cwd) = std::env::current_dir() {
        let path = cwd.join("extensions/souls");
        if path.join("Formula").join("souls").exists() {
            return Some(path);
        }
    }

    None
}

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

/// Get the local path for the official Soul repository.
pub fn official_repo_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home)
        .join(".alius")
        .join("repos")
        .join("souls")
}

/// Clone or update the official Alius Soul repository (legacy fallback).
///
/// This function is the fallback when bundled souls are not available.
/// Prefer [`sync_all_souls`] which tries the bundled path first.
#[allow(deprecated)]
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
            .args([
                "-C",
                path.to_str().unwrap(),
                "reset",
                "--hard",
                "origin/main",
            ])
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
            let status = std::process::Command::new("git")
                .args(["clone", OFFICIAL_HTTPS_REMOTE, path.to_str().unwrap()])
                .status()?;
            if !status.success() {
                anyhow::bail!("git clone failed");
            }
        }
    }
    Ok(path)
}

/// Check if the official repo has remote updates available (without applying them).
/// Returns `Ok(true)` if there are new commits on `origin/main` that are not
/// yet in the local `HEAD`.
pub fn check_soul_updates() -> Result<bool> {
    let path = official_repo_path();
    if !path.join(".git").exists() {
        return Ok(false);
    }
    let path_str = match path.to_str() {
        Some(s) => s,
        None => return Ok(false),
    };

    // Fetch latest refs
    let _ = std::process::Command::new("git")
        .args(["-C", path_str, "fetch", "--all"])
        .output();

    // Compare HEAD vs origin/main
    let output = std::process::Command::new("git")
        .args(["-C", path_str, "rev-parse", "HEAD", "origin/main"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut lines = stdout.lines();
    let head = lines.next().unwrap_or("").trim();
    let origin = lines.next().unwrap_or("").trim();

    Ok(!head.is_empty() && !origin.is_empty() && head != origin)
}

/// Sync the official Soul repository (clone or fetch).
///
/// Wrapper around [`update_repo`] with a clearer public name.
pub fn sync_official_repo() -> Result<PathBuf> {
    update_repo()
}

/// Sync all official soul formulas into `~/.alius/soul`.
///
/// Tries the bundled `extensions/souls` path first (no network required).
/// Falls back to cloning from the legacy git remote if bundled path is not found.
pub fn sync_all_souls() -> Result<Vec<FormulaDef>> {
    if let Some(bundled) = bundled_souls_path() {
        return sync_souls_from_repo(&bundled);
    }
    // Fallback to legacy git remote
    let repo_path = sync_official_repo()?;
    sync_souls_from_repo(&repo_path)
}

/// Install or refresh all soul formulas from a local formula repository.
pub fn sync_souls_from_repo(repo_path: &Path) -> Result<Vec<FormulaDef>> {
    let souls = list_formulas(repo_path, "souls")?;
    for soul in &souls {
        install_soul(soul, repo_path)?;
    }
    Ok(souls)
}

/// List all available souls from the official repository.
///
/// Tries the bundled path first (no network), then falls back to the legacy
/// git clone if available.
pub fn list_available_souls() -> Result<Vec<FormulaDef>> {
    if let Some(bundled) = bundled_souls_path() {
        return list_formulas(&bundled, "souls");
    }
    // Fallback: try the legacy repo cache
    let repo_path = official_repo_path();
    if repo_path.exists() {
        return list_formulas(&repo_path, "souls");
    }
    Ok(vec![])
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
/// Copies the Formula TOML and prompts/ to ~/.alius/soul/<id>/versions/<version>/
pub fn install_soul(formula: &FormulaDef, repo_path: &Path) -> Result<PathBuf> {
    let dest = soul_dir()
        .join(&formula.id)
        .join("versions")
        .join(&formula.version);
    std::fs::create_dir_all(&dest)?;

    let toml_content = toml::to_string_pretty(formula)?;
    std::fs::write(dest.join("formula.toml"), toml_content)?;

    // Copy prompt files (identity.md, style.md, rules.md) from the repo
    let src_soul = repo_path.join("Formula").join("souls").join(&formula.id);
    if src_soul.exists() {
        let dest_prompts = dest.join("prompts");
        std::fs::create_dir_all(&dest_prompts)?;
        for name in &["identity.md", "style.md", "rules.md"] {
            let src_file = src_soul.join(name);
            if src_file.exists() {
                std::fs::copy(&src_file, dest_prompts.join(name))?;
            }
        }
    }

    Ok(dest)
}

/// Load the system prompt from an installed soul's prompts/ directory.
///
/// Reads identity.md, style.md, rules.md and concatenates them.
/// Returns None if the soul is not installed or has no prompts.
pub fn load_soul_prompts(id: &str) -> Option<String> {
    let dir = soul_dir().join(id);
    if !dir.exists() {
        return None;
    }

    // Find the latest version
    let versions_dir = dir.join("versions");
    let mut versions: Vec<String> = std::fs::read_dir(&versions_dir)
        .ok()?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter_map(|e| e.file_name().into_string().ok())
        .collect();
    versions.sort();
    let latest = versions.last()?;

    let prompts_dir = versions_dir.join(latest).join("prompts");
    if !prompts_dir.exists() {
        return None;
    }

    let mut parts = Vec::new();
    for name in &["identity.md", "style.md", "rules.md"] {
        let path = prompts_dir.join(name);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                parts.push(content.trim().to_string());
            }
        }
    }

    if parts.is_empty() {
        None
    } else {
        Some(parts.join("\n\n"))
    }
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

/// Get the path to the unified active soul marker file for the current project.
///
/// Located at `./.alius/soul/.active` relative to the project root.
fn active_soul_file() -> PathBuf {
    project_alius_dir().join("soul").join(".active")
}

/// Migrate from legacy per-directory `.active` files to the unified marker.
///
/// Scans `./alius/soul/<id>/.active` for old activation markers. If found,
/// writes the first valid ID to `./.alius/soul/.active` and removes the old marker.
fn migrate_legacy_active() -> Option<String> {
    let legacy_active = PathBuf::from("alius").join("soul").join(".active");
    if let Some(id) = migrate_active_file(&legacy_active) {
        return Some(id);
    }

    let legacy_dir = PathBuf::from("alius").join("soul");
    if !legacy_dir.exists() {
        return None;
    }

    let entries = std::fs::read_dir(&legacy_dir).ok()?;
    for entry in entries.flatten() {
        if let Some(id) = migrate_active_file(&entry.path().join(".active")) {
            return Some(id);
        }
    }
    None
}

fn migrate_active_file(active_file: &Path) -> Option<String> {
    if !active_file.exists() {
        return None;
    }

    let content = std::fs::read_to_string(active_file).ok()?;
    let id = content.trim().to_string();
    if id.is_empty() {
        return None;
    }

    let new_path = active_soul_file();
    if let Some(parent) = new_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if std::fs::write(&new_path, format!("{}\n", id)).is_ok() {
        let _ = std::fs::remove_file(active_file);
        Some(id)
    } else {
        None
    }
}

fn project_alius_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    let mut dir = cwd.as_path();
    loop {
        if home.as_deref() == Some(dir) {
            return cwd.join(".alius");
        }

        let candidate = dir.join(".alius");
        if candidate.exists() {
            return candidate;
        }
        match dir.parent() {
            Some(parent) => dir = parent,
            None => return cwd.join(".alius"),
        }
    }
}

/// Activate a soul for the current project.
///
/// Writes the soul ID to `./.alius/soul/.active`. The soul must already be installed.
pub fn activate_soul(id: &str) -> Result<PathBuf> {
    let installed = soul_dir().join(id);
    if !installed.exists() {
        anyhow::bail!(
            "Soul '{}' is not installed. Run: alius soul install {}",
            id,
            id
        );
    }

    let active_path = active_soul_file();
    if let Some(parent) = active_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&active_path, format!("{}\n", id))?;

    Ok(active_path)
}

/// Get the currently activated soul ID for the project.
///
/// Reads from `./.alius/soul/.active`. Falls back to migrating legacy
/// per-directory `.active` files if the unified marker doesn't exist.
pub fn current_project_soul() -> Option<String> {
    let active_path = active_soul_file();
    if active_path.exists() {
        let content = std::fs::read_to_string(&active_path).ok()?;
        let id = content.trim().to_string();
        if !id.is_empty() {
            return Some(id);
        }
    }

    // Try legacy migration
    migrate_legacy_active()
}

/// Install and activate a soul by formula ID.
///
/// Looks up the formula in the bundled extensions or legacy repo,
/// installs it to the global directory, and activates it for the current project.
pub fn install_and_activate_soul(id: &str) -> Result<FormulaDef> {
    if !soul_dir().join(id).exists() {
        // Try bundled first, then legacy
        if let Some(bundled) = bundled_souls_path() {
            sync_souls_from_repo(&bundled)?;
        } else {
            sync_all_souls()?;
        }
    }

    let formula = list_installed_souls()?
        .into_iter()
        .find(|s| s.id == id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Soul '{}' not found. Run 'alius soul update' to sync local souls.",
                id
            )
        })?;

    activate_soul(id)?;

    Ok(formula)
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

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn make_formula(id: &str) -> FormulaDef {
        FormulaDef {
            id: id.to_string(),
            name: format!("{} Soul", id),
            version: "0.1.0".to_string(),
            formula_type: "soul".to_string(),
            description: format!("A {} soul", id),
            license: None,
            model: None,
        }
    }

    /// Create a mock formula repo in a temp dir with given formula definitions.
    fn mock_repo(dir: &std::path::Path, formulas: &[FormulaDef]) {
        let souls_dir = dir.join("Formula").join("souls");
        std::fs::create_dir_all(&souls_dir).unwrap();
        for f in formulas {
            let toml_content = toml::to_string_pretty(f).unwrap();
            std::fs::write(souls_dir.join(format!("{}.toml", f.id)), toml_content).unwrap();
            // Create prompt dir
            let prompt_dir = souls_dir.join(&f.id);
            std::fs::create_dir_all(&prompt_dir).unwrap();
            std::fs::write(prompt_dir.join("identity.md"), format!("I am {}", f.id)).unwrap();
            std::fs::write(prompt_dir.join("style.md"), "concise").unwrap();
            std::fs::write(prompt_dir.join("rules.md"), "be helpful").unwrap();
        }
    }

    /// Create a mock installed soul in a temp "HOME" directory.
    #[allow(dead_code)]
    fn mock_installed(home: &std::path::Path, formula: &FormulaDef) {
        let dest = home
            .join(".alius")
            .join("soul")
            .join(&formula.id)
            .join("versions")
            .join(&formula.version);
        std::fs::create_dir_all(dest.join("prompts")).unwrap();
        let toml_content = toml::to_string_pretty(formula).unwrap();
        std::fs::write(dest.join("formula.toml"), toml_content).unwrap();
        std::fs::write(dest.join("prompts").join("identity.md"), "identity").unwrap();
    }

    #[test]
    #[allow(deprecated)]
    fn official_remote_uses_maintainer_ssh_url() {
        assert_eq!(OFFICIAL_REMOTE, "git@github.com:AliusTech/alius-souls.git");
        assert_eq!(
            OFFICIAL_HTTPS_REMOTE,
            "https://github.com/AliusTech/alius-souls.git"
        );
    }

    struct EnvGuard {
        orig_home: Option<String>,
        orig_cwd: PathBuf,
        _lock: std::sync::MutexGuard<'static, ()>,
    }

    impl EnvGuard {
        fn save() -> Self {
            let lock = TEST_ENV_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner());
            Self {
                orig_home: std::env::var("HOME").ok(),
                orig_cwd: std::env::current_dir().unwrap(),
                _lock: lock,
            }
        }

        fn set_home_and_cwd(home: &Path, cwd: &Path) -> Self {
            let g = Self::save();
            std::env::set_var("HOME", home);
            std::env::set_current_dir(cwd).unwrap();
            g
        }

        fn set_cwd(cwd: &Path) -> Self {
            let g = Self::save();
            std::env::set_current_dir(cwd).unwrap();
            g
        }
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            match &self.orig_home {
                Some(h) => std::env::set_var("HOME", h),
                None => std::env::remove_var("HOME"),
            }
            let _ = std::env::set_current_dir(&self.orig_cwd);
        }
    }

    #[test]
    fn test_activate_soul_creates_single_active_file() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let home_dir = tmp.path().join("home");
        let soul_dest = home_dir
            .join(".alius")
            .join("soul")
            .join("coder")
            .join("versions")
            .join("0.1.0");
        std::fs::create_dir_all(&soul_dest).unwrap();
        std::fs::write(soul_dest.join("formula.toml"), "").unwrap();

        let _guard = EnvGuard::set_home_and_cwd(&home_dir, &project_dir);

        let result = activate_soul("coder");
        assert!(result.is_ok(), "activate_soul failed: {:?}", result);

        let active_file = active_soul_file();
        assert!(active_file.exists(), ".active file should exist");
        let content = std::fs::read_to_string(&active_file).unwrap();
        assert_eq!(content.trim(), "coder");
    }

    #[test]
    fn test_current_project_soul_reads_active_file() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        let soul_dir = project_dir.join(".alius").join("soul");
        std::fs::create_dir_all(&soul_dir).unwrap();
        std::fs::write(soul_dir.join(".active"), "coder\n").unwrap();

        let _guard = EnvGuard::set_cwd(&project_dir);

        let result = current_project_soul();
        assert_eq!(result, Some("coder".to_string()));
    }

    #[test]
    fn test_current_project_soul_returns_none_when_no_active() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let _guard = EnvGuard::set_cwd(&project_dir);

        let result = current_project_soul();
        assert!(result.is_none());
    }

    #[test]
    fn test_migrate_legacy_active() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        let soul_dir = project_dir.join("alius").join("soul");
        let legacy_dir = soul_dir.join("coder");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::write(legacy_dir.join(".active"), "coder\n").unwrap();

        let _guard = EnvGuard::set_cwd(&project_dir);

        let result = current_project_soul();
        assert_eq!(result, Some("coder".to_string()));

        // Verify migration: new .active file exists, old is gone
        assert!(active_soul_file().exists());
        assert!(!legacy_dir.join(".active").exists());
    }

    #[test]
    fn test_consecutive_activations_return_last() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let home_dir = tmp.path().join("home");
        for id in &["coder", "researcher"] {
            let soul_dest = home_dir
                .join(".alius")
                .join("soul")
                .join(id)
                .join("versions")
                .join("0.1.0");
            std::fs::create_dir_all(&soul_dest).unwrap();
            std::fs::write(soul_dest.join("formula.toml"), "").unwrap();
        }

        let _guard = EnvGuard::set_home_and_cwd(&home_dir, &project_dir);

        activate_soul("coder").unwrap();
        activate_soul("researcher").unwrap();

        let result = current_project_soul();
        assert_eq!(result, Some("researcher".to_string()));
    }

    #[test]
    fn test_activate_soul_rejects_uninstalled() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let home_dir = tmp.path().join("home");
        let _guard = EnvGuard::set_home_and_cwd(&home_dir, &project_dir);

        let result = activate_soul("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not installed"));
    }

    #[test]
    fn test_install_and_activate_soul_success() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let home_dir = tmp.path().join("home");
        let repo_dir = tmp.path().join("repo");
        mock_repo(&repo_dir, &[make_formula("coder")]);

        let _guard = EnvGuard::set_home_and_cwd(&home_dir, &project_dir);

        // Use install_soul + activate_soul directly (bypass install_and_activate_soul
        // which calls official_repo_path)
        let repo_path = repo_dir;
        let formula = find_formula(&repo_path, "souls", "coder").unwrap().unwrap();
        install_soul(&formula, &repo_path).unwrap();
        activate_soul("coder").unwrap();

        // Verify installed
        assert!(home_dir.join(".alius").join("soul").join("coder").exists());
        // Verify active
        assert_eq!(current_project_soul(), Some("coder".to_string()));
    }

    #[test]
    fn test_sync_souls_from_repo_installs_all_souls() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        let home_dir = tmp.path().join("home");
        let repo_dir = tmp.path().join("repo");
        mock_repo(
            &repo_dir,
            &[make_formula("coder"), make_formula("researcher")],
        );

        let _guard = EnvGuard::set_home_and_cwd(&home_dir, &project_dir);

        let synced = sync_souls_from_repo(&repo_dir).unwrap();
        assert_eq!(synced.len(), 2);
        assert!(home_dir.join(".alius").join("soul").join("coder").exists());
        assert!(home_dir
            .join(".alius")
            .join("soul")
            .join("researcher")
            .exists());

        let installed = list_installed_souls().unwrap();
        assert_eq!(installed.len(), 2);
        assert!(installed.iter().any(|s| s.id == "coder"));
        assert!(installed.iter().any(|s| s.id == "researcher"));
    }

    #[test]
    fn test_list_available_souls_empty_repo() {
        let tmp = tempfile::tempdir().unwrap();
        let repo_dir = tmp.path().join("repo");
        let souls_dir = repo_dir.join("Formula").join("souls");
        std::fs::create_dir_all(&souls_dir).unwrap();

        let result = list_formulas(&repo_dir, "souls").unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_sync_from_bundled_path() {
        let _guard = EnvGuard::save();
        let tmp = tempfile::tempdir().unwrap();

        // Create a mock bundled souls structure
        let bundled = tmp.path().join("extensions/souls/Formula/souls");
        std::fs::create_dir_all(bundled.join("test-soul")).unwrap();

        let toml = r#"
id = "test-soul"
name = "Test Soul"
version = "0.1.0"
type = "soul"
description = "A test soul"
"#;
        std::fs::write(bundled.join("test-soul.toml"), toml).unwrap();
        std::fs::write(bundled.join("test-soul/identity.md"), "I am a test").unwrap();
        std::fs::write(bundled.join("test-soul/style.md"), "Be testing").unwrap();
        std::fs::write(bundled.join("test-soul/rules.md"), "Always test").unwrap();

        // Set HOME to tmp so soul_dir() uses our temp
        std::env::set_var("HOME", tmp.path().join("home").to_str().unwrap());

        // Sync using the bundled path
        let souls = sync_souls_from_repo(tmp.path().join("extensions/souls").as_path()).unwrap();
        assert_eq!(souls.len(), 1);
        assert_eq!(souls[0].id, "test-soul");

        // Verify installed
        let installed = list_installed_souls().unwrap();
        assert_eq!(installed.len(), 1);
        assert_eq!(installed[0].id, "test-soul");

        // Verify prompts copied
        let prompts = load_soul_prompts("test-soul").unwrap();
        assert!(prompts.contains("I am a test"));
        assert!(prompts.contains("Be testing"));
        assert!(prompts.contains("Always test"));
    }
}
