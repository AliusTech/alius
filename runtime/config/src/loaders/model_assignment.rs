//! Loader for model.toml.
//!
//! `model.toml` stores the project-facing Plan/Execute/Review model
//! assignment. Older router tiers remain supported as an internal compatibility
//! layer through `ModelAssignmentConfig::from_provider_tiers`.

use crate::error::{ConfigError, ConfigResult};
use crate::views::{ModelAssignmentConfig, ProviderConfig};
use std::collections::HashSet;
use std::path::Path;

/// Load model.toml from the given path.
pub fn load_model_assignment(path: &Path) -> ConfigResult<ModelAssignmentConfig> {
    super::load_toml(path)
}

/// Save model.toml to the given path.
pub fn save_model_assignment(path: &Path, config: &ModelAssignmentConfig) -> ConfigResult<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::io(parent, e))?;
    }
    let content = toml::to_string_pretty(config).map_err(|e| ConfigError::serialize(path, e))?;
    std::fs::write(path, content).map_err(|e| ConfigError::io(path, e))
}

/// Load model.toml, or derive it from existing provider tiers when missing.
pub fn load_or_migrate_model_assignment(
    path: &Path,
    providers: &ProviderConfig,
) -> ModelAssignmentConfig {
    load_model_assignment(path)
        .unwrap_or_else(|_| ModelAssignmentConfig::from_provider_tiers(providers))
}

/// Return assignment validation issues against the enabled model pool.
pub fn validate_model_assignment(
    config: &ModelAssignmentConfig,
    providers: &ProviderConfig,
) -> Vec<String> {
    let enabled_ids = providers
        .model_library
        .models
        .iter()
        .filter(|entry| entry.enabled)
        .map(|entry| entry.id.as_str())
        .collect::<HashSet<_>>();

    let mut issues = Vec::new();
    for role in crate::views::ModelAssignmentRole::all() {
        let value = config.get(role).trim();
        if value.is_empty() {
            issues.push(format!("{} is not configured.", role.label()));
        } else if !enabled_ids.contains(value) {
            issues.push(format!(
                "{} references '{}' but it is not in the enabled model pool.",
                role.label(),
                value
            ));
        }
    }
    issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::{
        ModelAssignment, ModelAssignmentConfig, ModelLibraryConfig, ModelLibraryEntry,
        ProviderConfig, ReasoningNote, TierConfig,
    };
    use tempfile::TempDir;

    fn provider_config_with_models() -> ProviderConfig {
        let mut providers = ProviderConfig {
            model_library: ModelLibraryConfig {
                models: vec![
                    ModelLibraryEntry {
                        id: "planner".to_string(),
                        display_name: "planner".to_string(),
                        provider: "openai".to_string(),
                        base_url: "https://api.openai.com/v1".to_string(),
                        model_name: "gpt-plan".to_string(),
                        reasoning_note: ReasoningNote::Standard,
                        enabled: true,
                    },
                    ModelLibraryEntry {
                        id: "executor".to_string(),
                        display_name: "executor".to_string(),
                        provider: "openai".to_string(),
                        base_url: "https://api.openai.com/v1".to_string(),
                        model_name: "gpt-exec".to_string(),
                        reasoning_note: ReasoningNote::Standard,
                        enabled: true,
                    },
                    ModelLibraryEntry {
                        id: "disabled-reviewer".to_string(),
                        display_name: "reviewer".to_string(),
                        provider: "openai".to_string(),
                        base_url: "https://api.openai.com/v1".to_string(),
                        model_name: "gpt-review".to_string(),
                        reasoning_note: ReasoningNote::Standard,
                        enabled: false,
                    },
                ],
            },
            ..Default::default()
        };
        providers.tiers.light = TierConfig {
            description: "Plan Model".to_string(),
            provider: "openai".to_string(),
            model: "gpt-plan".to_string(),
        };
        providers.tiers.medium = TierConfig {
            description: "Execute Model".to_string(),
            provider: "openai".to_string(),
            model: "gpt-exec".to_string(),
        };
        providers.tiers.high = TierConfig {
            description: "Review Model".to_string(),
            provider: "openai".to_string(),
            model: "gpt-review".to_string(),
        };
        providers
    }

    #[test]
    fn model_assignment_roundtrips() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("model.toml");
        let config = ModelAssignmentConfig {
            schema_version: "0.1".to_string(),
            assignment: ModelAssignment {
                plan: "planner".to_string(),
                execute: "executor".to_string(),
                review: "reviewer".to_string(),
            },
        };

        save_model_assignment(&path, &config).unwrap();
        let loaded = load_model_assignment(&path).unwrap();

        assert_eq!(loaded, config);
    }

    #[test]
    fn missing_model_toml_migrates_from_provider_tiers() {
        let providers = provider_config_with_models();
        let migrated =
            load_or_migrate_model_assignment(Path::new("/missing/model.toml"), &providers);

        assert_eq!(migrated.assignment.plan, "planner");
        assert_eq!(migrated.assignment.execute, "executor");
        assert_eq!(migrated.assignment.review, "disabled-reviewer");
    }

    #[test]
    fn validation_requires_enabled_model_ids() {
        let providers = provider_config_with_models();
        let config = ModelAssignmentConfig {
            schema_version: "0.1".to_string(),
            assignment: ModelAssignment {
                plan: "planner".to_string(),
                execute: "executor".to_string(),
                review: "disabled-reviewer".to_string(),
            },
        };

        let issues = validate_model_assignment(&config, &providers);

        assert_eq!(issues.len(), 1);
        assert!(issues[0].contains("Review Model"));
    }
}
