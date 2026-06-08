//! Log record and severity level definitions.

use serde::{Deserialize, Serialize};

/// Log severity level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "TRACE"),
            LogLevel::Debug => write!(f, "DEBUG"),
            LogLevel::Info => write!(f, "INFO"),
            LogLevel::Warn => write!(f, "WARN"),
            LogLevel::Error => write!(f, "ERROR"),
        }
    }
}

/// A structured log record written to JSONL files.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LogRecord {
    /// ISO 8601 timestamp (UTC).
    pub timestamp: String,
    /// Log severity level.
    pub level: LogLevel,
    /// Module path that emitted this record (e.g. `core_runtime::loop_engine`).
    pub target: String,
    /// Human-readable log message.
    pub message: String,
    /// Optional workspace reference for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub workspace_ref: Option<String>,
    /// Optional session reference for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_ref: Option<String>,
    /// Optional run reference for correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub run_ref: Option<String>,
    /// Optional trace ID for cross-log correlation.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,
    /// Additional structured key-value pairs.
    #[serde(default)]
    pub fields: serde_json::Map<String, serde_json::Value>,
}

impl LogRecord {
    /// Create a new LogRecord with the current UTC timestamp.
    pub fn new(level: LogLevel, target: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            timestamp: chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
            level,
            target: target.into(),
            message: message.into(),
            workspace_ref: None,
            session_ref: None,
            run_ref: None,
            trace_id: None,
            fields: serde_json::Map::new(),
        }
    }

    /// Set workspace reference.
    pub fn with_workspace_ref(mut self, v: impl Into<String>) -> Self {
        self.workspace_ref = Some(v.into());
        self
    }

    /// Set session reference.
    pub fn with_session_ref(mut self, v: impl Into<String>) -> Self {
        self.session_ref = Some(v.into());
        self
    }

    /// Set run reference.
    pub fn with_run_ref(mut self, v: impl Into<String>) -> Self {
        self.run_ref = Some(v.into());
        self
    }

    /// Set trace ID.
    pub fn with_trace_id(mut self, v: impl Into<String>) -> Self {
        self.trace_id = Some(v.into());
        self
    }

    /// Add an extra field.
    pub fn with_field(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.fields.insert(key.into(), value);
        self
    }
}

impl Default for LogRecord {
    fn default() -> Self {
        Self::new(LogLevel::Info, "", "")
    }
}
