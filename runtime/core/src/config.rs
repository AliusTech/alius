//! Config Manager — Core Runtime submodule.
//!
//! Manages the `.alius/config/` directory lifecycle: initialization, reset,
//! and default template embedding. Only touches configuration files;
//! memory and workspace data are preserved during reset operations.

use std::path::{Path, PathBuf};

const DEFAULT_CONFIG_TOML: &str = include_str!("../config/defaults/config.toml");
const DEFAULT_PROVIDERS_TOML: &str = include_str!("../config/defaults/providers.toml");
const DEFAULT_SOUL_TOML: &str = include_str!("../config/defaults/soul.toml");
const DEFAULT_TOOLS_TOML: &str = include_str!("../config/defaults/tools.toml");
const DEFAULT_PERMISSIONS_TOML: &str = include_str!("../config/defaults/permissions.toml");
const DEFAULT_PROTOCOL_TOML: &str = include_str!("../config/defaults/protocol.toml");
const DEFAULT_MCP_JSON: &str = include_str!("../config/defaults/mcp.json");

/// Check whether a project-level `.alius/` directory exists.
pub fn project_config_exists() -> bool {
    project_alius_dir_for_write()
        .map(|dir| dir.exists())
        .unwrap_or(false)
}

/// Reset all project configuration to defaults.
///
/// 1. Deletes the legacy flat `.alius/config.toml` if it exists.
/// 2. Creates `.alius/config/` directory.
/// 3. Overwrites all files in `.alius/config/` with embedded defaults.
/// 4. If `preserve_locale` is provided, appends `[ui] locale = "..."` to config.toml
///    so the init wizard and subsequent loads retain the user's language choice.
/// 5. Ensures `.alius/memory/` subdirectories exist (does NOT clear memory data).
/// 6. Ensures `.alius/workspace/` directory exists.
pub fn reset_project_config(preserve_locale: Option<&str>) -> anyhow::Result<()> {
    let alius_dir = project_alius_dir_for_write()
        .map_err(|e| anyhow::anyhow!("Failed to resolve project directory: {}", e))?;

    // 1. Delete legacy flat config.toml
    let legacy_config = alius_dir.join("config.toml");
    if legacy_config.exists() {
        std::fs::remove_file(&legacy_config)?;
    }

    // 2. Create config/ directory
    let config_dir = alius_dir.join("config");
    std::fs::create_dir_all(&config_dir)?;

    // 3. Write all default config files
    write_file(&config_dir.join("config.toml"), DEFAULT_CONFIG_TOML)?;
    write_file(&config_dir.join("providers.toml"), DEFAULT_PROVIDERS_TOML)?;
    write_file(&config_dir.join("soul.toml"), DEFAULT_SOUL_TOML)?;
    write_file(&config_dir.join("tools.toml"), DEFAULT_TOOLS_TOML)?;
    write_file(
        &config_dir.join("permissions.toml"),
        DEFAULT_PERMISSIONS_TOML,
    )?;
    write_file(&config_dir.join("protocol.toml"), DEFAULT_PROTOCOL_TOML)?;
    write_file(&config_dir.join("mcp.json"), DEFAULT_MCP_JSON)?;

    // 4. Preserve locale if provided
    if let Some(locale) = preserve_locale {
        let config_path = config_dir.join("config.toml");
        let existing = std::fs::read_to_string(&config_path)?;
        let updated = format!("{}\n\n[ui]\nlocale = \"{}\"\n", existing.trim_end(), locale);
        std::fs::write(&config_path, updated)?;
    }

    // 5. Ensure memory/ subdirectories exist
    ensure_memory_dirs(&alius_dir)?;

    // 6. Ensure workspace/ exists
    std::fs::create_dir_all(alius_dir.join("workspace"))?;

    Ok(())
}

/// Ensure the full `.alius/` directory structure exists without overwriting.
///
/// Creates directories and files that don't exist yet. Safe to call on an
/// already-initialized project — existing files are never modified.
pub fn ensure_full_project_structure() -> anyhow::Result<()> {
    let alius_dir = project_alius_dir_for_write()
        .map_err(|e| anyhow::anyhow!("Failed to resolve project directory: {}", e))?;

    let config_dir = alius_dir.join("config");
    std::fs::create_dir_all(&config_dir)?;

    // Write defaults only for files that don't exist yet
    let files: &[(&str, &str)] = &[
        ("config.toml", DEFAULT_CONFIG_TOML),
        ("providers.toml", DEFAULT_PROVIDERS_TOML),
        ("soul.toml", DEFAULT_SOUL_TOML),
        ("tools.toml", DEFAULT_TOOLS_TOML),
        ("permissions.toml", DEFAULT_PERMISSIONS_TOML),
        ("protocol.toml", DEFAULT_PROTOCOL_TOML),
        ("mcp.json", DEFAULT_MCP_JSON),
    ];
    for (name, content) in files {
        let path = config_dir.join(name);
        if !path.exists() {
            write_file(&path, content)?;
        }
    }

    ensure_memory_dirs(&alius_dir)?;
    std::fs::create_dir_all(alius_dir.join("workspace"))?;

    Ok(())
}

