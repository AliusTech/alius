//! Memory Bridge — integration layer between memory and conversation flow.
//!
//! Provides context retrieval (before model call) and
//! write-back (storing important events after model response).
//!
//! Usage:
//! 1. Before sending to LLM: `let context = bridge.retrieve_context(&user_message)?;`
//! 2. Inject context into conversation (caller's responsibility)
//! 3. After receiving response: `bridge.record_interaction(&user_message, &response)?;`

use anyhow::Result;
use std::sync::Arc;

use crate::semantic::SemanticStore;

/// Memory bridge for conversation integration.
///
/// Holds a reference to `SemanticStore` for both retrieval (keyword search)
/// and write-back (recording interactions). Does not require `RetrievalEngine`
/// since the bridge operates directly on the semantic store.
pub struct MemoryBridge {
    semantic: Arc<SemanticStore>,
    /// Maximum number of memory hits to inject into context.
    max_context_hits: usize,
    /// Minimum score threshold for including a memory hit.
    min_score: f64,
}

impl MemoryBridge {
    /// Create a new memory bridge with the given semantic store.
    pub fn new(semantic: Arc<SemanticStore>) -> Self {
        Self {
            semantic,
            max_context_hits: 5,
            min_score: 0.1,
        }
    }

    /// Set the maximum number of context hits to inject.
    pub fn with_max_hits(mut self, max: usize) -> Self {
        self.max_context_hits = max;
        self
    }

    /// Set the minimum score threshold for including hits.
    pub fn with_min_score(mut self, score: f64) -> Self {
        self.min_score = score;
        self
    }

    /// Retrieve relevant memories for a user message.
    ///
    /// Returns formatted context string that can be prepended to the
    /// conversation or injected as a system message. Returns empty string
    /// if no relevant memories are found.
    pub fn retrieve_context(&self, user_message: &str) -> Result<String> {
        let hits = self
            .semantic
            .keyword_search(user_message, self.max_context_hits)?;

        // Filter by minimum score
        let hits: Vec<_> = hits
            .into_iter()
            .filter(|h| h.score >= self.min_score)
            .collect();

        if hits.is_empty() {
            return Ok(String::new());
        }

        let mut context = String::from("## Relevant Memory\n\n");
        for hit in &hits {
            context.push_str(&format!(
                "- [{}] {} (score: {:.2})\n",
                hit.memory_type, hit.content, hit.score
            ));
        }

        Ok(context)
    }

    /// Record an interaction for future retrieval.
    ///
    /// Stores a summary of the user message and response in semantic memory
    /// so it can be retrieved in future conversations.
    pub fn record_interaction(&self, user_message: &str, response: &str) -> Result<()> {
        let summary = format!("User asked: {}. Response: {}", user_message, response);
        let truncated = if summary.len() > 1000 {
            &summary[..1000]
        } else {
            &summary
        };

        self.semantic.upsert_fact(truncated, "conversation")?;
        Ok(())
    }
}

impl std::fmt::Debug for MemoryBridge {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MemoryBridge")
            .field("max_context_hits", &self.max_context_hits)
            .field("min_score", &self.min_score)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bridge_retrieve_context_empty() {
        let sem = Arc::new(SemanticStore::open_in_memory().unwrap());
        let bridge = MemoryBridge::new(sem);
        let ctx = bridge.retrieve_context("test query").unwrap();
        assert!(ctx.is_empty());
    }

    #[test]
    fn test_bridge_retrieve_context_with_hits() {
        let sem = Arc::new(SemanticStore::open_in_memory().unwrap());
        sem.upsert_fact("Rust is a systems programming language", "workspace")
            .unwrap();
        sem.upsert_fact("Python is a scripting language", "workspace")
            .unwrap();

        let bridge = MemoryBridge::new(sem);
        let ctx = bridge.retrieve_context("Rust").unwrap();

        assert!(!ctx.is_empty());
        assert!(ctx.contains("Rust"));
        assert!(ctx.contains("Relevant Memory"));
    }

    #[test]
    fn test_bridge_max_hits_respected() {
        let sem = Arc::new(SemanticStore::open_in_memory().unwrap());
        for i in 0..10 {
            sem.upsert_fact(&format!("fact {}", i), "workspace")
                .unwrap();
        }

        let bridge = MemoryBridge::new(sem).with_max_hits(3);
        let ctx = bridge.retrieve_context("fact").unwrap();

        let count = ctx.lines().filter(|l| l.starts_with("- ")).count();
        assert!(count <= 3, "should respect max_hits: got {}", count);
    }

    #[test]
    fn test_bridge_min_score_filters() {
        let sem = Arc::new(SemanticStore::open_in_memory().unwrap());
        sem.upsert_fact("completely unrelated content", "workspace")
            .unwrap();

        let bridge = MemoryBridge::new(sem).with_min_score(0.99);
        let ctx = bridge.retrieve_context("quantum physics").unwrap();

        assert!(ctx.is_empty() || !ctx.contains("unrelated"));
    }

    #[test]
    fn test_bridge_record_interaction() {
        let sem = Arc::new(SemanticStore::open_in_memory().unwrap());
        let bridge = MemoryBridge::new(sem.clone());

        bridge
            .record_interaction("What is Rust?", "Rust is a systems language")
            .unwrap();

        // Should be retrievable
        let bridge2 = MemoryBridge::new(sem);
        let ctx = bridge2.retrieve_context("Rust").unwrap();
        assert!(
            !ctx.is_empty(),
            "recorded interaction should be retrievable"
        );
    }
}
