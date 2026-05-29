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
//!   3. Environment variables with `ALIUS_` prefix

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use alius_protocol::{ProviderType, SoulRole};

/// Embedded default configuration file content, compiled into the binary.
const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

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
}

impl Settings {
    /// Load settings using the default configuration resolution chain.
    ///
    /// Layers (in order of precedence, lowest to highest):
    ///   1. Embedded default config (compiled into the binary)
    ///   2. User config at ~/.alius/config.toml (if it exists)
    ///   3. Project config at ./alius/config.toml (searched upward from cwd)
    ///   4. Environment variable overrides (ALIUS_ prefix, __ separator)
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder()
            .add_source(config::File::from_str(DEFAULT_CONFIG, config::FileFormat::Toml));

        let user_config_path = get_user_config_path();
        if user_config_path.exists() {
            builder = builder
                .add_source(config::File::from(user_config_path.as_path()).required(false));
        }

        if let Some(project_config_path) = find_project_config() {
            builder = builder
                .add_source(config::File::from(project_config_path).required(false));
        }

        builder = builder
            .add_source(config::Environment::with_prefix("ALIUS").separator("__"));

        let cfg = builder
            .build()
            .map_err(|e| anyhow::anyhow!("Config build error: {}", e))?;

        cfg.try_deserialize()
            .map_err(|e| anyhow::anyhow!("Config deserialize error: {}", e))
    }

    /// Load settings from a specific configuration file path.
    pub fn load_from_path<P: AsRef<std::path::Path>>(path: P) -> anyhow::Result<Self> {
        let builder = config::Config::builder()
            .add_source(config::File::from_str(DEFAULT_CONFIG, config::FileFormat::Toml))
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
            return std::env::var(env_var)
                .map_err(|_| anyhow::anyhow!(
                    "API key not set. Set {} env var or configure in /config",
                    env_var
                ));
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
}

/// Get the path to the user configuration file (~/.alius/config.toml).
fn get_user_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "~".to_string());
    PathBuf::from(home).join(".alius").join("config.toml")
}

/// Search upward from cwd for a project-level config at ./alius/config.toml.
///
/// Walks up the directory tree from the current working directory,
/// looking for an `alius/config.toml` file. Returns the first match found.
fn find_project_config() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("alius").join("config.toml");
        if candidate.exists() {
            return Some(candidate);
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

/// LLM provider configuration settings.
///
/// Defines which LLM provider to use, the model identifier, and how to authenticate.
/// Supports direct API key configuration or reading from environment variables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    /// The LLM provider type (OpenAI, Anthropic, Google, or Custom).
    pub provider: ProviderType,
    /// The model identifier (e.g., "gpt-4o", "claude-3-5-sonnet").
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
            ProviderType::Anthropic => "https://api.anthropic.com/v1".to_string(),
            ProviderType::Google => "https://generativelanguage.googleapis.com/v1".to_string(),
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
