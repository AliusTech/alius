//! Agent Card normalization for A2A publishing.
//!
//! This module converts SoulConfig to AgentCardView (A2A Agent Card JSON format).

use crate::error::{ConfigError, ConfigResult};
use crate::views::{
    AgentCardCapabilities, AgentCardInteraction, AgentCardInterface, AgentCardProvider,
    AgentCardSkill, AgentCardView, SoulConfig,
};
use std::path::Path;

/// Normalize SoulConfig to AgentCardView.
///
/// This converts the TOML-based soul.toml configuration to the
/// A2A Agent Card JSON format with camelCase field names.
pub fn normalize_agent_card(soul: &SoulConfig) -> AgentCardView {
    AgentCardView {
        name: soul.agent.name.clone(),
        description: soul.agent.description.clone(),
        version: soul.agent.version.clone(),
        documentation_url: soul.agent_card.documentation_url.clone(),
        icon_url: soul.agent_card.icon_url.clone(),
        provider: AgentCardProvider {
            organization: soul.provider.organization.clone(),
            url: soul.provider.url.clone(),
        },
        capabilities: AgentCardCapabilities {
            streaming: soul.capabilities.streaming,
            push_notifications: soul.capabilities.push_notifications,
            extended_agent_card: soul.capabilities.extended_agent_card,
        },
        supported_interfaces: soul
            .supported_interfaces
            .iter()
            .map(|si| AgentCardInterface {
                url: si.url.clone(),
                protocol_binding: si.protocol_binding.clone(),
                protocol_version: si.protocol_version.clone(),
            })
            .collect(),
        skills: soul
            .skills
            .iter()
            .map(|s| AgentCardSkill {
                id: s.id.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                tags: s.tags.clone(),
                examples: s.examples.clone(),
                input_modes: s.input_modes.clone(),
                output_modes: s.output_modes.clone(),
            })
            .collect(),
        interaction: AgentCardInteraction {
            default_input_modes: soul.interaction.default_input_modes.clone(),
            default_output_modes: soul.interaction.default_output_modes.clone(),
        },
    }
}

/// Export AgentCardView to JSON string.
pub fn export_agent_card_json(card: &AgentCardView) -> ConfigResult<String> {
    serde_json::to_string_pretty(card).map_err(|e| {
        ConfigError::validation("agent_card", format!("JSON serialization failed: {}", e))
    })
}

/// Export AgentCardView to file.
pub fn export_agent_card_to_file(card: &AgentCardView, path: &Path) -> ConfigResult<()> {
    let json = export_agent_card_json(card)?;

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| ConfigError::io(parent, e))?;
    }

    std::fs::write(path, json).map_err(|e| ConfigError::io(path, e))?;
    Ok(())
}

/// Validate that AgentCardView has required fields for publishing.
///
/// Returns a list of missing/invalid fields.
pub fn validate_agent_card_for_publishing(card: &AgentCardView) -> Vec<String> {
    let mut issues = Vec::new();

    if card.name.is_empty() {
        issues.push("agent.name is empty".to_string());
    }

    if card.description.is_empty() {
        issues.push("agent.description is empty".to_string());
    }

    // Check for valid supported interfaces
    let has_valid_interface = card
        .supported_interfaces
        .iter()
        .any(|si| !si.url.is_empty());

    if !has_valid_interface {
        issues.push("supported_interfaces has no valid URL".to_string());
    }

    // Check for valid skills
    let has_valid_skill = card
        .skills
        .iter()
        .any(|s| !s.id.is_empty() && !s.name.is_empty());

    if !has_valid_skill {
        issues.push("skills has no valid skill defined".to_string());
    }

    issues
}

