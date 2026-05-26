//! Message types for conversations

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A message in a conversation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub role: MessageRole,
    pub content: String,
    pub created_at: DateTime<Utc>,
}

/// Role of a message
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    System,
    User,
    Assistant,
    /// Summary of older messages (for context window management)
    Summary,
}

impl Message {
    pub fn new_system(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content,
            created_at: Utc::now(),
        }
    }

    pub fn new_user(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content,
            created_at: Utc::now(),
        }
    }

    pub fn new_assistant(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            created_at: Utc::now(),
        }
    }

    pub fn new_summary(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Summary,
            content,
            created_at: Utc::now(),
        }
    }
}