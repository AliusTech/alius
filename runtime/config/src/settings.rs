//! Settings configuration structures and defaults.
//!
//! This module defines the configuration hierarchy for the Alius application:
//! - `Settings`: Root configuration containing LLM, agent, and soul settings
//! - `LlmSettings`: LLM provider configuration (provider type, model, API key)
//! - `AgentSettings`: Agent behavior configuration (retries, timeout)
//! - `SoulSettings`: Agent persona configuration (role)
//! - `ConfigPaths`: File paths for configuration loading
//!
//! Configuration is loaded in layers (later layers override earlier ones):
//!   1. Embedded default config (compiled into the binary)
//!   2. User config at ~/.alius/config.toml
//!   3. Project config at ./.alius/config.toml
//!   4. Environment variables with `ALIUS_` prefix

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use protocol_interface::{ProviderMode, ProviderType, SoulRole};

/// Embedded default configuration file content, compiled into the binary.
const DEFAULT_CONFIG: &str = include_str!("default.toml");

/// Main application settings, deserialized from TOML configuration.
///
/// This is the root configuration structure that holds all settings
/// for the Alius application. It can be serialized/deserialized to/from TOML.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Settings {
    /// LLM provider settings (provider type, model, API key, base URL).
    pub llm: LlmSettings,
    /// Agent behavior settings (retries, timeout).
    pub agent: AgentSettings,
    /// Soul role settings (agent persona).
    pub soul: SoulSettings,
    /// UI settings (locale).
    #[serde(default)]
    pub ui: UiSettings,
}

impl Settings {
    /// Load settings using the default configuration resolution chain.
    ///
    /// Layers (in order of precedence, lowest to highest):
    ///   1. Embedded default config (compiled into the binary)
    ///   2. User config at ~/.alius/config.toml (if it exists)
    ///   3. Project config at ./.alius/config.toml (searched upward from cwd)
    ///   4. Environment variable overrides (ALIUS_ prefix, __ separator)
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder().add_source(config::File::from_str(
            DEFAULT_CONFIG,
            config::FileFormat::Toml,
        ));

        let user_config_path = get_user_config_path();
        if user_config_path.exists() {
            builder =
                builder.add_source(config::File::from(user_config_path.as_path()).required(false));
        }

        if let Some(project_config_path) = find_project_config() {
            builder = builder.add_source(config::File::from(project_config_path).required(false));
        }

        builder = builder.add_source(config::Environment::with_prefix("ALIUS").separator("__"));

        let cfg = builder
            .build()
            .map_err(|e| anyhow::anyhow!("Config build error: {}", e))?;

