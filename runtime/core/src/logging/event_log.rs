//! Append-only event log entry for audit and traceability.

use serde::{Deserialize, Serialize};

/// An immutable event log entry written to `event-log.jsonl`.
///
/// Each entry records a significant runtime event (tool invocation, permission
/// decision, model selection, etc.) and is never mutated after being written.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLogEntry {
    /// ISO 8601 timestamp (UTC).
    pub timestamp: String,
    /// Event category (e.g. `model_selected`, `tool_invoked`, `permission_decision`).
    pub event_type: String,
    /// Workspace this event belongs to.
    pub workspace_ref: String,
    /// Session this event belongs to.
    pub session_ref: String,
    /// Run this event belongs to.
    pub run_ref: String,
    /// Cross-log correlation trace ID.
    pub trace_id: String,
    /// Arbitrary structured payload for this event.
    pub data: serde_json::Value,
}

impl EventLogEntry {
    /// Create a new EventLogEntry with the current UTC timestamp.
    pub fn new(
        event_type: impl Into<String>,
        trace_id: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            event_type: event_type.into(),
            workspace_ref: String::new(),
            session_ref: String::new(),
            run_ref: String::new(),
            trace_id: trace_id.into(),
            data,
        }
    }

    /// Set workspace reference.
    pub fn with_workspace_ref(mut self, v: impl Into<String>) -> Self {
        self.workspace_ref = v.into();
        self
    }

    /// Set session reference.
    pub fn with_session_ref(mut self, v: impl Into<String>) -> Self {
        self.session_ref = v.into();
        self
    }

    /// Set run reference.
    pub fn with_run_ref(mut self, v: impl Into<String>) -> Self {
        self.run_ref = v.into();
        self
    }
}
