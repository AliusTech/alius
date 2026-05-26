//! Conversation storage

use anyhow::Result;
use std::path::PathBuf;

use alius_protocol::{Message, SessionId};

/// Conversation store for managing message persistence
pub struct ConversationStore {
    base_path: PathBuf,
}

impl ConversationStore {
    /// Create a new conversation store
    pub fn new() -> Result<Self> {
        let base_path = Self::get_base_path();
        std::fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }

    /// Get the base path for conversations
    fn get_base_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(format!("{}/.alius/sessions", home))
    }

    /// Get the messages file path for a session
    fn messages_path(&self, session_id: &SessionId) -> PathBuf {
        self.base_path.join(session_id.as_str()).join("messages.jsonl")
    }

    /// Save all messages for a session
    pub fn save_messages(&self, session_id: &SessionId, messages: &[Message]) -> Result<()> {
        let messages_path = self.messages_path(session_id);
        let mut content = String::new();
        for msg in messages {
            content.push_str(&serde_json::to_string(msg)?);
            content.push('\n');
        }
        std::fs::write(messages_path, content)?;
        Ok(())
    }

    /// Load all messages for a session
    pub fn load_messages(&self, session_id: &SessionId) -> Result<Vec<Message>> {
        let messages_path = self.messages_path(session_id);
        if !messages_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(messages_path)?;
        let messages: Vec<Message> = content
            .lines()
            .filter(|line| !line.is_empty())
            .map(|line| serde_json::from_str(line))
            .collect::<Result<Vec<Message>, _>>()?;

        Ok(messages)
    }

    /// Append a single message (append-only)
    pub fn append_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        let messages_path = self.messages_path(session_id);
        let line = serde_json::to_string(message)?;
        std::fs::write(&messages_path, format!("{}\n", line))?;
        Ok(())
    }

    /// Clear all messages for a session
    pub fn clear_messages(&self, session_id: &SessionId) -> Result<()> {
        let messages_path = self.messages_path(session_id);
        if messages_path.exists() {
            std::fs::remove_file(messages_path)?;
        }
        Ok(())
    }
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new().expect("Failed to create conversation store")
    }
}