fn write_file(path: &Path, content: &str) -> anyhow::Result<()> {
    std::fs::write(path, content)
        .map_err(|e| anyhow::anyhow!("Failed to write {}: {}", path.display(), e))
}

fn project_alius_dir_for_write() -> std::io::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from);
    let mut dir = cwd.as_path();

    loop {
        if home.as_deref() == Some(dir) {
            break;
        }

        if dir.join(".alius").exists() {
            return Ok(dir.join(".alius"));
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => break,
        }
    }

    Ok(cwd.join(".alius"))
}

fn ensure_memory_dirs(alius_dir: &Path) -> anyhow::Result<()> {
    let memory = alius_dir.join("memory");
    for subdir in &[
        "cache",
        "communications/sessions",
        "design",
        "episodic",
        "index",
        "procedural",
        "semantic",
        "logs",
    ] {
        std::fs::create_dir_all(memory.join(subdir))
            .map_err(|e| anyhow::anyhow!("Failed to create memory dir: {}", e))?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    #[test]
    fn test_reset_creates_all_config_files() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        reset_project_config(None).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(project_dir.join(".alius/config/config.toml").exists());
        assert!(project_dir.join(".alius/config/providers.toml").exists());
        assert!(project_dir.join(".alius/config/soul.toml").exists());
        assert!(project_dir.join(".alius/config/tools.toml").exists());
        assert!(project_dir.join(".alius/config/permissions.toml").exists());
        assert!(project_dir.join(".alius/config/protocol.toml").exists());
        assert!(project_dir.join(".alius/config/mcp.json").exists());
    }

    #[test]
    fn test_reset_deletes_legacy_flat_config() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".alius")).unwrap();
        std::fs::write(
            project_dir.join(".alius/config.toml"),
            "[llm]\nmodel = \"old\"\n",
        )
        .unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        reset_project_config(None).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(!project_dir.join(".alius/config.toml").exists());
        assert!(project_dir.join(".alius/config/config.toml").exists());
    }

    #[test]
    fn test_reset_creates_memory_directories() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        reset_project_config(None).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(project_dir.join(".alius/memory/cache").exists());
        assert!(project_dir
            .join(".alius/memory/communications/sessions")
            .exists());
        assert!(project_dir.join(".alius/memory/design").exists());
        assert!(project_dir.join(".alius/memory/episodic").exists());
        assert!(project_dir.join(".alius/memory/semantic").exists());
        assert!(project_dir.join(".alius/memory/procedural").exists());
        assert!(project_dir.join(".alius/memory/index").exists());
        assert!(project_dir.join(".alius/memory/logs").exists());
    }

    #[test]
    fn test_reset_preserves_memory_data() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".alius/memory/design")).unwrap();
        std::fs::write(
            project_dir.join(".alius/memory/design/important.md"),
            "data",
        )
        .unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        reset_project_config(None).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(project_dir
            .join(".alius/memory/design/important.md")
            .exists());
        assert_eq!(
            std::fs::read_to_string(project_dir.join(".alius/memory/design/important.md")).unwrap(),
            "data"
        );
    }

    #[test]
    fn test_reset_overwrites_existing_config_files() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".alius/config")).unwrap();
        std::fs::write(
            project_dir.join(".alius/config/config.toml"),
            "custom: value",
        )
        .unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        reset_project_config(None).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        let content =
            std::fs::read_to_string(project_dir.join(".alius/config/config.toml")).unwrap();
        assert_ne!(content, "custom: value");
        assert!(content.contains("[project]"));
    }

    #[test]
    fn test_ensure_does_not_overwrite_existing() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".alius/config")).unwrap();
        std::fs::write(
            project_dir.join(".alius/config/config.toml"),
            "custom: value",
        )
        .unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        ensure_full_project_structure().unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        let content =
            std::fs::read_to_string(project_dir.join(".alius/config/config.toml")).unwrap();
        assert_eq!(content, "custom: value");
    }

    #[test]
    fn test_reset_preserves_locale() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        reset_project_config(Some("zh-CN")).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        let content =
            std::fs::read_to_string(project_dir.join(".alius/config/config.toml")).unwrap();
        assert!(content.contains("[ui]"));
        assert!(content.contains("locale = \"zh-CN\""));
        assert!(content.contains("[project]"));
    }
}
