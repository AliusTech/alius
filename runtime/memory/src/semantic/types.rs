//! Semantic memory types.

use serde::{Deserialize, Serialize};

/// A factual piece of knowledge stored in semantic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticFact {
    /// Unique fact ID.
    pub id: String,
    /// The fact content.
    pub content: String,
    /// Scope (e.g. "workspace", "global", "project").
    pub scope: String,
    /// Confidence score (0.0 - 1.0).
    pub confidence: f64,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

/// A search hit from any memory layer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryHit {
    /// The matched content.
    pub content: String,
    /// Relevance score (0.0 - 1.0).
    pub score: f64,
    /// Which memory layer produced this hit.
    pub memory_type: String,
    /// Optional source reference (file path, fact ID, etc.).
    pub source: Option<String>,
}
