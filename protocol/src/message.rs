//! Message types for conversations.
//!
//! Defines the message structure used in LLM conversations:
//! - `Message`: A single message with role, content, and timestamp
//! - `MessageRole`: The role of the message sender (system, user, assistant, tool, summary)

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// A tool call requested by the assistant (function calling).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageToolCall {
    /// Tool-call id (matches the id returned by the model).
    pub id: String,
    /// Function/tool name.
    pub name: String,
    /// JSON-encoded arguments string.
    pub arguments: String,
}

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
    /// Tool calls attached to an assistant message (function calling).
    /// `None` for non-assistant messages and assistant messages without tools.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<MessageToolCall>>,
    /// Tool-call id answered by a tool result message.
    /// `None` for non-tool messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    /// Tool name answered by a tool result message.
    /// `None` for non-tool messages.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_name: Option<String>,
}

/// Role of a message sender in the conversation.
///
/// Maps to the OpenAI chat completion message roles:
/// - `System`: System prompt that sets the agent's behavior
/// - `User`: Messages from the human user
/// - `Assistant`: Responses from the LLM
/// - `Tool`: Tool result that responds to an assistant tool call
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
    /// Tool result that responds to an assistant tool call.
    Tool,
    /// Summary of older messages, used for context window management.
    Summary,
}

impl Message {
    /// Create a new system message.
    pub fn new_system(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::System,
            content,
            created_at: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create a new user message.
    pub fn new_user(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::User,
            content,
            created_at: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create a new assistant message.
    pub fn new_assistant(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            created_at: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create a new assistant message that also carries tool calls.
    /// Used when the model requests tools — the same assistant turn has both
    /// (optional) text content and the tool_calls list, so the next round can
    /// send tool results that correctly follow a tool_calls-bearing message.
    pub fn new_assistant_with_tools(content: String, tool_calls: Vec<MessageToolCall>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Assistant,
            content,
            created_at: Utc::now(),
            tool_calls: Some(tool_calls),
            tool_call_id: None,
            tool_name: None,
        }
    }

    /// Create a new tool result message.
    pub fn new_tool_result(tool_call_id: String, tool_name: String, content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Tool,
            content,
            created_at: Utc::now(),
            tool_calls: None,
            tool_call_id: Some(tool_call_id),
            tool_name: Some(tool_name),
        }
    }

    /// Create a new summary message.
    pub fn new_summary(content: String) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            role: MessageRole::Summary,
            content,
            created_at: Utc::now(),
            tool_calls: None,
            tool_call_id: None,
            tool_name: None,
        }
    }
}