        cfg.try_deserialize()
            .map_err(|e| anyhow::anyhow!("Config deserialize error: {}", e))
    }

    /// Load settings from a specific configuration file path.
    pub fn load_from_path<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let builder = config::Config::builder()
            .add_source(config::File::from_str(
                DEFAULT_CONFIG,
                config::FileFormat::Toml,
            ))
            .add_source(config::File::from(path.as_ref()).required(true))
            .add_source(config::Environment::with_prefix("ALIUS").separator("__"));

        let cfg = builder
            .build()
            .map_err(|e| anyhow::anyhow!("Config build error: {}", e))?;

        cfg.try_deserialize()
            .map_err(|e| anyhow::anyhow!("Config deserialize error: {}", e))
    }

    /// Save the current settings to the user configuration file (~/.alius/config.toml).
    pub fn save_to_user_config(&self) -> anyhow::Result<()> {
        let config_path = ensure_config_dir()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        std::fs::write(&config_path, content)
            .map_err(|e| anyhow::anyhow!("Failed to write config: {}", e))?;
        Ok(())
    }

    /// Save the current settings to the project configuration file (./.alius/config.toml).
    pub fn save_to_project_config(&self) -> anyhow::Result<()> {
        let config_path = ensure_project_config_dir()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| anyhow::anyhow!("Failed to serialize config: {}", e))?;
        std::fs::write(&config_path, content)
            .map_err(|e| anyhow::anyhow!("Failed to write project config: {}", e))?;
        Ok(())
    }

    /// Resolve the API key from configuration or environment variable.
    ///
    /// Resolution order:
    ///   1. Direct `api_key` field (if non-empty)
    ///   2. Environment variable named by `api_key_env`
    pub fn api_key(&self) -> anyhow::Result<String> {
        if let Some(key) = &self.llm.api_key {
            if !key.is_empty() {
                return Ok(key.clone());
            }
        }
        if let Some(env_var) = &self.llm.api_key_env {
            return std::env::var(env_var).map_err(|_| {
                anyhow::anyhow!(
                    "API key not set. Set {} env var or configure in /config",
                    env_var
                )
            });
        }
        Err(anyhow::anyhow!("No API key configured"))
    }

    /// Resolve the base URL for the LLM API endpoint.
    pub fn base_url(&self) -> String {
        if let Some(url) = &self.llm.base_url {
            if !url.is_empty() {
                return url.clone();
            }
        }
        self.llm.get_base_url()
    }

    /// Get the effective base URL (alias for `base_url()`).
    pub fn effective_base_url(&self) -> String {
        self.base_url()
    }

    /// Return the required configuration fields that are still missing for chat.
    pub fn missing_chat_requirements(&self) -> Vec<String> {
        let mut missing = Vec::new();

        if self.llm.model.trim().is_empty() {
            missing.push("model".to_string());
        }

        if self.soul.role.as_str().trim().is_empty() {
            missing.push("soul".to_string());
        }

        if self.llm.get_api_key().is_none() {
            missing.push("api_key".to_string());
        }

        missing
    }

    /// Check whether Alius has enough configuration to start a chat request.
    pub fn is_ready_for_chat(&self) -> bool {
        self.missing_chat_requirements().is_empty()
    }
}

/// Get the path to the user configuration file (~/.alius/config.toml).
fn get_user_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "~".to_string());
    PathBuf::from(home).join(".alius").join("config.toml")
}

/// Search upward from cwd for a project-level config at ./.alius/config.toml.
///
/// Walks up the directory tree from the current working directory,
/// looking for a `.alius/config.toml` file. Returns the first match found.
/// Falls back to legacy `alius/config.toml` for existing projects.
fn find_project_config() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from);
    let mut dir = cwd.as_path();
    loop {
        if home.as_deref() == Some(dir) {
            return None;
        }

        for candidate in [
            dir.join(".alius").join("config.toml"),
            dir.join(".alius").join("config").join("config.toml"),
            dir.join("alius").join("config.toml"),
        ] {
            if candidate.exists() {
                return Some(candidate);
            }
        }
        dir = dir.parent()?;
    }
}

/// Ensure the configuration directory exists, creating it if necessary.
fn ensure_config_dir() -> anyhow::Result<PathBuf> {
    let config_path = get_user_config_path();
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| anyhow::anyhow!("Failed to create config dir: {}", e))?;
        }
    }
    Ok(config_path)
}

/// Ensure the project configuration directory exists, creating it if necessary.
fn ensure_project_config_dir() -> anyhow::Result<PathBuf> {
    let alius_dir = project_alius_dir_for_write()
        .map_err(|e| anyhow::anyhow!("Failed to resolve current directory: {}", e))?;
    let config_dir = alius_dir.join("config");
    std::fs::create_dir_all(&config_dir)
        .map_err(|e| anyhow::anyhow!("Failed to create project config dir: {}", e))?;
    for memory_dir in [
        alius_dir.join("memory"),
        alius_dir
            .join("memory")
            .join("communications")
            .join("sessions"),
        alius_dir.join("memory").join("design"),
    ] {
        std::fs::create_dir_all(&memory_dir)
            .map_err(|e| anyhow::anyhow!("Failed to create project memory dir: {}", e))?;
    }

    let mcp_path = config_dir.join("mcp.json");
    if !mcp_path.exists() {
        std::fs::write(&mcp_path, "{\n  \"servers\": {}\n}\n")
            .map_err(|e| anyhow::anyhow!("Failed to write project MCP config: {}", e))?;
    }
    Ok(config_dir.join("config.toml"))
}

