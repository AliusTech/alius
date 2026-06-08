//! Loader for config.toml.

use crate::error::ConfigResult;
use crate::views::{CompatMeta, LoggingMeta, ModelMeta, ProjectMeta, RuntimeMeta, SessionMeta};
use std::path::Path;

/// Load config.toml from the given path.
pub fn load_config(path: &Path) -> ConfigResult<ConfigToml> {
    super::load_toml(path)
}

/// config.toml structure.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ConfigToml {
    /// Project metadata.
    pub project: ProjectMeta,
    /// Runtime settings.
    pub runtime: RuntimeMeta,
    /// Model settings.
    pub model: ModelMeta,
    /// Session settings.
    pub session: SessionMeta,
    /// Logging settings.
    pub logging: LoggingMeta,
    /// Compatibility settings.
    pub compat: CompatMeta,
}

impl Default for ConfigToml {
    fn default() -> Self {
        Self {
            project: ProjectMeta {
                name: String::new(),
                version: 1,
            },
            runtime: RuntimeMeta {
                default_mode: "plan".to_string(),
                tui_workspace: true,
                legacy_repl_env: "ALIUS_LEGACY_REPL".to_string(),
                auto_review: false,
            },
            model: ModelMeta {
                default_provider: String::new(),
                default_model: String::new(),
                router_profile: "standard".to_string(),
            },
            session: SessionMeta {
                persist_messages: true,
                persist_events: true,
            },
            logging: LoggingMeta {
                enabled: true,
                level: "info".to_string(),
                redact_secrets: true,
                flush_error_immediately: true,
            },
            compat: CompatMeta {
                read_legacy_project_config: true,
                read_legacy_mcp_config: true,
                read_legacy_project_memory: true,
                read_legacy_design_docs: true,
            },
        }
    }
}
