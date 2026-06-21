//! Config Manager core interface.
//!
//! This module provides the main entry points for loading and managing
//! project configuration.

use crate::error::{ConfigError, ConfigResult};
use crate::loaders::{
    load_config, load_or_migrate_model_assignment, load_permissions, load_protocol, load_providers,
    load_soul, load_tools,
};
use crate::views::{
    LogLevel, LoggingConfig, PermissionConfig, ProjectConfigSnapshot, ProviderConfig,
    ResolvedPermissionConfig, ResolvedProvider, ResolvedProviderConfig, ResolvedSoulConfig,
    ResolvedToolConfig, RuntimeConfigView, SessionConfig, ShellGateConfig, SoulConfig, ToolConfig,
};
use std::path::{Path, PathBuf};

/// Find the project root directory by searching upward for `.alius/`.
pub fn find_project_root(cwd: &Path) -> Option<PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from);

    let mut dir: &Path = cwd;
    loop {
        // Stop at home directory
        if home.as_deref() == Some(dir) {
            return None;
        }

        // Check for .alius directory
        if dir.join(".alius").exists() {
            return Some(dir.to_path_buf());
        }

        // Move up
        dir = dir.parent()?;
    }
}

/// Load the complete project configuration snapshot.
///
/// For each config file:
/// - If the file doesn't exist, use defaults
/// - If the file exists but fails to parse, propagate the error
pub fn load_project_config(cwd: &Path) -> ConfigResult<ProjectConfigSnapshot> {
    let project_root = find_project_root(cwd).ok_or(ConfigError::ProjectRootNotFound)?;

    let config_dir = project_root.join(".alius/config");

    // Load each config file, using defaults only if missing
    let config = load_config_if_exists(&config_dir.join("config.toml"))?.unwrap_or_default();

    let providers =
        load_providers_if_exists(&config_dir.join("providers.toml"))?.unwrap_or_default();

    let model_assignment =
        load_or_migrate_model_assignment(&config_dir.join("model.toml"), &providers)?;

    let tools = load_tools_if_exists(&config_dir.join("tools.toml"))?.unwrap_or_default();

    let permissions =
        load_permissions_if_exists(&config_dir.join("permissions.toml"))?.unwrap_or_default();

    let protocol = load_protocol_if_exists(&config_dir.join("protocol.toml"))?.unwrap_or_default();

    let soul = load_soul_if_exists(&config_dir.join("soul.toml"))?.unwrap_or_default();

    Ok(ProjectConfigSnapshot {
        project: config.project,
        runtime: config.runtime,
        model: config.model,
        model_assignment,
        session: config.session,
        logging: config.logging,
        compat: config.compat,
        providers,
        tools,
        permissions,
        protocol,
        soul,
    })
}

/// Load a config file if it exists, returning None for missing files.
/// Propagates parse errors for existing files.
fn load_config_if_exists(path: &Path) -> ConfigResult<Option<crate::loaders::ConfigToml>> {
    if !path.exists() {
        return Ok(None);
    }
    load_config(path).map(Some)
}

/// Load providers.toml if it exists.
fn load_providers_if_exists(path: &Path) -> ConfigResult<Option<ProviderConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    load_providers(path).map(Some)
}

/// Load tools.toml if it exists.
fn load_tools_if_exists(path: &Path) -> ConfigResult<Option<ToolConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    load_tools(path).map(Some)
}

/// Load permissions.toml if it exists.
fn load_permissions_if_exists(path: &Path) -> ConfigResult<Option<PermissionConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    load_permissions(path).map(Some)
}

/// Load protocol.toml if it exists.
fn load_protocol_if_exists(path: &Path) -> ConfigResult<Option<crate::views::ProtocolConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    load_protocol(path).map(Some)
}

/// Load soul.toml if it exists.
fn load_soul_if_exists(path: &Path) -> ConfigResult<Option<SoulConfig>> {
    if !path.exists() {
        return Ok(None);
    }
    load_soul(path).map(Some)
}

