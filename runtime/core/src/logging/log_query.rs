//! Query filter for log retrieval.

use super::LogLevel;

/// Filter parameters for querying log records from JSONL files.
#[derive(Debug, Clone, Default)]
pub struct LogQuery {
    /// Filter by workspace reference.
    pub workspace_ref: Option<String>,
    /// Filter by session reference.
    pub session_ref: Option<String>,
    /// Filter by run reference.
    pub run_ref: Option<String>,
    /// Filter by trace ID.
    pub trace_id: Option<String>,
    /// Filter by minimum log level (inclusive).
    pub min_level: Option<LogLevel>,
    /// Maximum number of records to return.
    pub limit: Option<usize>,
}

impl LogQuery {
    /// Create an empty query (matches everything).
    pub fn new() -> Self {
        Self::default()
    }

    /// Filter by session reference.
    pub fn with_session_ref(mut self, v: impl Into<String>) -> Self {
        self.session_ref = Some(v.into());
        self
    }

    /// Filter by trace ID.
    pub fn with_trace_id(mut self, v: impl Into<String>) -> Self {
        self.trace_id = Some(v.into());
        self
    }

    /// Filter by minimum log level.
    pub fn with_min_level(mut self, level: LogLevel) -> Self {
        self.min_level = Some(level);
        self
    }

    /// Limit the number of results.
    pub fn with_limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }

    /// Check whether a log record matches this query.
    pub fn matches(&self, record: &super::LogRecord) -> bool {
        if let Some(ref ws) = self.workspace_ref {
            if record.workspace_ref.as_deref() != Some(ws.as_str()) {
                return false;
            }
        }
        if let Some(ref sr) = self.session_ref {
            if record.session_ref.as_deref() != Some(sr.as_str()) {
                return false;
            }
        }
        if let Some(ref rr) = self.run_ref {
            if record.run_ref.as_deref() != Some(rr.as_str()) {
                return false;
            }
        }
        if let Some(ref tid) = self.trace_id {
            if record.trace_id.as_deref() != Some(tid.as_str()) {
                return false;
            }
        }
        if let Some(min) = self.min_level {
            if record.level < min {
                return false;
            }
        }
        true
    }
}