pub fn project_alius_dir_for_write() -> std::io::Result<PathBuf> {
    let cwd = std::env::current_dir()?;
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .ok()
        .map(PathBuf::from);
    let mut dir = cwd.as_path();
    loop {
        if home.as_deref() == Some(dir) {
            return Ok(cwd.join(".alius"));
        }

        let candidate = dir.join(".alius");
        if candidate.exists() {
            return Ok(candidate);
        }

        match dir.parent() {
            Some(parent) => dir = parent,
            None => return Ok(cwd.join(".alius")),
        }
    }
}

/// LLM provider configuration settings.
///
/// Defines which LLM provider to use, the model identifier, and how to authenticate.
/// Supports direct API key configuration or reading from environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    /// The LLM provider type (OpenAI, Anthropic, Google, BigModel, or Custom).
    pub provider: ProviderType,
    /// The provider mode (general, coding, native, etc).
    #[serde(default)]
    pub provider_mode: Option<ProviderMode>,
    /// The model identifier (e.g., "gpt-4o", "glm-5.1").
    pub model: String,
    /// Direct API key value. Takes precedence over `api_key_env` if set.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of the environment variable containing the API key.
    /// Used as a fallback when `api_key` is not set directly.
    #[serde(default)]
    pub api_key_env: Option<String>,
    /// Custom base URL for the LLM API endpoint.
    /// If not set, the default URL for the provider is used.
    #[serde(default)]
    pub base_url: Option<String>,
    /// Model used for the /review command (dual-model mechanism).
    /// If not set, falls back to the main model.
    #[serde(default)]
    pub review_model: Option<String>,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "gpt-4o-mini".to_string(),
            api_key: None,
            api_key_env: Some("OPENAI_API_KEY".to_string()),
            base_url: None,
            review_model: None,
        }
    }
}

impl LlmSettings {
    /// Resolve the API key from configuration or environment variable.
    ///
    /// Resolution order:
    ///   1. Direct `api_key` field (if set)
    ///   2. Environment variable named by `api_key_env` (if set)
    ///
    /// Returns `None` if neither source provides a valid key.
    pub fn get_api_key(&self) -> Option<String> {
        if let Some(key) = &self.api_key {
            return Some(key.clone());
        }
        if let Some(env_var) = &self.api_key_env {
            return std::env::var(env_var).ok();
        }
        None
    }

    /// Get the base URL for the LLM API endpoint.
    ///
    /// Returns the custom `base_url` if configured, otherwise returns the
    /// default URL for the configured provider.
    pub fn get_base_url(&self) -> String {
        if let Some(url) = &self.base_url {
            return url.clone();
        }
        match self.provider {
            ProviderType::Openai => "https://api.openai.com/v1".to_string(),
            ProviderType::BigModel => match self.provider_mode {
                Some(ProviderMode::Coding) => {
                    "https://open.bigmodel.cn/api/coding/paas/v4".to_string()
                }
                _ => "https://open.bigmodel.cn/api/paas/v4".to_string(),
            },
            ProviderType::Anthropic => match self.provider_mode {
                Some(ProviderMode::OpenAICompatible) => "https://api.anthropic.com/v1".to_string(),
                _ => "https://api.anthropic.com".to_string(),
            },
            ProviderType::Google => match self.provider_mode {
                Some(ProviderMode::OpenAICompatible) => {
                    "https://generativelanguage.googleapis.com/v1beta/openai".to_string()
                }
                _ => "https://generativelanguage.googleapis.com/v1beta".to_string(),
            },
            ProviderType::Custom => "http://localhost:8080/v1".to_string(),
        }
    }
}

/// Agent behavior configuration.
///
/// Controls retry behavior and request timeouts for LLM API calls.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Maximum number of retry attempts for failed LLM API calls.
    pub max_retries: u32,
    /// Timeout in seconds for LLM API requests.
    pub timeout_seconds: u64,
}

impl Default for AgentSettings {
    fn default() -> Self {
        Self {
            max_retries: 3,
            timeout_seconds: 60,
        }
    }
}

/// Soul role configuration, defining the agent's persona.
///
/// The soul role determines the agent's behavior, expertise, and response style.
/// See `soul::system_prompt_for_role()` for the mapping of roles to system prompts.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SoulSettings {
    /// The soul role (e.g., "Frontend Engineer", "Backend Developer").
    pub role: SoulRole,
}