/// Build the runtime configuration view from a snapshot.
pub fn build_runtime_config(
    snapshot: &ProjectConfigSnapshot,
    workspace_root: &Path,
) -> ConfigResult<RuntimeConfigView> {
    // Resolve provider configuration
    let provider = resolve_provider_config(&snapshot.providers, &snapshot.model)?;

    // Resolve tool configuration
    let tools = resolve_tool_config(&snapshot.tools);

    // Resolve permission configuration
    let permissions = resolve_permission_config(&snapshot.permissions);

    // Build shell gate configuration
    let shell_gate = build_shell_gate_config(&snapshot.permissions.shell, workspace_root);

    // Build logging configuration
    let logging = build_logging_config(&snapshot.logging, workspace_root);

    // Build session configuration
    let session = build_session_config(&snapshot.session, workspace_root);

    // Resolve soul configuration
    let soul = resolve_soul_config(&snapshot.soul);

    Ok(RuntimeConfigView {
        provider,
        tools,
        permissions,
        shell_gate,
        logging,
        session,
        soul,
    })
}

/// Resolve provider configuration with API keys.
fn resolve_provider_config(
    config: &ProviderConfig,
    model: &crate::views::ModelMeta,
) -> ConfigResult<ResolvedProviderConfig> {
    let mut providers: Vec<ResolvedProvider> = Vec::new();

    for (name, settings) in &config.providers {
        if !settings.enabled {
            continue;
        }

        let api_key = std::env::var(&settings.api_key_env).ok();

        providers.push(ResolvedProvider {
            name: name.clone(),
            kind: settings.kind.clone(),
            base_url: settings.base_url.clone(),
            api_key,
        });
    }

    Ok(ResolvedProviderConfig {
        default_provider: model.default_provider.clone(),
        default_model: model.default_model.clone(),
        router_strategy: config.router.strategy.clone(),
        providers,
    })
}

/// Resolve tool configuration.
fn resolve_tool_config(config: &ToolConfig) -> ResolvedToolConfig {
    ResolvedToolConfig {
        rust_wasm_modules: config.registry.rust_wasm_modules,
        mcp_tools: config.registry.mcp_tools,
        workflow_tools: config.registry.workflow_tools,
        max_parallel_tools: config.execution.max_parallel_tools,
        default_timeout_ms: config.execution.default_timeout_ms,
    }
}

/// Resolve permission configuration.
fn resolve_permission_config(config: &PermissionConfig) -> ResolvedPermissionConfig {
    ResolvedPermissionConfig {
        allow_read: config.filesystem.allow_read,
        allow_write: config.filesystem.allow_write,
        allow_delete: config.filesystem.allow_delete,
        require_confirmation_for_write: config.filesystem.require_confirmation_for_write,
        require_confirmation_for_delete: config.filesystem.require_confirmation_for_delete,
    }
}

/// Build shell gate configuration.
fn build_shell_gate_config(
    config: &crate::views::ShellPermission,
    workspace_root: &Path,
) -> ShellGateConfig {
    ShellGateConfig {
        enabled: config.enabled,
        require_confirmation: config.require_confirmation,
        workspace_root: workspace_root.to_path_buf(),
        allowlist: config.allowlist.clone(),
        denylist: config.denylist.clone(),
        allow_read_outside_workspace: config.scope.allow_read_outside_workspace,
        allow_write_outside_workspace: config.scope.allow_write_outside_workspace,
        allow_delete_workspace_root: config.scope.allow_delete_workspace_root,
        allow_delete_outside_workspace: config.scope.allow_delete_outside_workspace,
        follow_symlink_outside_workspace: config.scope.follow_symlink_outside_workspace,
        allow_redirection_outside_workspace: config.scope.allow_redirection_outside_workspace,
    }
}

/// Build logging configuration.
fn build_logging_config(
    config: &crate::views::LoggingMeta,
    workspace_root: &Path,
) -> LoggingConfig {
    let level = config.level.parse::<LogLevel>().unwrap_or(LogLevel::Info);

    let log_dir = workspace_root.join(".alius/memory/logs");

    LoggingConfig {
        enabled: config.enabled,
        level,
        redact_secrets: config.redact_secrets,
        flush_error_immediately: config.flush_error_immediately,
        log_dir,
    }
}

/// Build session configuration.
fn build_session_config(
    config: &crate::views::SessionMeta,
    workspace_root: &Path,
) -> SessionConfig {
    let session_path = workspace_root.join(".alius/memory/communications/sessions");

    SessionConfig {
        persist_messages: config.persist_messages,
        persist_events: config.persist_events,
        session_path,
    }
}

