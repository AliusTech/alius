//! Shared types for Alius

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

/// Session identifier
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionId(String);

impl SessionId {
    pub fn new() -> Self {
        Self(Uuid::new_v4().to_string())
    }

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

/// Provider type for LLM
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProviderType {
    Openai,
    Anthropic,
    Google,
    Custom,
}

impl Default for ProviderType {
    fn default() -> Self {
        Self::Openai
    }
}

/// Soul role
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SoulRole(String);

impl SoulRole {
    pub fn new(role: String) -> Self {
        Self(role)
    }

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

/// Session metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMetadata {
    pub id: SessionId,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub workspace: Option<PathBuf>,
    pub model: String,
    pub soul: Option<SoulRole>,
}

impl SessionMetadata {
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