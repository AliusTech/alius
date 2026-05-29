//! Shared types for the Alius application.
//!
//! Defines core data structures used across all crates:
//! - `SessionId`: Unique session identifier (UUID-based)
//! - `ProviderType`: Supported LLM provider types
//! - `SoulRole`: Agent persona/role identifier
//! - `SessionMetadata`: Session state and configuration

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Unique session identifier, backed by a UUID v4 string.
///
/// Each REPL session gets a unique ID for tracking conversation history
/// and session state.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionId(String);

impl SessionId {
    /// Create a new random session identifier.
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    /// Get the session ID as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SessionId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for SessionId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Supported LLM provider types.
///
/// Each provider has its own API format and authentication method.
/// The `Custom` variant allows using any OpenAI-compatible endpoint.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    /// OpenAI API (GPT models)
    #[default]
    Openai,
    /// Anthropic API (Claude models)
    Anthropic,
    /// Google Generative AI API (Gemini models)
    Google,
    /// Custom OpenAI-compatible endpoint
    Custom,
}

/// Soul role (agent persona) identifier.
///
/// Defines the agent's behavior, expertise, and response style.
/// Maps to a system prompt via `soul::system_prompt_for_role()`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoulRole(String);

impl SoulRole {
    /// Create a new soul role with the given name.
    pub fn new(role: String) -> Self {
        Self(role)
    }

    /// Get the role name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for SoulRole {
    fn default() -> Self {
        Self::new("Frontend Engineer".to_string())
    }
}

impl std::fmt::Display for SoulRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Session metadata, tracking the state and configuration of a REPL session.
///
/// Stores the session's unique ID, timestamps, workspace path, model, and soul role.
/// Used for session persistence and restoration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    /// Unique session identifier.
    pub id: SessionId,
    /// Timestamp when the session was created.
    pub created_at: DateTime<Utc>,
    /// Timestamp of the last activity in the session.
    pub updated_at: DateTime<Utc>,
    /// Optional workspace directory path for file operations.
    pub workspace: Option<PathBuf>,
    /// The LLM model being used in this session.
    pub model: String,
    /// The soul role (agent persona) for this session.
    pub soul: Option<SoulRole>,
}

impl SessionMetadata {
    /// Create new session metadata with the given model.
    ///
    /// Sets the current time as both created_at and updated_at.
    /// Generates a new random session ID.
    pub fn new(model: String) -> Self {
        let now = Utc::now();
        Self {
            id: SessionId::new(),
            created_at: now,
            updated_at: now,
            workspace: None,
            model,
            soul: None,
        }
    }
}

/// Provider-agnostic tool definition for LLM function calling.
///
/// Each provider converts this to its native format:
/// - OpenAI: `{type:"function", function:{name, description, parameters}}`
/// - Anthropic: `{name, description, input_schema}`
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDef {
    /// Tool name (used by the LLM to call the tool).
    pub name: String,
    /// Tool description (shown to the LLM).
    pub description: String,
    /// JSON Schema for the tool's input parameters.
    pub parameters: serde_json::Value,
}