/// Resolve soul configuration.
fn resolve_soul_config(config: &SoulConfig) -> ResolvedSoulConfig {
    // Generate system prompt from agent description
    let system_prompt = if config.agent.description.is_empty() {
        format!("You are {}.", config.agent.name)
    } else {
        format!(
            "You are {}, a {}.",
            config.agent.name, config.agent.description
        )
    };

    ResolvedSoulConfig {
        name: config.agent.name.clone(),
        description: config.agent.description.clone(),
        system_prompt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_project_structure(project_dir: &Path) {
        let config_dir = project_dir.join(".alius/config");
        std::fs::create_dir_all(&config_dir).unwrap();

        let config_content = r#"
[project]
name = "test-project"
version = 1

[runtime]
default_mode = "plan"
tui_workspace = true
legacy_repl_env = "ALIUS_LEGACY_REPL"
auto_review = false

[model]
default_provider = "openai"
default_model = "gpt-4o"
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
"#;
        std::fs::write(config_dir.join("config.toml"), config_content).unwrap();

        let providers_content = r#"
[router]
strategy = "tiered"
default_tier = "medium"
fallback_tier = "medium"

[tiers.light]
description = "Fast tasks"
provider = "openai"
model = ""

[tiers.medium]
description = "Default work"
provider = "openai"
model = ""

[tiers.high]
description = "Complex tasks"
provider = "openai"
model = ""

[providers.openai]
enabled = true
kind = "openai"
base_url = "https://api.openai.com/v1"
api_key_env = "OPENAI_API_KEY"
"#;
        std::fs::write(config_dir.join("providers.toml"), providers_content).unwrap();

        let tools_content = r#"
[registry]
rust_wasm_modules = true
mcp_tools = false
workflow_tools = false

[execution]
default_timeout_ms = 120000
max_parallel_tools = 4
persist_tool_trace = true

[confirmation]
read_only_tools = "auto"
write_tools = "ask"
shell_tools = "ask"
network_tools = "ask"
remote_a2a_tools = "deny"

[mcp]
config = ".alius/config/mcp.json"
load_on_workspace_start = false
register_as_tools = false

[plugin]
load_on_workspace_start = false
register_as_tools = false

[workflow]
load_project_workflows = false
register_as_tools = false
"#;
        std::fs::write(config_dir.join("tools.toml"), tools_content).unwrap();

        let permissions_content = r#"
[filesystem]
workspace_root = "."
allow_read = true
allow_write = true
allow_delete = false
require_confirmation_for_write = true
require_confirmation_for_delete = true

[shell]
enabled = true
require_confirmation = true
workspace_scoped = true
deny_unknown_scope = true
require_confirmation_for_outside_workspace = true
allowlist = []
denylist = ["rm -rf /", "rm -rf ~"]

[shell.scope]
allow_read_outside_workspace = false
allow_write_outside_workspace = false
allow_delete_workspace_root = false
allow_delete_outside_workspace = false
follow_symlink_outside_workspace = false
allow_redirection_outside_workspace = false
allow_shell_eval_without_inspection = false

[network]
enabled = true
require_confirmation = true
allowlist = []
denylist = []

[memory]
allow_read = true
allow_write = true
allow_semantic_index_rebuild = true

[project_documents]
allow_update = true
require_history_entry = true
root = ".alius/workspace"

[remote_a2a]
enabled = false
allow_filesystem = false
allow_shell = false
allow_network = false
allowed_tools = []
"#;
        std::fs::write(config_dir.join("permissions.toml"), permissions_content).unwrap();

        let protocol_content = r#"
[protocol]
major = 1
minor = 0
trace_enabled = true
event_sequence_enabled = true

[local_rust]
enabled = true
transport = "in-process"
default_origin = "LocalTui"

[json_rpc]
enabled = false
transport = "stdio-or-socket"
socket_path = ".alius/run/alius.sock"
method_prefix = "alius"

[ide_rpc]
enabled = false
transport = "lsp-like"
workspace_scoped_filesystem = true

[a2a]
enabled = false
server_enabled = false
client_enabled = false
agent_card_source = ".alius/config/soul.toml"
default_remote_capability = "minimal"

[ffi]
enabled = false
core = "lite"
event_delivery = "poll"

[events]
buffer_size = 1024
persist = true
allow_resume = true
visibility_default = "ProductVisible"

[commands]
approve_tool = true
reject_tool = true
answer_question = true
select_option = true
update_plan = true
cancel_run = true
pause_run = false
resume_run = true
"#;
        std::fs::write(config_dir.join("protocol.toml"), protocol_content).unwrap();

        let soul_content = r#"
[agent]
name = "Test Agent"
description = "A test agent for development"
version = "1.0.0"

[agent_card]
documentation_url = ""
icon_url = ""
export_path = ".well-known/agent-card.json"

[[supported_interfaces]]
url = ""
protocol_binding = "HTTP+JSON"
protocol_version = "1.0"

[provider]
organization = "Test Org"
url = ""

[capabilities]
streaming = true
push_notifications = false
extended_agent_card = false

[interaction]
default_input_modes = ["text/plain", "application/json"]
default_output_modes = ["text/plain", "application/json"]
"#;
        std::fs::write(config_dir.join("soul.toml"), soul_content).unwrap();
    }

    #[test]
    fn test_load_project_config() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        create_project_structure(&project_dir);

        let snapshot = load_project_config(&project_dir).unwrap();

        assert_eq!(snapshot.project.name, "test-project");
        assert_eq!(snapshot.model.default_provider, "openai");
        assert_eq!(snapshot.model.default_model, "gpt-4o");
        assert!(snapshot.logging.enabled);
        assert_eq!(snapshot.providers.router.strategy, "tiered");
        assert!(snapshot.tools.registry.rust_wasm_modules);
        assert!(snapshot.permissions.filesystem.allow_read);
        assert!(snapshot.protocol.local_rust.enabled);
        assert_eq!(snapshot.soul.agent.name, "Test Agent");
    }

    #[test]
    fn test_build_runtime_config() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();

        create_project_structure(&project_dir);

        let snapshot = load_project_config(&project_dir).unwrap();
        let runtime = build_runtime_config(&snapshot, &project_dir).unwrap();

        assert_eq!(runtime.provider.default_provider, "openai");
        assert_eq!(runtime.provider.default_model, "gpt-4o");
        assert!(runtime.tools.rust_wasm_modules);
        assert!(runtime.permissions.allow_read);
        assert!(runtime.shell_gate.enabled);
        assert!(runtime.logging.enabled);
        assert!(runtime.session.persist_messages);
        assert_eq!(runtime.soul.name, "Test Agent");
    }

    #[test]
    fn test_find_project_root() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        let nested_dir = project_dir.join("src").join("components");

        std::fs::create_dir_all(project_dir.join(".alius/config")).unwrap();
        std::fs::create_dir_all(&nested_dir).unwrap();

        let root = find_project_root(&nested_dir).unwrap();
        assert_eq!(root, project_dir);
    }

    #[test]
    fn test_find_project_root_not_found() {
        let tmp = TempDir::new().unwrap();
        let nested_dir = tmp.path().join("src").join("components");
        std::fs::create_dir_all(&nested_dir).unwrap();

        let result = find_project_root(&nested_dir);
        assert!(result.is_none());
    }

    #[test]
    fn test_load_with_defaults() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(project_dir.join(".alius/config")).unwrap();

        // Only create config.toml, others should use defaults
        let config_content = r#"
[project]
name = "minimal-project"
version = 1

[runtime]
default_mode = "chat"
tui_workspace = false
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
level = "debug"
redact_secrets = true
flush_error_immediately = true

[compat]
read_legacy_project_config = true
read_legacy_mcp_config = true
read_legacy_project_memory = true
read_legacy_design_docs = true
"#;
        std::fs::write(
            project_dir.join(".alius/config/config.toml"),
            config_content,
        )
        .unwrap();

        let snapshot = load_project_config(&project_dir).unwrap();

        // Should use defaults for missing files
        assert_eq!(snapshot.project.name, "minimal-project");
        assert_eq!(snapshot.providers.router.strategy, "tiered");
        assert!(snapshot.tools.registry.rust_wasm_modules);
        assert!(snapshot.permissions.shell.enabled);
    }
}
