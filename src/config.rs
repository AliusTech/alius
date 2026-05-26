use crate::error::Result;
use crate::error::AliusError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub llm: LlmSettings,
    pub agent: AgentSettings,
    #[serde(default)]
    pub soul: Option<SoulSettings>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulSettings {
    pub role: String,
}

pub const SOUL_ROLES: &[&str] = &[
    "Frontend Engineer",
    "Operations Personnel",
    "Backend Developer",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub provider: String,
    pub model: String,
    #[serde(default)]
    pub api_key: Option<String>,
    pub api_key_env: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

pub const PROVIDERS: &[&str] = &["openai", "anthropic", "google"];

pub const DEFAULT_BASE_URLS: &[(&str, &str)] = &[
    ("openai", "https://api.openai.com/v1"),
    ("anthropic", "https://api.anthropic.com/v1"),
    ("google", "https://generativelanguage.googleapis.com/v1"),
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub max_retries: u32,
    pub timeout_seconds: u64,
}

fn get_user_config_path() -> PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| "~".to_string());
    PathBuf::from(home).join(".alius").join("config.toml")
}

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
    pub fn load() -> Result<Self> {
        // 1. Start with embedded default config
        let mut builder = config::Config::builder()
            .add_source(config::File::from_str(DEFAULT_CONFIG, config::FileFormat::Toml));

        // 2. Try user config at ~/.alius/config.toml
        let user_config_path = get_user_config_path();
        if user_config_path.exists() {
            builder = builder
                .add_source(config::File::from(user_config_path.as_path()).required(false));
        }

        // 3. Environment variables override
        builder = builder
            .add_source(config::Environment::with_prefix("ALIUS").separator("__"));

        let config = builder
            .build()
            .map_err(|e| AliusError::Config(e.to_string()))?;

        config
            .try_deserialize()
            .map_err(|e| AliusError::Config(e.to_string()))
    }

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

    pub fn save_to_user_config(&self) -> Result<()> {
        let config_path = ensure_config_dir()?;
        let content = toml::to_string_pretty(self)
            .map_err(|e| AliusError::Config(format!("Failed to serialize: {}", e)))?;
        std::fs::write(&config_path, content)
            .map_err(|e| AliusError::Config(format!("Failed to write config: {}", e)))?;
        Ok(())
    }

    pub fn user_config_path() -> PathBuf {
        get_user_config_path()
    }

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

    pub fn effective_base_url(&self) -> String {
        self.base_url()
    }
}