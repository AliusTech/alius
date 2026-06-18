//! Model Router — tiered routing with credential resolution and fallback chains.
//!
//! Routes model requests to the appropriate provider based on tier configuration.
//! Falls back to a lower tier when the primary provider fails.

use anyhow::{bail, Result};

use crate::credential;

/// A fully resolved route to a specific provider and model.
#[derive(Debug, Clone)]
pub struct ResolvedRoute {
    /// Provider name (e.g. "bigmodel", "openai").
    pub provider: String,
    /// Provider kind (e.g. "anthropic", "openai", "google").
    pub kind: String,
    /// Model identifier.
    pub model: String,
    /// Base URL for API calls.
    pub base_url: String,
    /// Resolved API key.
    pub api_key: String,
    /// Tier that produced this route.
    pub tier: String,
}

/// Tier configuration.
#[derive(Debug, Clone)]
pub struct TierEntry {
    pub provider: String,
    pub model: String,
}

/// Provider configuration.
#[derive(Debug, Clone)]
pub struct ProviderEntry {
    pub kind: String,
    pub base_url: String,
    pub api_key_env: String,
}

/// Model Router configuration.
#[derive(Debug, Clone)]
pub struct ModelRouterConfig {
    pub default_tier: String,
    pub fallback_tier: String,
    pub default_model: String,
    pub tiers: std::collections::HashMap<String, TierEntry>,
    pub providers: std::collections::HashMap<String, ProviderEntry>,
}

/// Model Router — resolves tier to provider+model+credentials.
pub struct ModelRouter {
    config: ModelRouterConfig,
}

impl ModelRouter {
    /// Create a new ModelRouter from configuration.
    pub fn new(config: ModelRouterConfig) -> Self {
        Self { config }
    }

    /// Route a model request for the given tier.
    ///
    /// Returns a `ResolvedRoute` with all information needed to make an API call.
    pub fn route_model(&self, tier: &str) -> Result<ResolvedRoute> {
        let tier_entry = self
            .config
            .tiers
            .get(tier)
            .ok_or_else(|| anyhow::anyhow!("tier '{}' not found in configuration", tier))?;

        let provider = self
            .config
            .providers
            .get(&tier_entry.provider)
            .ok_or_else(|| {
                anyhow::anyhow!(
                    "provider '{}' referenced by tier '{}' not found",
                    tier_entry.provider,
                    tier
                )
            })?;

        let model = if tier_entry.model.is_empty() {
            self.config.default_model.clone()
        } else {
            tier_entry.model.clone()
        };

        let api_key = credential::resolve_secret(&format!("env:{}", provider.api_key_env))?;

        Ok(ResolvedRoute {
            provider: tier_entry.provider.clone(),
            kind: provider.kind.clone(),
            model,
            base_url: provider.base_url.clone(),
            api_key,
            tier: tier.to_string(),
        })
    }

    /// Attempt a fallback route when the primary provider fails.
    ///
    /// Falls back to the configured `fallback_tier`.
    pub fn fallback_route(&self, failed_provider: &str, error: &str) -> Result<ResolvedRoute> {
        let fallback_tier = &self.config.fallback_tier;

        let route = self.route_model(fallback_tier)?;

        if route.provider == failed_provider {
            bail!(
                "fallback tier '{}' uses the same provider '{}' that failed: {}",
                fallback_tier,
                failed_provider,
                error
            );
        }

        Ok(ResolvedRoute {
            tier: format!("{} (fallback from error: {})", fallback_tier, error),
            ..route
        })
    }

    /// Route using the default tier.
    pub fn route_default(&self) -> Result<ResolvedRoute> {
        self.route_model(&self.config.default_tier)
    }
}

impl ResolvedRoute {
    /// Convert a resolved route to `LlmSettings` for use with `LlmClient`.
    pub fn to_llm_settings(&self) -> runtime_config::LlmSettings {
        let provider = match self.kind.to_lowercase().as_str() {
            "openai" => protocol_interface::ProviderType::Openai,
            "anthropic" => protocol_interface::ProviderType::Anthropic,
            "google" => protocol_interface::ProviderType::Google,
            "bigmodel" => protocol_interface::ProviderType::BigModel,
            "deepseek" => protocol_interface::ProviderType::DeepSeek,
            "xiaomi_mimo" | "xiaomi-mimo" => protocol_interface::ProviderType::XiaomiMimo,
            _ => protocol_interface::ProviderType::Custom,
        };
        runtime_config::LlmSettings {
            provider,
            provider_mode: None,
            model: self.model.clone(),
            api_key: Some(self.api_key.clone()),
            api_key_env: None,
            base_url: if self.base_url.is_empty() {
                None
            } else {
                Some(self.base_url.clone())
            },
            review_model: None,
        }
    }
}

