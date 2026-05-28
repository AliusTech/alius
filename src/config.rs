use crate::error::Result;
use crate::error::AliusError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Embedded default configuration file content, compiled into the binary.
/// This serves as the base configuration layer that can be overridden by
/// user config and environment variables.
const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

/// Main application settings, deserialized from TOML configuration.
///
/// Configuration is loaded in layers (later layers override earlier ones):
///   1. Embedded default config
///   2. User config at ~/.alius/config.toml
///   3. Environment variables with `ALIUS_` prefix
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    /// LLM provider settings (provider type, model, API key, base URL).
    pub llm: LlmSettings,
    /// Agent behavior settings (retries, timeout).
    pub agent: AgentSettings,
    /// Optional soul role settings (agent persona).
    #[serde(default)]
    pub soul: Option<SoulSettings>,
}

/// Soul role configuration, defining the agent's persona and behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulSettings {
    /// The role name (e.g., "Frontend Engineer", "Backend Developer").
    pub role: String,
}

/// Available soul roles that can be assigned to the agent.
/// Each role influences the agent's expertise and response style.
pub const SOUL_ROLES: &[&str] = &[
    "Frontend Engineer",
    "Operations Personnel",
    "Backend Developer",
];

/// LLM provider configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    /// The LLM provider name (e.g., "openai", "anthropic", "google").
    pub provider: String,
    /// The model identifier (e.g., "gpt-4o", "claude-3-5-sonnet").
    pub model: String,
    /// Direct API key value. Takes precedence over `api_key_env` if set.
    #[serde(default)]
    pub api_key: Option<String>,
    /// Name of the environment variable containing the API key.
    /// Used as a fallback when `api_key` is not set directly.
    pub api_key_env: String,
    /// Custom base URL for the LLM API endpoint.
    /// If not set, the default URL for the provider is used.
    #[serde(default)]
    pub base_url: Option<String>,
}

/// Supported LLM provider identifiers.
pub const PROVIDERS: &[&str] = &["openai", "anthropic", "google"];

/// Default base URLs for each supported LLM provider.
/// Used when no custom `base_url` is configured.
pub const DEFAULT_BASE_URLS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("anthropic", "https://api.anthropic.com/v1"),
    ("google", "https://generativelanguage.googleapis.com/v1"),
];

/// Agent behavior configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    /// Maximum number of retry attempts for failed LLM API calls.
    pub max_retries: u32,
    /// Timeout in seconds for LLM API requests.
    pub timeout_seconds: u64,
}

/// Get the path to the user configuration file (~/.alius/config.toml).
///
/// Uses `HOME` on Unix or `USERPROFILE` on Windows. Falls back to "~" if neither is set.
fn get_user_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "~".to_string());
    PathBuf::from(home).join(".alius").join("config.toml")
}

/// Ensure the configuration directory exists, creating it if necessary.
///
/// Returns the path to the configuration file within the directory.
fn ensure_config_dir() -> Result<PathBuf> {
    let config_path = get_user_config_path();
    let config_dir = config_path.parent().unwrap();

    if !config_dir.exists() {
        std::fs::create_dir_all(config_dir)
            .map_err(|e| AliusError::Config(format!("Failed to create config dir: {}", e)))?;
    }

    Ok(config_path)
}

impl Settings {
    /// Load settings using the default configuration resolution chain.
    ///
    /// Layers (in order of precedence, lowest to highest):
    ///   1. Embedded default config (compiled into the binary)
    ///   2. User config at ~/.alius/config.toml (if it exists)
    ///   3. Environment variables with `ALIUS_` prefix (e.g., `ALIUS_LLM__MODEL`)
    pub fn load() -> Result<Self> {
        // Layer 1: Embedded default config
        let mut builder = config::Config::builder()
            .add_source(config::File::from_str(DEFAULT_CONFIG, config::FileFormat::Toml));

        // Layer 2: User config file (optional, only if it exists)
        let user_config_path = get_user_config_path();
        if user_config_path.exists() {
            builder = builder
                .add_source(config::File::from(user_config_path.as_path()).required(false));
        }

        // Layer 3: Environment variable overrides
        builder = builder
            .add_source(config::Environment::with_prefix("ALIUS").separator("__"));

        let config = builder
            .build()
            .map_err(|e| AliusError::Config(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| AliusError::Config(e.to_string()))
    }

    /// Load settings from a specific configuration file path.
    ///
    /// Merges the embedded defaults with the provided file and environment variables.
    /// The specified file is required to exist.
    pub fn load_from_path<P: AsRef<std::path::Path>>(path: P) -> Result<Self> {
        let config_path = path.as_ref();

        let builder = config::Config::builder()
            .add_source(config::File::from_str(DEFAULT_CONFIG, config::FileFormat::Toml))
            .add_source(config::File::from(config_path).required(true))
            .add_source(config::Environment::with_prefix("ALIUS").separator("__"));

        let config = builder
            .build()
            .map_err(|e| AliusError::Config(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| AliusError::Config(e.to_string()))
    }

    /// Save the current settings to the user configuration file (~/.alius/config.toml).
    ///
    /// Creates the configuration directory if it doesn't exist.
    /// Serializes the settings as pretty-printed TOML.
    pub fn save_to_user_config(&self) -> Result<()> {
        let config_path = ensure_config_dir()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| AliusError::Config(format!("Failed to serialize: {}", e)))?;
        std::fs::write(&config_path, content)
            .map_err(|e| AliusError::Config(format!("Failed to write config: {}", e)))?;
        Ok(())
    }

    /// Get the path to the user configuration file.
    pub fn user_config_path() -> PathBuf {
        get_user_config_path()
    }

    /// Resolve the API key from configuration or environment variable.
    ///
    /// Resolution order:
    ///   1. Direct `api_key` field in config (if non-empty)
    ///   2. Environment variable named by `api_key_env`
    ///
    /// Returns an error if neither source provides a valid key.
    pub fn api_key(&self) -> Result<String> {
        // First try direct api_key, then fall back to env var
        if let Some(key) = &self.llm.api_key {
            if !key.is_empty() {
                return Ok(key.clone());
            }
        }
        std::env::var(&self.llm.api_key_env).map_err(|_| {
            AliusError::MissingConfig(format!(
                "API key not set. Set {} env var or configure in /config",
                self.llm.api_key_env
            ))
        })
    }

    /// Resolve the base URL for the LLM API endpoint.
    ///
    /// Returns the custom `base_url` if configured, otherwise returns the
    /// default URL for the configured provider. Falls back to OpenAI's URL
    /// if the provider is unknown.
    pub fn base_url(&self) -> String {
        if let Some(url) = &self.llm.base_url {
            if !url.is_empty() {
                return url.clone();
            }
        }
        // Get default base URL for provider
        for (provider, url) in DEFAULT_BASE_URLS {
            if self.llm.provider == *provider {
                return url.to_string();
            }
        }
        DEFAULT_BASE_URLS[0].1.to_string() // Default to OpenAI
    }

    /// Get the effective base URL (alias for `base_url()`).
    pub fn effective_base_url(&self) -> String {
        self.base_url()
    }
}
