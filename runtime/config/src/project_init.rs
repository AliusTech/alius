//! Project initialization IO for the `/init` wizard.
//!
//! The wizard itself stays pure in `init_wizard`; this module owns the local
//! filesystem side effects that can be retried and persisted by the CLI.

use crate::capability::CapabilityManager;
use crate::config_manager::find_project_root;
use crate::init_wizard::{GitStatus, InitWizard, WorkspaceCheckResult};
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::process::Command;

const DEFAULT_CONFIG_TOML: &str = r#"# Alius project configuration

[project]
name = ""
version = 1

[runtime]
default_mode = "plan"
tui_workspace = true
legacy_repl_env = "ALIUS_LEGACY_REPL"
auto_review = false

[model]
default_provider = ""
default_model = ""
router_profile = "standard"

[session]
persist_messages = true
persist_events = true

[logging]
enabled = true
level = "info"
redact_secrets = true
flush_error_immediately = true

[compat]
read_legacy_project_config = true
read_legacy_mcp_config = true
read_legacy_project_memory = true
read_legacy_design_docs = true
schema_version = "0.1"
"#;

const DEFAULT_MODEL_TOML: &str = r#"schema_version = "0.1"

[assignment]
plan = ""
execute = ""
review = ""
"#;

const DEFAULT_PROVIDERS_TOML: &str = r#"# Model providers and routing configuration

[router]
strategy = "tiered"
default_tier = "medium"
fallback_tier = "medium"

[tiers.light]
description = "Plan Model compatibility tier."
provider = "bigmodel"
model = ""

[tiers.medium]
description = "Execute Model compatibility tier."
provider = "bigmodel"
model = ""

[tiers.high]
description = "Review Model compatibility tier."
provider = "bigmodel"
model = ""

[providers.bigmodel]
enabled = true
kind = "openai-compatible"
base_url = "https://open.bigmodel.cn/api/coding/paas/v4"
api_key_env = "BIGMODEL_API_KEY"

[providers.xiaomi_mimo]
enabled = false
kind = "openai-compatible"
base_url = "https://api.xiaomimimo.com/v1"
api_key_env = "XIAOMI_MIMO_API_KEY"

[providers.deepseek]
enabled = false
kind = "openai-compatible"
base_url = "https://api.deepseek.com"
api_key_env = "DEEPSEEK_API_KEY"
"#;

const DEFAULT_SOUL_TOML: &str = r#"[soul]
role = ""
"#;

const DEFAULT_TOOLS_TOML: &str = r#"[tools]
enabled = true
"#;

const DEFAULT_PERMISSIONS_TOML: &str = r#"[filesystem]
mode = "workspace"

[[filesystem.roots]]
root = ".alius/workspace"
read = true
write = true

[network]
enabled = true
"#;

const DEFAULT_PROTOCOL_TOML: &str = r#"[protocol]
version = "0.1"

[json_rpc]
enabled = false
socket_path = ".alius/run/alius.sock"

[agent_card]
agent_card_source = ".alius/config/soul.toml"
"#;

const DEFAULT_MCP_JSON: &str = "{}\n";

/// Resolve the project root for init writes. Existing `.alius` wins; otherwise
/// the provided cwd becomes the new project root.
pub fn project_root_for_init(cwd: &Path) -> PathBuf {
    find_project_root(cwd).unwrap_or_else(|| cwd.to_path_buf())
}

/// Resolve the `.alius` directory for init writes.
pub fn project_alius_dir(cwd: &Path) -> PathBuf {
    project_root_for_init(cwd).join(".alius")
}

/// Persistent wizard state path.
pub fn init_state_path(cwd: &Path) -> PathBuf {
    project_alius_dir(cwd)
        .join("runtime")
        .join("init-state.toml")
}

/// Load a saved `/init` wizard, if present.
pub fn load_init_state(cwd: &Path) -> Result<Option<InitWizard>> {
    let path = init_state_path(cwd);
    if !path.exists() {
        return Ok(None);
    }
    let content =
        std::fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let wizard = toml::from_str(&content).with_context(|| format!("parse {}", path.display()))?;
    Ok(Some(wizard))
}

/// Save wizard progress to `.alius/runtime/init-state.toml`.
pub fn save_init_state(cwd: &Path, wizard: &InitWizard) -> Result<()> {
    let path = init_state_path(cwd);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    let content = toml::to_string_pretty(wizard)?;
    std::fs::write(&path, content).with_context(|| format!("write {}", path.display()))
}