/// Check if AgentCardView is ready for A2A publishing.
pub fn is_ready_for_publishing(card: &AgentCardView) -> bool {
    validate_agent_card_for_publishing(card).is_empty()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::views::{
        AgentCapabilities, AgentCardSettings, AgentIdentity, AgentProvider, AgentSkill,
        InteractionModes, SupportedInterface,
    };
    use tempfile::TempDir;

    fn create_test_soul_config() -> SoulConfig {
        SoulConfig {
            agent: AgentIdentity {
                name: "Developer Assistant".to_string(),
                description: "Helps with code review and refactoring".to_string(),
                version: "1.0.0".to_string(),
            },
            agent_card: AgentCardSettings {
                documentation_url: "https://docs.example.com".to_string(),
                icon_url: "https://example.com/icon.png".to_string(),
                export_path: ".well-known/agent-card.json".to_string(),
            },
            supported_interfaces: vec![SupportedInterface {
                url: "https://api.example.com/a2a".to_string(),
                protocol_binding: "HTTP+JSON".to_string(),
                protocol_version: "1.0".to_string(),
            }],
            provider: AgentProvider {
                organization: "Example Inc".to_string(),
                url: "https://example.com".to_string(),
            },
            capabilities: AgentCapabilities {
                streaming: true,
                push_notifications: false,
                extended_agent_card: false,
            },
            interaction: InteractionModes {
                default_input_modes: vec!["text/plain".to_string()],
                default_output_modes: vec!["text/plain".to_string()],
            },
            skills: vec![AgentSkill {
                id: "code-review".to_string(),
                name: "Code Review".to_string(),
                description: "Reviews code for quality".to_string(),
                tags: vec!["code".to_string()],
                examples: vec!["Review this PR".to_string()],
                input_modes: vec!["text/plain".to_string()],
                output_modes: vec!["text/plain".to_string()],
            }],
        }
    }

    #[test]
    fn test_normalize_agent_card() {
        let soul = create_test_soul_config();
        let card = normalize_agent_card(&soul);

        assert_eq!(card.name, "Developer Assistant");
        assert_eq!(card.description, "Helps with code review and refactoring");
        assert_eq!(card.version, "1.0.0");
        assert_eq!(card.documentation_url, "https://docs.example.com");
        assert_eq!(card.provider.organization, "Example Inc");
        assert!(card.capabilities.streaming);
        assert_eq!(card.supported_interfaces.len(), 1);
        assert_eq!(card.skills.len(), 1);
        assert_eq!(card.skills[0].id, "code-review");
    }

    #[test]
    fn test_export_agent_card_json() {
        let soul = create_test_soul_config();
        let card = normalize_agent_card(&soul);
        let json = export_agent_card_json(&card).unwrap();

        // Check JSON has camelCase field names
        assert!(json.contains("\"name\""));
        assert!(json.contains("\"description\""));
        assert!(json.contains("\"documentationUrl\""));
        assert!(json.contains("\"iconUrl\""));
        assert!(json.contains("\"supportedInterfaces\""));
        assert!(json.contains("\"protocolBinding\""));
        assert!(json.contains("\"pushNotifications\""));
        assert!(json.contains("\"defaultInputModes\""));
        assert!(json.contains("\"inputModes\""));
    }

    #[test]
    fn test_export_agent_card_to_file() {
        let soul = create_test_soul_config();
        let card = normalize_agent_card(&soul);

        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join(".well-known/agent-card.json");

        export_agent_card_to_file(&card, &path).unwrap();

        assert!(path.exists());

        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("\"Developer Assistant\""));
    }

    #[test]
    fn test_validate_agent_card_for_publishing() {
        let soul = create_test_soul_config();
        let card = normalize_agent_card(&soul);

        let issues = validate_agent_card_for_publishing(&card);
        assert!(issues.is_empty());
        assert!(is_ready_for_publishing(&card));
    }

    #[test]
    fn test_validate_incomplete_agent_card() {
        let soul = SoulConfig {
            agent: AgentIdentity {
                name: String::new(),
                description: String::new(),
                version: "1.0.0".to_string(),
            },
            agent_card: AgentCardSettings {
                documentation_url: String::new(),
                icon_url: String::new(),
                export_path: ".well-known/agent-card.json".to_string(),
            },
            supported_interfaces: vec![SupportedInterface {
                url: String::new(),
                protocol_binding: "HTTP+JSON".to_string(),
                protocol_version: "1.0".to_string(),
            }],
            provider: AgentProvider {
                organization: String::new(),
                url: String::new(),
            },
            capabilities: AgentCapabilities {
                streaming: true,
                push_notifications: false,
                extended_agent_card: false,
            },
            interaction: InteractionModes {
                default_input_modes: vec!["text/plain".to_string()],
                default_output_modes: vec!["text/plain".to_string()],
            },
            skills: vec![AgentSkill {
                id: String::new(),
                name: String::new(),
                description: String::new(),
                tags: vec![],
                examples: vec![],
                input_modes: vec!["text/plain".to_string()],
                output_modes: vec!["text/plain".to_string()],
            }],
        };

        let card = normalize_agent_card(&soul);
        let issues = validate_agent_card_for_publishing(&card);

        assert!(!issues.is_empty());
        assert!(issues.contains(&"agent.name is empty".to_string()));
        assert!(issues.contains(&"agent.description is empty".to_string()));
        assert!(issues.contains(&"supported_interfaces has no valid URL".to_string()));
        assert!(issues.contains(&"skills has no valid skill defined".to_string()));
        assert!(!is_ready_for_publishing(&card));
    }
}
