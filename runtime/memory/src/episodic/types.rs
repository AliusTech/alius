//! Episodic memory types.

use serde::{Deserialize, Serialize};

/// A core event recorded in episodic memory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreEvent {
    /// Unique event ID.
    pub id: String,
    /// Cross-log correlation trace ID.
    pub trace_id: String,
    /// Session this event belongs to.
    pub session_id: String,
    /// Run this event belongs to.
    pub run_id: String,
    /// Event type (e.g. "turn_started", "tool_called", "model_response").
    pub event_type: String,
    /// Structured event payload.
    pub data: serde_json::Value,
    /// ISO 8601 timestamp.
    pub created_at: String,
}

impl CoreEvent {
    /// Create a new CoreEvent with auto-generated ID and timestamp.
    pub fn new(
        trace_id: impl Into<String>,
        session_id: impl Into<String>,
        run_id: impl Into<String>,
        event_type: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            trace_id: trace_id.into(),
            session_id: session_id.into(),
            run_id: run_id.into(),
            event_type: event_type.into(),
            data,
            created_at: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
        }
    }
}