/// Clear saved wizard progress.
pub fn clear_init_state(cwd: &Path) -> Result<()> {
    let path = init_state_path(cwd);
    if path.exists() {
        std::fs::remove_file(&path).with_context(|| format!("remove {}", path.display()))?;
    }
    Ok(())
}

/// Check whether the workspace can be initialized.
pub fn check_workspace(cwd: &Path) -> WorkspaceCheckResult {
    let alius_dir = project_alius_dir(cwd);
    let alius_existed = alius_dir.exists();
    if !cwd.exists() {
        return WorkspaceCheckResult {
            ok: false,
            git: None,
            message: Some(format!("Workspace does not exist: {}", cwd.display())),
            alius_existed,
        };
    }
    if !cwd.is_dir() {
        return WorkspaceCheckResult {
            ok: false,
            git: None,
            message: Some(format!("Workspace is not a directory: {}", cwd.display())),
            alius_existed,
        };
    }
    let writable_parent = alius_dir.parent().unwrap_or(cwd);
    let git = current_git_branch(cwd).map(|branch| GitStatus {
        branch: Some(branch),
    });

    match std::fs::create_dir_all(&alius_dir) {
        Ok(()) => {
            let probe = alius_dir.join(".init-write-check");
            match std::fs::write(&probe, b"ok").and_then(|_| std::fs::remove_file(&probe)) {
                Ok(()) => WorkspaceCheckResult {
                    ok: true,
                    git,
                    message: Some("Workspace is writable.".to_string()),
                    alius_existed,
                },
                Err(error) => WorkspaceCheckResult {
                    ok: false,
                    git,
                    message: Some(format!(
                        "Workspace is not writable at {}: {}",
                        writable_parent.display(),
                        error
                    )),
                    alius_existed,
                },
            }
        }
        Err(error) => WorkspaceCheckResult {
            ok: false,
            git,
            message: Some(format!("Cannot create {}: {}", alius_dir.display(), error)),
            alius_existed,
        },
    }
}

/// Create or complete `.alius/` project configuration without overwriting
/// existing user files.
pub fn ensure_project_defaults(cwd: &Path) -> Result<()> {
    let alius_dir = project_alius_dir(cwd);
    let config_dir = alius_dir.join("config");
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("create {}", config_dir.display()))?;

    let files = [
        ("config.toml", DEFAULT_CONFIG_TOML),
        ("model.toml", DEFAULT_MODEL_TOML),
        ("providers.toml", DEFAULT_PROVIDERS_TOML),
        ("soul.toml", DEFAULT_SOUL_TOML),
        ("tools.toml", DEFAULT_TOOLS_TOML),
        ("permissions.toml", DEFAULT_PERMISSIONS_TOML),
        ("protocol.toml", DEFAULT_PROTOCOL_TOML),
        ("mcp.json", DEFAULT_MCP_JSON),
    ];
    for (name, content) in files {
        write_if_missing(&config_dir.join(name), content)?;
    }

    ensure_memory_dirs(&alius_dir)?;
    std::fs::create_dir_all(alius_dir.join("workspace"))?;
    std::fs::create_dir_all(alius_dir.join("capability"))?;
    std::fs::create_dir_all(alius_dir.join("runtime"))?;
    Ok(())
}

/// Reset project configuration defaults while preserving memory/workspace data.
pub fn reset_project_defaults(cwd: &Path) -> Result<()> {
    let alius_dir = project_alius_dir(cwd);
    let config_dir = alius_dir.join("config");
    std::fs::create_dir_all(&config_dir)
        .with_context(|| format!("create {}", config_dir.display()))?;

    let legacy_config = alius_dir.join("config.toml");
    if legacy_config.exists() {
        std::fs::remove_file(&legacy_config)
            .with_context(|| format!("remove {}", legacy_config.display()))?;
    }

    write_file(&config_dir.join("config.toml"), DEFAULT_CONFIG_TOML)?;
    write_file(&config_dir.join("model.toml"), DEFAULT_MODEL_TOML)?;
    write_file(&config_dir.join("providers.toml"), DEFAULT_PROVIDERS_TOML)?;
    write_file(&config_dir.join("soul.toml"), DEFAULT_SOUL_TOML)?;
    write_file(&config_dir.join("tools.toml"), DEFAULT_TOOLS_TOML)?;
    write_file(
        &config_dir.join("permissions.toml"),
        DEFAULT_PERMISSIONS_TOML,
    )?;
    write_file(&config_dir.join("protocol.toml"), DEFAULT_PROTOCOL_TOML)?;
    write_file(&config_dir.join("mcp.json"), DEFAULT_MCP_JSON)?;

    ensure_memory_dirs(&alius_dir)?;
    std::fs::create_dir_all(alius_dir.join("workspace"))?;
    std::fs::create_dir_all(alius_dir.join("capability"))?;
    std::fs::create_dir_all(alius_dir.join("runtime"))?;
    Ok(())
}

