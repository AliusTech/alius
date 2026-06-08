//! JSONL file writer with broadcast subscription and sensitive data redaction.

use std::io::Write;
use std::path::Path;

use super::error::LoggingError;
use super::event_log::EventLogEntry;
use super::log_query::LogQuery;
use super::log_record::LogRecord;
use super::LogLevel;

const BROADCAST_CAPACITY: usize = 256;

/// Sensitive patterns for redaction.
const REDACT_PATTERNS: &[&str] = &[
    r#"sk-[a-zA-Z0-9]{20,}"#,
    r#"key-[a-zA-Z0-9]{20,}"#,
    r#"Bearer\s+[a-zA-Z0-9\._\-]{20,}"#,
    r#"(?i)(api[_-]?key|token|secret|password|credential)\s*[=:]\s*["']?[a-zA-Z0-9\._\-]{8,}["']?"#,
    r#"[0-9a-f]{40,}"#,
];

/// Manages JSONL log files, real-time broadcast, and query retrieval.
pub struct LogWriter {
    runtime_log: std::fs::File,
    error_log: std::fs::File,
    event_log: std::fs::File,
    log_dir: std::path::PathBuf,
    tx: tokio::sync::broadcast::Sender<LogRecord>,
    redact_enabled: bool,
}

impl LogWriter {
    /// Create a new LogWriter, creating the log directory if needed.
    ///
    /// Opens (or creates) three JSONL files:
    /// - `runtime.log.jsonl` — all log records
    /// - `error.log.jsonl` — error-level records only
    /// - `event-log.jsonl` — structured event entries
    pub fn new(log_dir: &Path) -> Result<Self, LoggingError> {
        Self::with_redaction(log_dir, true)
    }

    /// Create a new LogWriter with explicit redaction control.
    pub fn with_redaction(log_dir: &Path, redact_enabled: bool) -> Result<Self, LoggingError> {
        std::fs::create_dir_all(log_dir)
            .map_err(|_| LoggingError::DirectoryNotFound(log_dir.to_path_buf()))?;

        let log_dir = log_dir.to_path_buf();

        let runtime_path = log_dir.join("runtime.log.jsonl");
        let error_path = log_dir.join("error.log.jsonl");
        let event_path = log_dir.join("event-log.jsonl");

        let runtime_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&runtime_path)
            .map_err(LoggingError::Io)?;

