//! Configuration layer merging.
//!
//! This module handles merging configuration from multiple sources:
//! - Embedded defaults
//! - User config (~/.alius/config.toml)
//! - Project config (.alius/config/*.toml)
//! - Environment overrides
//! - CLI parameter overrides

use crate::views::{ProjectConfigSnapshot, RuntimeMeta};
use std::collections::HashMap;

/// Embedded default configuration.
#[derive(Debug, Clone)]
pub struct EmbeddedDefaults {
    /// Runtime mode.
    pub default_mode: String,
    /// TUI workspace enabled.
    pub tui_workspace: bool,
    /// Log level.
    pub log_level: String,
}

impl Default for EmbeddedDefaults {
    fn default() -> Self {
        Self {
            default_mode: "plan".to_string(),
            tui_workspace: true,
            log_level: "info".to_string(),
        }
    }
}

/// User-level configuration from ~/.alius/config.toml.
#[derive(Debug, Clone, Default)]
pub struct UserConfig {
    /// Preferred provider.
    pub default_provider: Option<String>,
    /// Preferred model.
    pub default_model: Option<String>,
    /// Preferred soul role.
    pub soul_role: Option<String>,
    /// Locale preference.
    pub locale: Option<String>,
}

/// Environment variable overrides.
#[derive(Debug, Clone, Default)]
pub struct EnvOverrides {
    /// Provider override (ALIUS__LLM__PROVIDER).
    pub provider: Option<String>,
    /// Model override (ALIUS__LLM__MODEL).
    pub model: Option<String>,
    /// API key overrides by provider.
    pub api_keys: HashMap<String, String>,
    /// Mode override (ALIUS__RUNTIME__DEFAULT_MODE).
    pub default_mode: Option<String>,
}

/// CLI parameter overrides.
#[derive(Debug, Clone, Default)]
pub struct CliOverrides {
    /// Provider override.
    pub provider: Option<String>,
    /// Model override.
    pub model: Option<String>,
    /// Mode override.
    pub mode: Option<String>,
}

/// Load environment variable overrides.
pub fn load_env_overrides() -> EnvOverrides {
    let mut overrides = EnvOverrides::default();

    // Provider override
    if let Ok(provider) = std::env::var("ALIUS__LLM__PROVIDER") {
        overrides.provider = Some(provider);
    }

    // Model override
    if let Ok(model) = std::env::var("ALIUS__LLM__MODEL") {
        overrides.model = Some(model);
    }

    // Mode override
    if let Ok(mode) = std::env::var("ALIUS__RUNTIME__DEFAULT_MODE") {
        overrides.default_mode = Some(mode);
    }

    // API key overrides by provider
    for (env_var, provider) in [
        ("ALIUS__OPENAI_API_KEY", "openai"),
        ("ALIUS__ANTHROPIC_API_KEY", "anthropic"),
        ("ALIUS__GOOGLE_API_KEY", "google"),
        ("ALIUS__BIGMODEL_API_KEY", "bigmodel"),
        ("ALIUS__CUSTOM_API_KEY", "custom"),
    ] {
        if let Ok(key) = std::env::var(env_var) {
            overrides.api_keys.insert(provider.to_string(), key);
        }
    }

    overrides
}

/// Merge runtime mode from all layers.
///
/// Priority (highest to lowest):
/// 1. CLI override
/// 2. Environment override
/// 3. Project config
/// 4. User config
/// 5. Embedded defaults
pub fn merge_runtime_mode(
    embedded: &EmbeddedDefaults,
    _user: &UserConfig,
    project: &RuntimeMeta,
    env: &EnvOverrides,
    cli: &CliOverrides,
) -> String {
    cli.mode
        .clone()
        .or_else(|| env.default_mode.clone())
        .or_else(|| {
            if !project.default_mode.is_empty() {
                Some(project.default_mode.clone())
            } else {
                None
            }
        })
        .unwrap_or_else(|| embedded.default_mode.clone())
}