/// UI configuration settings.
///
/// Controls the display language and other UI preferences.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UiSettings {
    /// Locale for the UI (e.g., "en", "zh-CN", "ja").
    #[serde(default = "default_locale")]
    pub locale: String,
}

fn default_locale() -> String {
    "en".to_string()
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            locale: default_locale(),
        }
    }
}

/// Configuration file paths for loading and saving settings.
///
/// Manages the paths to the default config (embedded in binary) and
/// the user config (~/.alius/config.toml).
pub struct ConfigPaths {
    /// Path to the default configuration file (embedded in the binary).
    pub default_config: PathBuf,
    /// Path to the user configuration file (~/.alius/config.toml).
    pub user_config: PathBuf,
}

impl ConfigPaths {
    /// Create a new ConfigPaths with default locations.
    ///
    /// Uses `HOME` environment variable to locate the user config directory.
    /// Falls back to "." if `HOME` is not set.
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            default_config: PathBuf::from("config/default.toml"),
            user_config: PathBuf::from(format!("{}/.alius/config.toml", home)),
        }
    }

    /// Get the path to the Alius configuration directory (~/.alius).
    pub fn get_alius_dir() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(format!("{}/.alius", home))
    }
}

impl Default for ConfigPaths {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn parse_toml_only() -> Settings {
        let cfg = config::Config::builder()
            .add_source(config::File::from_str(
                DEFAULT_CONFIG,
                config::FileFormat::Toml,
            ))
            .build()
            .expect("embedded default config should parse");
        cfg.try_deserialize()
            .expect("embedded default config should deserialize")
    }

    #[test]
    fn test_toml_default_empty_model() {
        let settings = parse_toml_only();
        assert!(
            settings.llm.model.is_empty(),
            "embedded TOML default model should be empty"
        );
    }

    #[test]
    fn test_toml_default_no_api_key_env() {
        let settings = parse_toml_only();
        assert!(
            settings.llm.api_key_env.is_none(),
            "embedded TOML default api_key_env should be None, got {:?}",
            settings.llm.api_key_env
        );
    }

    #[test]
    fn test_toml_default_no_api_key() {
        let settings = parse_toml_only();
        assert!(settings.llm.api_key.is_none());
    }

    #[test]
    fn test_toml_default_empty_soul() {
        let settings = parse_toml_only();
        assert!(
            settings.soul.role.as_str().is_empty(),
            "embedded TOML default soul should be empty"
        );
    }

    #[test]
    fn test_default_toml_is_not_ready_for_chat() {
        let settings = parse_toml_only();
        assert!(!settings.is_ready_for_chat());
        assert!(settings
            .missing_chat_requirements()
            .contains(&"model".to_string()));
        assert!(settings
            .missing_chat_requirements()
            .contains(&"soul".to_string()));
    }

    #[test]
    fn test_ready_for_chat_requires_model_and_soul() {
        let mut settings = parse_toml_only();
        settings.llm.api_key = Some("test-key".to_string());

        settings.llm.model = "glm-5.1".to_string();
        assert!(!settings.is_ready_for_chat());
        assert_eq!(
            settings.missing_chat_requirements(),
            vec!["soul".to_string()]
        );

        settings.soul.role = protocol_interface::SoulRole::new("Backend Developer".to_string());
        assert!(settings.is_ready_for_chat());
        assert!(settings.missing_chat_requirements().is_empty());
    }