/// Create the default workspace template directories.
pub fn create_workspace_template(cwd: &Path) -> Result<()> {
    let workspace = project_alius_dir(cwd).join("workspace");
    for subdir in [
        "architecture",
        "docs",
        "implementation",
        "plans",
        "specs",
        "tasks",
    ] {
        std::fs::create_dir_all(workspace.join(subdir))
            .with_context(|| format!("create workspace/{subdir}"))?;
    }
    Ok(())
}

/// Resolve and write an empty capability lock. Real capability derivation can
/// grow here without coupling the pure wizard to model/runtime crates.
pub fn resolve_capability_lock(cwd: &Path) -> Result<()> {
    let alius_dir = project_alius_dir(cwd);
    std::fs::create_dir_all(alius_dir.join("capability"))?;
    let manager = CapabilityManager::new(&alius_dir);
    let plan = CapabilityManager::resolve_capabilities(&[], &[], &[]);
    manager.install_capabilities(&plan)?;
    Ok(())
}

fn write_if_missing(path: &Path, content: &str) -> Result<()> {
    if !path.exists() {
        write_file(path, content)?;
    }
    Ok(())
}

fn write_file(path: &Path, content: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("create {}", parent.display()))?;
    }
    std::fs::write(path, content).with_context(|| format!("write {}", path.display()))
}

fn ensure_memory_dirs(alius_dir: &Path) -> Result<()> {
    let memory = alius_dir.join("memory");
    for subdir in [
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
            .with_context(|| format!("create memory/{subdir}"))?;
    }
    Ok(())
}

fn current_git_branch(cwd: &Path) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(cwd)
        .arg("rev-parse")
        .arg("--abbrev-ref")
        .arg("HEAD")
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let branch = String::from_utf8_lossy(&output.stdout).trim().to_string();
    (!branch.is_empty()).then_some(branch)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::init_wizard::{InitContext, InitState};
    use tempfile::TempDir;

    #[test]
    fn init_state_roundtrips_and_clears() {
        let dir = TempDir::new().unwrap();
        let mut wizard = InitWizard::with_context(InitContext::new(dir.path().to_path_buf()));
        wizard.state = InitState::SelectLanguage;

        save_init_state(dir.path(), &wizard).unwrap();
        let loaded = load_init_state(dir.path()).unwrap().unwrap();
        assert_eq!(loaded.state, InitState::SelectLanguage);

        clear_init_state(dir.path()).unwrap();
        assert!(load_init_state(dir.path()).unwrap().is_none());
    }

    #[test]
    fn ensure_project_defaults_creates_required_files() {
        let dir = TempDir::new().unwrap();

        ensure_project_defaults(dir.path()).unwrap();

        assert!(dir.path().join(".alius/config/config.toml").exists());
        assert!(dir.path().join(".alius/config/model.toml").exists());
        assert!(dir.path().join(".alius/config/providers.toml").exists());
        assert!(dir.path().join(".alius/runtime").exists());
    }

    #[test]
    fn reset_project_defaults_clears_role_and_locale() {
        let dir = TempDir::new().unwrap();

        reset_project_defaults(dir.path()).unwrap();

        let config = std::fs::read_to_string(dir.path().join(".alius/config/config.toml")).unwrap();
        let soul = std::fs::read_to_string(dir.path().join(".alius/config/soul.toml")).unwrap();
        assert!(!config.contains("[ui]"));
        assert!(soul.contains("role = \"\""));
    }

    #[test]
    fn workspace_template_creates_known_dirs() {
        let dir = TempDir::new().unwrap();

        create_workspace_template(dir.path()).unwrap();

        assert!(dir.path().join(".alius/workspace/plans").exists());
        assert!(dir.path().join(".alius/workspace/specs").exists());
    }
}
