//! Loader for providers.toml.

use crate::error::ConfigResult;
use crate::views::{
    ModelLibraryConfig, ProviderConfig, ProviderSettings, RouterConfig, TierConfig, TierConfigs,
};
use std::collections::HashMap;
use std::path::Path;

/// Load providers.toml from the given path.
pub fn load_providers(path: &Path) -> ConfigResult<ProviderConfig> {
    let raw: ProvidersToml = super::load_toml(path)?;
    Ok(raw.into())
}

/// Save providers.toml to the given path.
pub fn save_providers(path: &Path, config: &ProviderConfig) -> ConfigResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| crate::error::ConfigError::io(parent, e))?;
    }
    let content = toml::to_string_pretty(config)
        .map_err(|e| crate::error::ConfigError::serialize(path, e))?;
    std::fs::write(path, content).map_err(|e| crate::error::ConfigError::io(path, e))
}

/// providers.toml raw structure.
#[derive(Debug, Clone, serde::Deserialize)]
struct ProvidersToml {
    router: RouterConfig,
    tiers: TierConfigs,
    providers: HashMap<String, ProviderSettings>,
    #[serde(default)]
    model_library: ModelLibraryConfig,
}

impl From<ProvidersToml> for ProviderConfig {
    fn from(raw: ProvidersToml) -> Self {
        Self {
            router: raw.router,
            tiers: raw.tiers,
            providers: raw.providers,
            model_library: raw.model_library,
        }
    }
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            router: RouterConfig {
                strategy: "tiered".to_string(),
                default_tier: "medium".to_string(),
                fallback_tier: "medium".to_string(),
            },
            tiers: TierConfigs {
                light: TierConfig {
                    description: "Fast, low-cost tasks".to_string(),
                    provider: "openai".to_string(),
                    model: String::new(),
                },
                medium: TierConfig {
                    description: "Default project work".to_string(),
                    provider: "openai".to_string(),
                    model: String::new(),
                },
                high: TierConfig {
                    description: "Large refactors, architecture reasoning".to_string(),
                    provider: "openai".to_string(),
                    model: String::new(),
                },
            },
            providers: {
                let mut map = HashMap::new();
                map.insert(
                    "bigmodel".to_string(),
                    ProviderSettings {
                        enabled: true,
                        kind: "openai-compatible".to_string(),
                        base_url: "https://open.bigmodel.cn/api/coding/paas/v4".to_string(),
                        api_key_env: "BIGMODEL_API_KEY".to_string(),
                    },
                );
                map.insert(
                    "xiaomi_mimo".to_string(),
                    ProviderSettings {
                        enabled: false,
                        kind: "openai-compatible".to_string(),
                        base_url: "https://token-plan-cn.xiaomimimo.com/v1".to_string(),
                        api_key_env: "XIAOMI_MIMO_API_KEY".to_string(),
                    },
                );
                map.insert(
                    "deepseek".to_string(),
                    ProviderSettings {
                        enabled: false,
                        kind: "openai-compatible".to_string(),
                        base_url: "https://api.deepseek.com".to_string(),
                        api_key_env: "DEEPSEEK_API_KEY".to_string(),
                    },
                );
                map
            },
            model_library: ModelLibraryConfig::default(),
        }
    }
}
