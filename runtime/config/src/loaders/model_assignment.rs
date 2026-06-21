//! Loader for model.toml.
//!
//! `model.toml` stores the project-facing Plan/Execute/Review model
//! assignment. Older router tiers remain supported as an internal compatibility
//! layer through `ModelAssignmentConfig::from_provider_tiers`.

use crate::error::{ConfigError, ConfigResult};
use crate::views::{ModelAssignmentConfig, ModelAssignmentRole, ProviderConfig};
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
///
/// Only migrates when the file doesn't exist. Parse errors are propagated.
pub fn load_or_migrate_model_assignment(
    path: &Path,
    providers: &ProviderConfig,
) -> ConfigResult<ModelAssignmentConfig> {
    if !path.exists() {
        return Ok(ModelAssignmentConfig::from_provider_tiers(providers));
    }
    load_model_assignment(path)
}

/// Structured model-assignment readiness issue for request-time guards.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelAssignmentReadinessIssue {
    pub role: ModelAssignmentRole,
    pub model_id: Option<String>,
    pub kind: ModelAssignmentReadinessIssueKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAssignmentReadinessIssueKind {
    NotConfigured,
    MissingModel,
    DisabledModel,
}

impl ModelAssignmentReadinessIssue {
    pub fn message(&self) -> String {
        match self.kind {
            ModelAssignmentReadinessIssueKind::NotConfigured => {
                format!("{} is not configured.", self.role.label())
            }
            ModelAssignmentReadinessIssueKind::MissingModel
            | ModelAssignmentReadinessIssueKind::DisabledModel => format!(
                "{} references '{}' but it is not in the enabled model pool.",
                self.role.label(),
                self.model_id.as_deref().unwrap_or_default()
            ),
        }
    }
}

/// Return structured assignment readiness issues against the model pool.
pub fn model_assignment_readiness_issues(
    config: &ModelAssignmentConfig,
    providers: &ProviderConfig,
) -> Vec<ModelAssignmentReadinessIssue> {
    let mut issues = Vec::new();
    for role in ModelAssignmentRole::all() {
        let value = config.get(role).trim();
        if value.is_empty() {
            issues.push(ModelAssignmentReadinessIssue {
                role,
                model_id: None,
                kind: ModelAssignmentReadinessIssueKind::NotConfigured,
            });
            continue;
        }

        match providers
            .model_library
            .models
            .iter()
            .find(|entry| entry.id == value)
        {
            None => issues.push(ModelAssignmentReadinessIssue {
                role,
                model_id: Some(value.to_string()),
                kind: ModelAssignmentReadinessIssueKind::MissingModel,
            }),
            Some(entry) if !entry.enabled => issues.push(ModelAssignmentReadinessIssue {
                role,
                model_id: Some(value.to_string()),
                kind: ModelAssignmentReadinessIssueKind::DisabledModel,
            }),
            Some(_) => {}
        }
    }
    issues
}

/// Return assignment validation issues against the enabled model pool.
pub fn validate_model_assignment(
    config: &ModelAssignmentConfig,
    providers: &ProviderConfig,
) -> Vec<String> {
    model_assignment_readiness_issues(config, providers)
        .into_iter()
        .map(|issue| issue.message())
        .collect()
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
            load_or_migrate_model_assignment(Path::new("/missing/model.toml"), &providers).unwrap();

        assert_eq!(migrated.assignment.plan, "planner");
        assert_eq!(migrated.assignment.execute, "executor");
        assert_eq!(migrated.assignment.review, "disabled-reviewer");
    }

    #[test]
    fn existing_malformed_model_toml_returns_error() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("model.toml");
        std::fs::write(&path, "this is not valid toml [[[[").unwrap();

        let providers = provider_config_with_models();
        let result = load_or_migrate_model_assignment(&path, &providers);

        assert!(result.is_err(), "malformed model.toml must return error");
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

    #[test]
    fn readiness_reports_unconfigured_missing_and_disabled_assignments() {
        let providers = provider_config_with_models();
        let config = ModelAssignmentConfig {
            schema_version: "0.1".to_string(),
            assignment: ModelAssignment {
                plan: String::new(),
                execute: "deleted-executor".to_string(),
                review: "disabled-reviewer".to_string(),
            },
        };

        let issues = model_assignment_readiness_issues(&config, &providers);

        assert_eq!(issues.len(), 3);
        assert_eq!(issues[0].role, crate::views::ModelAssignmentRole::Plan);
        assert_eq!(
            issues[0].kind,
            ModelAssignmentReadinessIssueKind::NotConfigured
        );
        assert_eq!(issues[1].role, crate::views::ModelAssignmentRole::Execute);
        assert_eq!(
            issues[1].kind,
            ModelAssignmentReadinessIssueKind::MissingModel
        );
        assert_eq!(issues[2].role, crate::views::ModelAssignmentRole::Review);
        assert_eq!(
            issues[2].kind,
            ModelAssignmentReadinessIssueKind::DisabledModel
        );
    }
}