/// Merge provider selection from all layers.
pub fn merge_provider(
    _embedded: &EmbeddedDefaults,
    user: &UserConfig,
    project_provider: &str,
    env: &EnvOverrides,
    cli: &CliOverrides,
) -> String {
    cli.provider
        .clone()
        .or_else(|| env.provider.clone())
        .or_else(|| user.default_provider.clone())
        .or_else(|| {
            if !project_provider.is_empty() {
                Some(project_provider.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "openai".to_string())
}

/// Merge model selection from all layers.
pub fn merge_model(
    _embedded: &EmbeddedDefaults,
    user: &UserConfig,
    project_model: &str,
    env: &EnvOverrides,
    cli: &CliOverrides,
) -> String {
    cli.model
        .clone()
        .or_else(|| env.model.clone())
        .or_else(|| user.default_model.clone())
        .or_else(|| {
            if !project_model.is_empty() {
                Some(project_model.to_string())
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Configuration merge result.
#[derive(Debug, Clone)]
pub struct MergeResult {
    /// Final merged snapshot.
    pub snapshot: ProjectConfigSnapshot,
    /// Sources used for each field.
    pub sources: HashMap<String, ConfigSource>,
}

/// Configuration source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSource {
    /// Embedded defaults.
    Embedded,
    /// User config file.
    UserConfig,
    /// Project config file.
    ProjectConfig,
    /// Environment variable.
    Environment,
    /// CLI parameter.
    CliParam,
}

impl std::fmt::Display for ConfigSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigSource::Embedded => write!(f, "embedded defaults"),
            ConfigSource::UserConfig => write!(f, "user config"),
            ConfigSource::ProjectConfig => write!(f, "project config"),
            ConfigSource::Environment => write!(f, "environment variable"),
            ConfigSource::CliParam => write!(f, "CLI parameter"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_runtime_mode_cli_override() {
        let embedded = EmbeddedDefaults::default();
        let user = UserConfig::default();
        let project = RuntimeMeta {
            default_mode: "chat".to_string(),
            tui_workspace: true,
            legacy_repl_env: "ALIUS_LEGACY_REPL".to_string(),
            auto_review: false,
        };
        let env = EnvOverrides::default();
        let cli = CliOverrides {
            mode: Some("plan".to_string()),
            ..Default::default()
        };

        let result = merge_runtime_mode(&embedded, &user, &project, &env, &cli);
        assert_eq!(result, "plan");
    }

    #[test]
    fn test_merge_runtime_mode_env_override() {
        let embedded = EmbeddedDefaults::default();
        let user = UserConfig::default();
        let project = RuntimeMeta {
            default_mode: "chat".to_string(),
            tui_workspace: true,
            legacy_repl_env: "ALIUS_LEGACY_REPL".to_string(),
            auto_review: false,
        };
        let env = EnvOverrides {
            default_mode: Some("plan".to_string()),
            ..Default::default()
        };
        let cli = CliOverrides::default();

        let result = merge_runtime_mode(&embedded, &user, &project, &env, &cli);
        assert_eq!(result, "plan");
    }

    #[test]
    fn test_merge_runtime_mode_project_fallback() {
        let embedded = EmbeddedDefaults {
            default_mode: "plan".to_string(),
            tui_workspace: true,
            log_level: "info".to_string(),
        };
        let user = UserConfig::default();
        let project = RuntimeMeta {
            default_mode: "chat".to_string(),
            tui_workspace: true,
            legacy_repl_env: "ALIUS_LEGACY_REPL".to_string(),
            auto_review: false,
        };
        let env = EnvOverrides::default();
        let cli = CliOverrides::default();

        let result = merge_runtime_mode(&embedded, &user, &project, &env, &cli);
        assert_eq!(result, "chat");
    }

    #[test]
    fn test_merge_runtime_mode_embedded_fallback() {
        let embedded = EmbeddedDefaults {
            default_mode: "plan".to_string(),
            tui_workspace: true,
            log_level: "info".to_string(),
        };
        let user = UserConfig::default();
        let project = RuntimeMeta {
            default_mode: "".to_string(),
            tui_workspace: true,
            legacy_repl_env: "ALIUS_LEGACY_REPL".to_string(),
            auto_review: false,
        };
        let env = EnvOverrides::default();
        let cli = CliOverrides::default();

        let result = merge_runtime_mode(&embedded, &user, &project, &env, &cli);
        assert_eq!(result, "plan");
    }

    #[test]
    fn test_merge_provider_priority() {
        let embedded = EmbeddedDefaults::default();
        let user = UserConfig {
            default_provider: Some("anthropic".to_string()),
            ..Default::default()
        };

        // CLI override wins
        let cli = CliOverrides {
            provider: Some("google".to_string()),
            ..Default::default()
        };
        let env = EnvOverrides::default();
        let result = merge_provider(&embedded, &user, "openai", &env, &cli);
        assert_eq!(result, "google");

        // Env override wins over user
        let cli = CliOverrides::default();
        let env = EnvOverrides {
            provider: Some("bigmodel".to_string()),
            ..Default::default()
        };
        let result = merge_provider(&embedded, &user, "openai", &env, &cli);
        assert_eq!(result, "bigmodel");

        // User config wins over project
        let env = EnvOverrides::default();
        let result = merge_provider(&embedded, &user, "openai", &env, &cli);
        assert_eq!(result, "anthropic");

        // Project config wins when user has no preference
        let user = UserConfig::default();
        let result = merge_provider(&embedded, &user, "anthropic", &env, &cli);
        assert_eq!(result, "anthropic");

        // Embedded default when nothing is set
        let result = merge_provider(&embedded, &user, "", &env, &cli);
        assert_eq!(result, "openai");
    }

    #[test]
    fn test_load_env_overrides() {
        // Set environment variables
        std::env::set_var("ALIUS__LLM__PROVIDER", "anthropic");
        std::env::set_var("ALIUS__LLM__MODEL", "claude-3");
        std::env::set_var("ALIUS__RUNTIME__DEFAULT_MODE", "chat");
        std::env::set_var("ALIUS__OPENAI_API_KEY", "test-key-123");

        let overrides = load_env_overrides();

        assert_eq!(overrides.provider, Some("anthropic".to_string()));
        assert_eq!(overrides.model, Some("claude-3".to_string()));
        assert_eq!(overrides.default_mode, Some("chat".to_string()));
        assert_eq!(
            overrides.api_keys.get("openai"),
            Some(&"test-key-123".to_string())
        );

        // Clean up
        std::env::remove_var("ALIUS__LLM__PROVIDER");
        std::env::remove_var("ALIUS__LLM__MODEL");
        std::env::remove_var("ALIUS__RUNTIME__DEFAULT_MODE");
        std::env::remove_var("ALIUS__OPENAI_API_KEY");
    }

    #[test]
    fn test_config_source_display() {
        assert_eq!(ConfigSource::Embedded.to_string(), "embedded defaults");
        assert_eq!(ConfigSource::UserConfig.to_string(), "user config");
        assert_eq!(ConfigSource::ProjectConfig.to_string(), "project config");
        assert_eq!(
            ConfigSource::Environment.to_string(),
            "environment variable"
        );
        assert_eq!(ConfigSource::CliParam.to_string(), "CLI parameter");
    }
}