impl ModelRouterConfig {
    /// Build a `ModelRouterConfig` from a project config snapshot.
    ///
    /// Returns `None` if the project config doesn't have provider/tier data.
    pub fn from_project_config(
        snapshot: &runtime_config::views::ProjectConfigSnapshot,
    ) -> Option<Self> {
        let router = &snapshot.providers.router;
        let tiers = &snapshot.providers.tiers;

        let mut tier_map = std::collections::HashMap::new();
        tier_map.insert(
            "light".to_string(),
            TierEntry {
                provider: tiers.light.provider.clone(),
                model: tiers.light.model.clone(),
            },
        );
        tier_map.insert(
            "medium".to_string(),
            TierEntry {
                provider: tiers.medium.provider.clone(),
                model: tiers.medium.model.clone(),
            },
        );
        tier_map.insert(
            "high".to_string(),
            TierEntry {
                provider: tiers.high.provider.clone(),
                model: tiers.high.model.clone(),
            },
        );

        let mut provider_map = std::collections::HashMap::new();
        for (name, ps) in &snapshot.providers.providers {
            if ps.enabled {
                provider_map.insert(
                    name.clone(),
                    ProviderEntry {
                        kind: ps.kind.clone(),
                        base_url: ps.base_url.clone(),
                        api_key_env: ps.api_key_env.clone(),
                    },
                );
            }
        }

        // Need at least one tier and one provider
        if tier_map.is_empty() || provider_map.is_empty() {
            return None;
        }

        Some(ModelRouterConfig {
            default_tier: router.default_tier.clone(),
            fallback_tier: router.fallback_tier.clone(),
            default_model: snapshot.model.default_model.clone(),
            tiers: tier_map,
            providers: provider_map,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_config() -> ModelRouterConfig {
        let mut tiers = std::collections::HashMap::new();
        tiers.insert(
            "light".to_string(),
            TierEntry {
                provider: "bigmodel".to_string(),
                model: "glm-4-flash".to_string(),
            },
        );
        tiers.insert(
            "medium".to_string(),
            TierEntry {
                provider: "bigmodel".to_string(),
                model: String::new(), // use default
            },
        );
        tiers.insert(
            "high".to_string(),
            TierEntry {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-20250514".to_string(),
            },
        );

        let mut providers = std::collections::HashMap::new();
        providers.insert(
            "bigmodel".to_string(),
            ProviderEntry {
                kind: "anthropic".to_string(),
                base_url: "https://open.bigmodel.cn/api/anthropic".to_string(),
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
            },
        );
        providers.insert(
            "anthropic".to_string(),
            ProviderEntry {
                kind: "anthropic".to_string(),
                base_url: "https://api.anthropic.com".to_string(),
                api_key_env: "ANTHROPIC_API_KEY".to_string(),
            },
        );

        ModelRouterConfig {
            default_tier: "medium".to_string(),
            fallback_tier: "medium".to_string(),
            default_model: "glm-4.7".to_string(),
            tiers,
            providers,
        }
    }

    #[test]
    fn test_route_light_tier() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let router = ModelRouter::new(make_test_config());
        let route = router.route_model("light").unwrap();
        assert_eq!(route.provider, "bigmodel");
        assert_eq!(route.model, "glm-4-flash");
        assert_eq!(route.kind, "anthropic");
    }

    #[test]
    fn test_route_medium_tier() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let router = ModelRouter::new(make_test_config());
        let route = router.route_model("medium").unwrap();
        assert_eq!(route.provider, "bigmodel");
        assert_eq!(route.model, "glm-4.7"); // uses default_model
    }

    #[test]
    fn test_route_high_tier() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let router = ModelRouter::new(make_test_config());
        let route = router.route_model("high").unwrap();
        assert_eq!(route.provider, "anthropic");
        assert_eq!(route.model, "claude-sonnet-4-20250514");
    }

    #[test]
    fn test_tier_model_empty_uses_default() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let router = ModelRouter::new(make_test_config());
        let route = router.route_model("medium").unwrap();
        assert_eq!(route.model, "glm-4.7");
    }

    #[test]
    fn test_fallback_on_provider_error() {
        std::env::set_var("ANTHROPIC_API_KEY", "test-key-123");
        let mut config = make_test_config();
        config.fallback_tier = "medium".to_string();
        let router = ModelRouter::new(config);
        // high tier uses anthropic; fallback to medium (bigmodel) should work
        let route = router.fallback_route("anthropic", "timeout").unwrap();
        assert_eq!(route.provider, "bigmodel");
    }

    #[test]
    fn test_route_unknown_tier_returns_error() {
        let router = ModelRouter::new(make_test_config());
        let result = router.route_model("unknown");
        assert!(result.is_err());
    }
}
