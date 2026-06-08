//! Context window management for the loop engine.
//!
//! Tracks token usage and triggers truncation when the conversation
//! exceeds the configured context window budget.

use runtime_model::Conversation;

/// Result of a truncation operation.
pub struct TruncationResult {
    pub truncated_count: usize,
    pub estimated_tokens_saved: usize,
}

/// Manages context window budget for a loop run.
pub struct ContextManager {
    max_tokens: usize,
    keep_recent: usize,
}

impl ContextManager {
    pub fn new(max_tokens: usize) -> Self {
        Self {
            max_tokens,
            keep_recent: 10,
        }
    }

    pub fn with_keep_recent(mut self, count: usize) -> Self {
        self.keep_recent = count;
        self
    }

    /// Estimate token count for the conversation using tiktoken.
    pub fn estimate_tokens(&self, conversation: &Conversation) -> usize {
        let encoder =
            tiktoken::get_encoding("cl100k_base").or_else(|| tiktoken::get_encoding("o200k_base"));

        match encoder {
            Some(enc) => {
                let mut total = 0;
                if let Some(sp) = conversation.system_prompt() {
                    total += enc.count(sp);
                }
                for msg in conversation.messages() {
                    total += enc.count(&msg.content);
                }
                total
            }
            None => {
                // Fallback: rough char-based estimation (~4 chars per token)
                let mut total_chars: usize = 0;
                if let Some(sp) = conversation.system_prompt() {
                    total_chars += sp.len();
                }
                for msg in conversation.messages() {
                    total_chars += msg.content.len();
                }
                total_chars / 4
            }
        }
    }

    /// Check if the conversation exceeds the token budget.
    pub fn needs_truncation(&self, conversation: &Conversation) -> bool {
        self.estimate_tokens(conversation) > self.max_tokens / 2
    }

    /// Truncate older messages, keeping `keep_recent` most recent.
    /// Returns the number of messages removed and estimated tokens saved.
    pub fn truncate(&self, conversation: &mut Conversation) -> TruncationResult {
        let tokens_before = self.estimate_tokens(conversation);
        let messages_to_keep = self.keep_recent;

        let msgs_to_summarize = conversation.messages_to_summarize(messages_to_keep);
        let truncated_count = msgs_to_summarize.len();

        if truncated_count > 0 {
            let summary_parts: Vec<String> = msgs_to_summarize
                .iter()
                .map(|m| format!("[{:?}] {}", m.role, m.content))
                .collect();
            let summary = format!(
                "Summary of earlier conversation:\n{}",
                summary_parts.join("\n")
            );
            conversation.truncate_with_summary(summary, messages_to_keep);
        }

        let tokens_after = self.estimate_tokens(conversation);

        TruncationResult {
            truncated_count,
            estimated_tokens_saved: tokens_before.saturating_sub(tokens_after),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn estimate_tokens_counts_system_and_messages() {
        let mut conv = Conversation::new(Some("You are helpful.".to_string()));
        conv.add_user_message("Hello world".to_string());
        conv.add_assistant_message("Hi there!".to_string());

        let mgr = ContextManager::new(4096);
        let tokens = mgr.estimate_tokens(&conv);
        assert!(tokens > 0);
    }

    #[test]
    fn needs_truncation_returns_false_for_small_conversation() {
        let mut conv = Conversation::new(None);
        conv.add_user_message("short message".to_string());

        let mgr = ContextManager::new(4096);
        assert!(!mgr.needs_truncation(&conv));
    }

    #[test]
    fn truncate_removes_old_messages() {
        let mut conv = Conversation::new(None);
        for i in 0..20 {
            conv.add_user_message(format!("message {}", i));
            conv.add_assistant_message(format!("response {}", i));
        }

        let mgr = ContextManager::new(100).with_keep_recent(4);
        assert!(mgr.needs_truncation(&conv));

        let result = mgr.truncate(&mut conv);
        assert!(result.truncated_count > 0);
        assert!(conv.messages().len() < 40);
    }
}
