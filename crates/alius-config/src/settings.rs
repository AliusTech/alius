//! Settings configuration

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use alius_protocol::{ProviderType, SoulRole};

/// Main settings structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub llm: LlmSettings,
    pub agent: AgentSettings,
    pub soul: SoulSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            llm: LlmSettings::default(),
            agent: AgentSettings::default(),
            soul: SoulSettings::default(),
        }
    }
}

/// LLM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmSettings {
    pub provider: ProviderType,
    pub model: String,
    pub api_key: Option<String>,
    pub api_key_env: Option<String>,
    pub base_url: Option<String>,
}

impl Default for LlmSettings {
    fn default() -> Self {
        Self {
            provider: ProviderType::Openai,
            model: "gpt-4o-mini".to_string(),
            api_key: None,
            api_key_env: Some("OPENAI_API_KEY".to_string()),
            base_url: None,
        }
    }
}

impl LlmSettings {
    /// Get the effective API key
    pub fn get_api_key(&self) -> Option<String> {
        if let Some(key) = &self.api_key {
            return Some(key.clone());
        }
        if let Some(env_var) = &self.api_key_env {
            return std::env::var(env_var).ok();
        }
        None
    }

    /// Get the base URL for the provider
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

/// Agent configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSettings {
    pub max_retries: u32,
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

/// Soul configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulSettings {
    pub role: SoulRole,
}

impl Default for SoulSettings {
    fn default() -> Self {
        Self {
            role: SoulRole::default(),
        }
    }
}

/// Configuration file paths
pub struct ConfigPaths {
    pub default_config: PathBuf,
    pub user_config: PathBuf,
}

impl ConfigPaths {
    pub fn new() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        Self {
            default_config: PathBuf::from("config/default.toml"),
            user_config: PathBuf::from(format!("{}/.alius/config.toml", home)),
        }
    }

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