    #[test]
    fn test_api_key_resolution_direct() {
        let settings = LlmSettings {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "gpt-4o".to_string(),
            api_key: Some("direct-key".to_string()),
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        assert_eq!(settings.get_api_key(), Some("direct-key".to_string()));
    }

    #[test]
    fn test_api_key_resolution_none() {
        let settings = LlmSettings {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "gpt-4o".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        assert_eq!(settings.get_api_key(), None);
    }

    #[test]
    fn test_base_url_defaults() {
        let s = LlmSettings {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "test".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        assert_eq!(s.get_base_url(), "https://api.openai.com/v1");

        let s = LlmSettings {
            provider: ProviderType::BigModel,
            provider_mode: None,
            model: "test".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        assert_eq!(s.get_base_url(), "https://open.bigmodel.cn/api/paas/v4");
    }

    #[test]
    fn test_base_url_bigmodel_coding() {
        let s = LlmSettings {
            provider: ProviderType::BigModel,
            provider_mode: Some(ProviderMode::Coding),
            model: "test".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        assert_eq!(
            s.get_base_url(),
            "https://open.bigmodel.cn/api/coding/paas/v4"
        );
    }

    #[test]
    fn test_base_url_anthropic_native() {
        let s = LlmSettings {
            provider: ProviderType::Anthropic,
            provider_mode: None,
            model: "test".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        assert_eq!(s.get_base_url(), "https://api.anthropic.com");
    }

    #[test]
    fn test_base_url_custom() {
        let s = LlmSettings {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "test".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: Some("http://localhost:1234/v1".to_string()),
            review_model: None,
        };
        assert_eq!(s.get_base_url(), "http://localhost:1234/v1");
    }

    #[test]
    fn test_rust_default_vs_toml_default() {
        let rust_default = LlmSettings::default();
        assert_eq!(rust_default.model, "gpt-4o-mini");
        assert_eq!(rust_default.api_key_env, Some("OPENAI_API_KEY".to_string()));

        let toml_default = parse_toml_only();
        assert!(toml_default.llm.model.is_empty());
        assert!(toml_default.llm.api_key_env.is_none());
    }

    #[test]
    fn test_settings_serialization_roundtrip() {
        let settings = Settings {
            llm: LlmSettings {
                provider: ProviderType::Openai,
                provider_mode: None,
                model: "gpt-4o".to_string(),
                api_key: Some("test-key".to_string()),
                api_key_env: None,
                base_url: Some("https://api.openai.com/v1".to_string()),
                review_model: None,
            },
            agent: AgentSettings::default(),
            soul: SoulSettings {
                role: protocol_interface::SoulRole::new("Backend Developer".to_string()),
            },
            ui: UiSettings::default(),
        };
        let toml_str = toml::to_string_pretty(&settings).expect("should serialize");
        assert!(toml_str.contains("gpt-4o"));
        assert!(toml_str.contains("test-key"));
    }

    #[test]
    fn test_find_project_config_prefers_dot_alius() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        let nested_dir = project_dir.join("src");
        std::fs::create_dir_all(project_dir.join(".alius")).unwrap();
        std::fs::create_dir_all(project_dir.join("alius")).unwrap();
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(project_dir.join(".alius").join("config.toml"), "").unwrap();
        std::fs::write(project_dir.join("alius").join("config.toml"), "").unwrap();

        std::env::set_current_dir(&nested_dir).unwrap();
        let found = find_project_config();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert_eq!(
            found.and_then(|path| std::fs::canonicalize(path).ok()),
            std::fs::canonicalize(project_dir.join(".alius").join("config.toml")).ok()
        );
    }

    #[test]
    fn test_find_project_config_reads_legacy_alius() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        let nested_dir = project_dir.join("src");
        std::fs::create_dir_all(project_dir.join("alius")).unwrap();
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(project_dir.join("alius").join("config.toml"), "").unwrap();

        std::env::set_current_dir(&nested_dir).unwrap();
        let found = find_project_config();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert_eq!(
            found.and_then(|path| std::fs::canonicalize(path).ok()),
            std::fs::canonicalize(project_dir.join("alius").join("config.toml")).ok()
        );
    }

    #[test]
    fn test_save_to_project_config_writes_dot_alius_config() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        Settings::default().save_to_project_config().unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(project_dir
            .join(".alius")
            .join("config")
            .join("config.toml")
            .exists());
        assert!(project_dir
            .join(".alius")
            .join("config")
            .join("mcp.json")
            .exists());
        assert!(project_dir.join(".alius").join("memory").exists());
        assert!(project_dir
            .join(".alius")
            .join("memory")
            .join("communications")
            .join("sessions")
            .exists());
        assert!(project_dir
            .join(".alius")
            .join("memory")
            .join("design")
            .exists());
    }
}
