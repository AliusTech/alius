//! Audit sink for WASM host function calls.
//!
//! Every host import call (allow or deny) emits an audit event through
//! [`HostAuditSink`]. The default implementation logs via `tracing::info!`.
//!
//! **Security invariants:**
//! - File content, env values, shell stdout/stderr are NEVER recorded.
//! - Sensitive args (passwords, tokens) are redacted.
//! - Sink failures are non-blocking: a diagnostic is logged but execution
//!   continues (deny or allow is determined by the permission matcher, not
//!   the audit sink).

use std::fmt;

/// Audit event emitted for every host function call.
#[derive(Debug, Clone)]
pub struct HostAuditEvent {
    /// Trace ID linking this call to a broader execution context.
    pub trace_id: String,
    /// Plugin that initiated the call.
    pub plugin_id: String,
    /// Host function name: `read_file`, `write_file`, `list_dir`, `env_get`, `shell`, `fetch`.
    pub action: String,
    /// Target resource (path, URL, variable name, or command base).
    /// Sensitive values are redacted before reaching the sink.
    pub target: String,
    /// Whether the operation was allowed.
    pub allowed: bool,
    /// Reason for deny, or "ok" for allow.
    pub reason: String,
    /// Unix timestamp in milliseconds.
    pub ts: u64,
}

impl fmt::Display for HostAuditEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "[audit] plugin={} action={} target={} allowed={} reason={} trace={} ts={}",
            self.plugin_id,
            self.action,
            self.target,
            self.allowed,
            self.reason,
            self.trace_id,
            self.ts
        )
    }
}

/// Trait for receiving audit events.
///
/// Implementations must be `Send + Sync` because host imports execute in
/// wasmtime's async context which may cross thread boundaries.
pub trait HostAuditSink: Send + Sync {
    fn emit(&self, event: HostAuditEvent);
}

/// Default audit sink that logs events via `tracing`.
pub struct TracingAuditSink;

impl HostAuditSink for TracingAuditSink {
    fn emit(&self, event: HostAuditEvent) {
        if event.allowed {
            tracing::info!(
                plugin = %event.plugin_id,
                action = %event.action,
                target = %event.target,
                trace = %event.trace_id,
                ts = event.ts,
                "host call allowed"
            );
        } else {
            tracing::warn!(
                plugin = %event.plugin_id,
                action = %event.action,
                target = %event.target,
                reason = %event.reason,
                trace = %event.trace_id,
                ts = event.ts,
                "host call denied"
            );
        }
    }
}

/// No-op audit sink for testing.
pub struct NoopAuditSink;

impl HostAuditSink for NoopAuditSink {
    fn emit(&self, _event: HostAuditEvent) {}
}

/// Helper to create a [`HostAuditEvent`] with the current timestamp.
pub fn audit_event(
    trace_id: &str,
    plugin_id: &str,
    action: &str,
    target: &str,
    allowed: bool,
    reason: &str,
) -> HostAuditEvent {
    HostAuditEvent {
        trace_id: trace_id.to_string(),
        plugin_id: plugin_id.to_string(),
        action: action.to_string(),
        target: target.to_string(),
        allowed,
        reason: reason.to_string(),
        ts: epoch_millis(),
    }
}

fn epoch_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    struct RecordingSink {
        events: Arc<Mutex<Vec<HostAuditEvent>>>,
    }

    impl RecordingSink {
        fn new() -> (Self, Arc<Mutex<Vec<HostAuditEvent>>>) {
            let events = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    events: Arc::clone(&events),
                },
                events,
            )
        }
    }

    impl HostAuditSink for RecordingSink {
        fn emit(&self, event: HostAuditEvent) {
            self.events.lock().unwrap().push(event);
        }
    }

    #[test]
    fn test_audit_event_creation() {
        let event = audit_event("tr-1", "plug-1", "read_file", "src/main.rs", true, "ok");
        assert_eq!(event.trace_id, "tr-1");
        assert_eq!(event.plugin_id, "plug-1");
        assert_eq!(event.action, "read_file");
        assert_eq!(event.target, "src/main.rs");
        assert!(event.allowed);
        assert_eq!(event.reason, "ok");
        assert!(event.ts > 0);
    }

    #[test]
    fn test_recording_sink_captures_events() {
        let (sink, events) = RecordingSink::new();
        let event = audit_event("tr-1", "plug-1", "read_file", "src/main.rs", true, "ok");
        sink.emit(event);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].action, "read_file");
        assert!(events[0].allowed);
    }

    #[test]
    fn test_recording_sink_captures_denied_events() {
        let (sink, events) = RecordingSink::new();
        let event = audit_event(
            "tr-2",
            "plug-1",
            "write_file",
            "/etc/passwd",
            false,
            "absolute paths are not allowed",
        );
        sink.emit(event);

        let events = events.lock().unwrap();
        assert_eq!(events.len(), 1);
        assert!(!events[0].allowed);
        assert!(events[0].reason.contains("absolute"));
    }

    #[test]
    fn test_noop_sink_does_not_panic() {
        let sink = NoopAuditSink;
        let event = audit_event("tr-1", "plug-1", "shell", "rm -rf /", false, "denied");
        sink.emit(event);
    }

    #[test]
    fn test_display_format() {
        let event = audit_event("tr-1", "plug-1", "read_file", "src/main.rs", true, "ok");
        let display = format!("{event}");
        assert!(display.contains("plugin=plug-1"));
        assert!(display.contains("action=read_file"));
        assert!(display.contains("allowed=true"));
    }
}