        let error_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&error_path)
            .map_err(LoggingError::Io)?;

        let event_log = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&event_path)
            .map_err(LoggingError::Io)?;

        let (tx, _) = tokio::sync::broadcast::channel(BROADCAST_CAPACITY);

        Ok(Self {
            runtime_log,
            error_log,
            event_log,
            log_dir,
            tx,
            redact_enabled,
        })
    }

    /// Emit a log record.
    ///
    /// Writes to `runtime.log.jsonl` (all levels) and `error.log.jsonl`
    /// (error level only), then broadcasts to live subscribers.
    pub fn emit(&mut self, record: LogRecord) -> Result<(), LoggingError> {
        let message = if self.redact_enabled {
            Self::redact(&record.message)
        } else {
            record.message.clone()
        };

        let mut record = record;
        record.message = message;

        let line = serde_json::to_string(&record).map_err(LoggingError::Serialization)?;
        let line_with_newline = format!("{}\n", line);

        // Write to runtime log (all levels).
        self.runtime_log
            .write_all(line_with_newline.as_bytes())
            .map_err(LoggingError::Io)?;

        // Write to error log (error level only).
        if record.level == LogLevel::Error {
            self.error_log
                .write_all(line_with_newline.as_bytes())
                .map_err(LoggingError::Io)?;
        }

        // Broadcast — drop old messages if channel is full.
        let _ = self.tx.send(record);

        Ok(())
    }

    /// Append an event log entry to the immutable `event-log.jsonl`.
    pub fn append_event(&mut self, entry: EventLogEntry) -> Result<(), LoggingError> {
        let line = serde_json::to_string(&entry).map_err(LoggingError::Serialization)?;
        let line_with_newline = format!("{}\n", line);
        self.event_log
            .write_all(line_with_newline.as_bytes())
            .map_err(LoggingError::Io)?;
        Ok(())
    }

    /// Flush all file buffers to disk.
    pub fn flush(&mut self) -> Result<(), LoggingError> {
        self.runtime_log.flush().map_err(LoggingError::Io)?;
        self.error_log.flush().map_err(LoggingError::Io)?;
        self.event_log.flush().map_err(LoggingError::Io)?;
        Ok(())
    }

    /// Subscribe to real-time log records.
    pub fn subscribe(&self) -> tokio::sync::broadcast::Receiver<LogRecord> {
        self.tx.subscribe()
    }

    /// Query historical log records from `runtime.log.jsonl`.
    pub fn query(&self, query: &LogQuery) -> Result<Vec<LogRecord>, LoggingError> {
        let path = self.log_dir.join("runtime.log.jsonl");
        let content = std::fs::read_to_string(&path)?;
        let mut results: Vec<LogRecord> = Vec::new();

        for line in content.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(record) = serde_json::from_str::<LogRecord>(line) {
                if query.matches(&record) {
                    results.push(record);
                    if let Some(limit) = query.limit {
                        if results.len() >= limit {
                            break;
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Redact sensitive patterns from a string.
    ///
    /// Patterns removed: API keys (`sk-*`, `key-*`), Bearer tokens,
    /// key=value assignments with long hex, and long hex strings.
    pub fn redact(input: &str) -> String {
        let mut result = input.to_string();
        for pattern in REDACT_PATTERNS {
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, "[REDACTED]").to_string();
            }
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_writer(dir: &Path) -> LogWriter {
        LogWriter::new(dir).expect("LogWriter creation failed")
    }

    #[test]
    fn test_emit_writes_to_runtime_log() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        let record = LogRecord::new(LogLevel::Info, "test_module", "hello world");
        writer.emit(record).unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("runtime.log.jsonl")).unwrap();
        assert!(content.contains("hello world"));
        assert!(content.contains("Info"));
    }

    #[test]
    fn test_emit_error_writes_to_error_log() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        let record = LogRecord::new(LogLevel::Error, "test_module", "something broke");
        writer.emit(record).unwrap();
        writer.flush().unwrap();

        let runtime = std::fs::read_to_string(dir.path().join("runtime.log.jsonl")).unwrap();
        let error = std::fs::read_to_string(dir.path().join("error.log.jsonl")).unwrap();
        assert!(runtime.contains("something broke"));
        assert!(error.contains("something broke"));
    }

    #[test]
    fn test_emit_info_does_not_write_to_error_log() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        let record = LogRecord::new(LogLevel::Info, "test_module", "just info");
        writer.emit(record).unwrap();
        writer.flush().unwrap();

        let error = std::fs::read_to_string(dir.path().join("error.log.jsonl")).unwrap();
        assert!(error.is_empty());
    }

    #[test]
    fn test_append_event_writes_to_event_log() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        let entry = EventLogEntry::new(
            "tool_invoked",
            "trace-123",
            serde_json::json!({"tool": "read_file"}),
        );
        writer.append_event(entry).unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("event-log.jsonl")).unwrap();
        assert!(content.contains("tool_invoked"));
        assert!(content.contains("trace-123"));
    }

    #[test]
    fn test_redact_strips_api_keys() {
        let input = "Using key sk-abc123def456ghi789jkl012mno345 for auth";
        let redacted = LogWriter::redact(input);
        assert!(!redacted.contains("sk-abc123"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_strips_bearer_tokens() {
        let input = "Authorization: Bearer eyJhbGciOiJIUzI1NiJ9.longtoken123456";
        let redacted = LogWriter::redact(input);
        assert!(!redacted.contains("eyJhbGciOi"));
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_redact_strips_key_assignments() {
        let input = r#"api_key = "abcdef1234567890abcdef""#;
        let redacted = LogWriter::redact(input);
        assert!(redacted.contains("[REDACTED]"));
    }

    #[test]
    fn test_flush_persists_to_disk() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        writer
            .emit(LogRecord::new(LogLevel::Info, "test", "flush test"))
            .unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("runtime.log.jsonl")).unwrap();
        assert!(!content.is_empty());
    }

    #[test]
    fn test_log_record_includes_trace_id() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        let record =
            LogRecord::new(LogLevel::Info, "test", "with trace").with_trace_id("trace-abc-123");
        writer.emit(record).unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("runtime.log.jsonl")).unwrap();
        assert!(content.contains("trace-abc-123"));
    }

    #[test]
    fn test_log_directory_created_on_init() {
        let dir = TempDir::new().unwrap();
        let log_dir = dir.path().join("deep").join("nested").join("logs");
        let writer = LogWriter::new(&log_dir);
        assert!(writer.is_ok());
        assert!(log_dir.exists());
    }

    #[test]
    fn test_concurrent_writes_safe() {
        use std::sync::{Arc, Mutex};

        let dir = TempDir::new().unwrap();
        let writer = Arc::new(Mutex::new(make_writer(dir.path())));
        let mut handles = vec![];

        for i in 0..10 {
            let w = Arc::clone(&writer);
            handles.push(std::thread::spawn(move || {
                let mut w = w.lock().unwrap();
                w.emit(LogRecord::new(
                    LogLevel::Info,
                    "concurrent",
                    format!("msg-{}", i),
                ))
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        writer.lock().unwrap().flush().unwrap();
        let content = std::fs::read_to_string(dir.path().join("runtime.log.jsonl")).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn test_query_by_session_ref() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        writer
            .emit(LogRecord::new(LogLevel::Info, "t", "s1-msg").with_session_ref("session-1"))
            .unwrap();
        writer
            .emit(LogRecord::new(LogLevel::Info, "t", "s2-msg").with_session_ref("session-2"))
            .unwrap();
        writer.flush().unwrap();

        let results = writer
            .query(&LogQuery::new().with_session_ref("session-1"))
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "s1-msg");
    }

    #[test]
    fn test_query_by_level() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        writer
            .emit(LogRecord::new(LogLevel::Info, "t", "info-msg"))
            .unwrap();
        writer
            .emit(LogRecord::new(LogLevel::Error, "t", "error-msg"))
            .unwrap();
        writer.flush().unwrap();

        let results = writer
            .query(&LogQuery::new().with_min_level(LogLevel::Error))
            .unwrap();
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].message, "error-msg");
    }

    #[test]
    fn test_query_returns_empty_for_no_match() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        writer
            .emit(LogRecord::new(LogLevel::Info, "t", "msg"))
            .unwrap();
        writer.flush().unwrap();

        let results = writer
            .query(&LogQuery::new().with_trace_id("nonexistent"))
            .unwrap();
        assert!(results.is_empty());
    }

    #[test]
    fn test_subscribe_receives_events() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        let mut rx = writer.subscribe();

        writer
            .emit(LogRecord::new(LogLevel::Info, "test", "live msg"))
            .unwrap();

        let received = rx.try_recv();
        assert!(received.is_ok());
        assert_eq!(received.unwrap().message, "live msg");
    }
}
