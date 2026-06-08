//! Loader for soul.toml.
//!
//! This loader parses the complete soul.toml file, including all fields
//! needed for A2A Agent Card generation.

use crate::error::ConfigResult;
use crate::views::{
    AgentCapabilities, AgentCardSettings, AgentIdentity, AgentProvider, AgentSkill,
    InteractionModes, SoulConfig, SupportedInterface,
};
use std::path::Path;

/// Load soul.toml from the given path.
pub fn load_soul(path: &Path) -> ConfigResult<SoulConfig> {
    let raw: SoulToml = super::load_toml(path)?;
    Ok(raw.into())
}

/// soul.toml raw structure.
#[derive(Debug, Clone, serde::Deserialize)]
struct SoulToml {
    agent: AgentIdentity,
    agent_card: AgentCardSettings,
    supported_interfaces: Vec<SupportedInterface>,
    provider: AgentProvider,
    capabilities: AgentCapabilities,
    interaction: InteractionModes,
    #[serde(default)]
    skills: Vec<AgentSkill>,
}

impl From<SoulToml> for SoulConfig {
    fn from(raw: SoulToml) -> Self {
        Self {
            agent: raw.agent,
            agent_card: raw.agent_card,
            supported_interfaces: raw.supported_interfaces,
            provider: raw.provider,
            capabilities: raw.capabilities,
            interaction: raw.interaction,
            skills: raw.skills,
        }
    }
}

impl Default for SoulConfig {
    fn default() -> Self {
        Self {
            agent: AgentIdentity {
                name: String::new(),
                description: String::new(),
                version: "0.1.0".to_string(),
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
                default_input_modes: vec!["text/plain".to_string(), "application/json".to_string()],
                default_output_modes: vec![
                    "text/plain".to_string(),
                    "application/json".to_string(),
                ],
            },
            skills: vec![AgentSkill {
                id: String::new(),
                name: String::new(),
                description: String::new(),
                tags: vec![],
                examples: vec![],
                input_modes: vec!["text/plain".to_string(), "application/json".to_string()],
                output_modes: vec!["text/plain".to_string(), "application/json".to_string()],
            }],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_soul_toml(content: &str) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        write!(file, "{}", content).unwrap();
        file
    }

    #[test]
    fn test_load_soul_minimal() {
        let content = r#"
[agent]
name = "Test Agent"
description = "A test agent"
version = "1.0.0"

[agent_card]
documentation_url = ""
icon_url = ""
export_path = ".well-known/agent-card.json"

[[supported_interfaces]]
url = ""
protocol_binding = "HTTP+JSON"
protocol_version = "1.0"

[provider]
organization = "Test Org"
url = ""

[capabilities]
streaming = true
push_notifications = false
extended_agent_card = false

[interaction]
default_input_modes = ["text/plain"]
default_output_modes = ["text/plain"]
"#;
        let file = create_temp_soul_toml(content);
        let config = load_soul(file.path()).unwrap();

        assert_eq!(config.agent.name, "Test Agent");
        assert_eq!(config.agent.description, "A test agent");
        assert_eq!(config.agent.version, "1.0.0");
        assert_eq!(config.provider.organization, "Test Org");
        assert!(config.capabilities.streaming);
        assert!(!config.capabilities.push_notifications);
    }

    #[test]
    fn test_load_soul_with_skills() {
        let content = r#"
[agent]
name = "Developer Assistant"
description = "Helps with development tasks"
version = "1.0.0"

[agent_card]
documentation_url = "https://docs.example.com"
icon_url = "https://example.com/icon.png"
export_path = ".well-known/agent-card.json"

[[supported_interfaces]]
url = "https://api.example.com/a2a"
protocol_binding = "HTTP+JSON"
protocol_version = "1.0"

[provider]
organization = "Example Inc"
url = "https://example.com"

[capabilities]
streaming = true
push_notifications = true
extended_agent_card = false

[interaction]
default_input_modes = ["text/plain", "application/json"]
default_output_modes = ["text/plain", "application/json"]

[[skills]]
id = "code-review"
name = "Code Review"
description = "Reviews code for quality and security"
tags = ["code", "review", "quality"]
examples = ["Review the changes in this PR"]
input_modes = ["text/plain"]
output_modes = ["text/plain"]
"#;
        let file = create_temp_soul_toml(content);
        let config = load_soul(file.path()).unwrap();

        assert_eq!(config.agent.name, "Developer Assistant");
        assert_eq!(
            config.agent_card.documentation_url,
            "https://docs.example.com"
        );
        assert_eq!(config.supported_interfaces.len(), 1);
        assert_eq!(config.skills.len(), 1);
        assert_eq!(config.skills[0].id, "code-review");
        assert_eq!(config.skills[0].tags, vec!["code", "review", "quality"]);
    }
}
