pub mod settings;
pub mod soul;

pub use protocol_interface::{ProviderMode, ProviderType, SoulRole};
pub use settings::{AgentSettings, LlmSettings, Settings, SoulSettings, UiSettings};
pub use soul::system_prompt_for_role;

/// Parse a provider type string (e.g., "openai", "anthropic") into a ProviderType.
pub fn parse_provider_type(s: &str) -> ProviderType {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or_default()
}
