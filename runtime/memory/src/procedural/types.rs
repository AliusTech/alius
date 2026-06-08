//! Procedural memory types.

use serde::{Deserialize, Serialize};

/// A stored procedure (workflow or known-good approach).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Procedure {
    /// Unique procedure ID.
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Step-by-step instructions (JSON).
    pub steps: serde_json::Value,
    /// Scope (e.g. "workspace", "global").
    pub scope: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}

/// A search hit from procedural memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcedureHit {
    /// Matched procedure.
    pub procedure: Procedure,
    /// Relevance score.
    pub score: f64,
}

/// A recorded failure pattern with resolution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailurePattern {
    /// Unique ID.
    pub id: String,
    /// Symptoms that characterize this failure (JSON).
    pub symptoms: serde_json::Value,
    /// How the failure was resolved.
    pub resolution: String,
    /// ISO 8601 creation timestamp.
    pub created_at: String,
}
