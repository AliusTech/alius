//! Conversation management

use alius_protocol::Message;

/// Conversation with context window management
pub struct Conversation {
    messages: Vec<Message>,
    system_prompt: Option<String>,
    max_tokens: usize,
    summary: Option<String>,
}

impl Conversation {
    /// Create a new conversation
    pub fn new(system_prompt: Option<String>) -> Self {
        Self {
            messages: Vec::new(),
            system_prompt,
            max_tokens: 4096, // Default context window
            summary: None,
        }
    }

    /// Create a conversation with custom context window
    pub fn with_max_tokens(system_prompt: Option<String>, max_tokens: usize) -> Self {
        Self {
            messages: Vec::new(),
            system_prompt,
            max_tokens,
            summary: None,
        }
    }

    /// Add a user message
    pub fn add_user_message(&mut self, text: String) {
        self.messages.push(Message::new_user(text));
    }

    /// Add an assistant message
    pub fn add_assistant_message(&mut self, text: String) {
        self.messages.push(Message::new_assistant(text));
    }

    /// Get all messages
    pub fn messages(&self) -> &[Message] {
        &self.messages
    }

    /// Get system prompt
    pub fn system_prompt(&self) -> Option<&str> {
        self.system_prompt.as_deref()
    }

    /// Set system prompt
    pub fn set_system_prompt(&mut self, prompt: String) {
        self.system_prompt = Some(prompt);
    }

    /// Get summary of old messages
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    /// Set summary
    pub fn set_summary(&mut self, summary: String) {
        self.summary = Some(summary);
    }

    /// Check if context window is near limit (approximate)
    pub fn needs_summarization(&self) -> bool {
        // Simple estimation: ~4 chars per token
        let total_chars: usize = self.messages.iter().map(|m| m.content.len()).sum();
        let estimated_tokens = total_chars / 4;
        estimated_tokens > self.max_tokens / 2 // Trigger at half capacity
    }

    /// Get messages that should be summarized (older than keep_recent)
    pub fn messages_to_summarize(&self, keep_recent: usize) -> Vec<Message> {
        if self.messages.len() <= keep_recent {
            return Vec::new();
        }
        self.messages[..self.messages.len() - keep_recent].to_vec()
    }

    /// Truncate old messages after summarization
    pub fn truncate_with_summary(&mut self, summary: String, keep_recent: usize) {
        if self.messages.len() > keep_recent {
            self.messages = self.messages[self.messages.len() - keep_recent..].to_vec();
            // Insert summary at the beginning
            self.messages.insert(0, Message::new_summary(summary));
        }
    }

    /// Clear all messages
    pub fn clear(&mut self) {
        self.messages.clear();
        self.summary = None;
    }

    /// Create a conversation from existing messages (e.g. loaded from store).
    pub fn from_messages(system_prompt: Option<String>, messages: Vec<Message>) -> Self {
        Self {
            messages,
            system_prompt,
            max_tokens: 4096,
            summary: None,
        }
    }

    /// Get message count
    pub fn len(&self) -> usize {
        self.messages.len()
    }

    /// Check if empty
    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }
}

impl Default for Conversation {
    fn default() -> Self {
        Self::new(None)
    }
}