//! Conversation storage

use anyhow::Result;
use std::path::PathBuf;

use protocol_interface::{Message, SessionId};

use super::paths;

/// Conversation store for managing message persistence
pub struct ConversationStore {
    base_path: PathBuf,
    legacy_base_paths: Vec<PathBuf>,
}

impl ConversationStore {
    /// Create a new conversation store
    pub fn new() -> Result<Self> {
        let base_path = Self::get_base_path();
        std::fs::create_dir_all(&base_path)?;
        Ok(Self {
            base_path,
            legacy_base_paths: Self::get_legacy_base_paths(),
        })
    }

    /// Get the base path for conversations
    fn get_base_path() -> PathBuf {
        paths::project_communication_sessions_dir()
    }

    fn get_legacy_base_paths() -> Vec<PathBuf> {
        vec![
            paths::project_alius_dir().join("sessions"),
            paths::global_alius_dir().join("sessions"),
        ]
    }

    /// Get the messages file path for a session
    fn messages_path(&self, session_id: &SessionId) -> PathBuf {
        self.base_path
            .join(session_id.as_str())
            .join("messages.jsonl")
    }

    fn readable_messages_path(&self, session_id: &SessionId) -> PathBuf {
        let primary = self.messages_path(session_id);
        if primary.exists() {
            return primary;
        }

        for base_path in &self.legacy_base_paths {
            let legacy = base_path.join(session_id.as_str()).join("messages.jsonl");
            if legacy.exists() {
                return legacy;
            }
        }

        primary
    }

    fn all_messages_paths(&self, session_id: &SessionId) -> Vec<PathBuf> {
        let mut paths = vec![self.messages_path(session_id)];
        paths.extend(
            self.legacy_base_paths
                .iter()
                .map(|base| base.join(session_id.as_str()).join("messages.jsonl")),
        );
        paths
    }

    /// Save all messages for a session
    pub fn save_messages(&self, session_id: &SessionId, messages: &[Message]) -> Result<()> {
        let messages_path = self.messages_path(session_id);
        if let Some(parent) = messages_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
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
        let messages_path = self.readable_messages_path(session_id);
        if !messages_path.exists() {
            return Ok(Vec::new());
        }

        let content = std::fs::read_to_string(messages_path)?;
        let messages: Vec<Message> = content
            .lines()
            .filter(|line| !line.is_empty())
            .map(serde_json::from_str)
            .collect::<Result<Vec<Message>, _>>()?;

        Ok(messages)
    }

    /// Append a single message (append-only)
    pub fn append_message(&self, session_id: &SessionId, message: &Message) -> Result<()> {
        let messages_path = self.messages_path(session_id);
        if let Some(parent) = messages_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let line = serde_json::to_string(message)?;
        std::fs::write(&messages_path, format!("{}\n", line))?;
        Ok(())
    }

    /// Clear all messages for a session
    pub fn clear_messages(&self, session_id: &SessionId) -> Result<()> {
        for messages_path in self.all_messages_paths(session_id) {
            if messages_path.exists() {
                std::fs::remove_file(messages_path)?;
            }
        }
        Ok(())
    }
}

impl Default for ConversationStore {
    fn default() -> Self {
        Self::new().expect("Failed to create conversation store")
    }
}
