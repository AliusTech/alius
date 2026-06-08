//! Message types for conversations.
//!
//! Defines the message structure used in LLM conversations:
//! - `Message`: A single message with role, content, and timestamp
//! - `MessageRole`: The role of the message sender (system, user, assistant, summary)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A message in a conversation.
///
/// Each message has a unique ID, a role indicating who sent it,
/// the message content, and a timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Unique message identifier (UUID).
    pub id: String,
    /// The role of the message sender.
    pub role: MessageRole,
    /// The message content text.
    pub content: String,
    /// Timestamp when the message was created.
    pub created_at: DateTime<Utc>,
}

/// Role of a message sender in the conversation.
///
/// Maps to the OpenAI chat completion message roles:
/// - `System`: System prompt that sets the agent's behavior
/// - `User`: Messages from the human user
/// - `Assistant`: Responses from the LLM
/// - `Summary`: Condensed summary of older messages (for context window management)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    /// System prompt that defines the agent's behavior and expertise.
    System,
    /// Messages from the human user.
    User,
    /// Responses from the LLM assistant.
    Assistant,
    /// Summary of older messages, used for context window management.
    Summary,
}

impl Message {
    /// Create a new system message.
    ///
    /// System messages set the agent's behavior and are typically sent
    /// at the beginning of a conversation.
    pub fn new_system(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content,
            created_at: Utc::now(),
        }
    }

    /// Create a new user message.
    ///
    /// User messages represent input from the human user.
    pub fn new_user(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content,
            created_at: Utc::now(),
        }
    }

    /// Create a new assistant message.
    ///
    /// Assistant messages represent responses from the LLM.
    pub fn new_assistant(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            created_at: Utc::now(),
        }
    }

    /// Create a new summary message.
    ///
    /// Summary messages are condensed versions of older messages,
    /// used to manage the context window size.
    pub fn new_summary(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Summary,
            content,
            created_at: Utc::now(),
        }
    }
}
