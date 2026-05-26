use crate::error::Result;
use crate::error::AliusError;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

const DEFAULT_CONFIG: &str = include_str!("../config/default.toml");

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub llm: LlmSettings,
    pub agent: AgentSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub provider: String,
    pub model: String,
    pub api_key_env: String,
    #[serde(default)]
    pub base_url: Option<String>,
}

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
        std::env::var(&self.llm.api_key_env).map_err(|_| {
            AliusError::MissingConfig(format!(
                "Environment variable {} not set",
                self.llm.api_key_env
            ))
        })
    }
}