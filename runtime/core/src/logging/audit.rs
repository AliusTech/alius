//! Audit logging for permission decisions, tool invocations, and shell gate actions.

use super::error::LoggingError;
use super::event_log::EventLogEntry;
use super::log_writer::LogWriter;

/// Log a permission decision to the event log.
pub fn log_permission_decision(
    writer: &mut LogWriter,
    decision: &str,
    resource: &str,
    trace_id: &str,
) -> Result<(), LoggingError> {
    let entry = EventLogEntry::new(
        "permission_decision",
        trace_id,
        serde_json::json!({
            "decision": decision,
            "resource": resource,
        }),
    );
    writer.append_event(entry)
}

/// Log a tool invocation to the event log.
pub fn log_tool_invocation(
    writer: &mut LogWriter,
    tool: &str,
    input: &serde_json::Value,
    success: bool,
    trace_id: &str,
) -> Result<(), LoggingError> {
    let entry = EventLogEntry::new(
        "tool_invoked",
        trace_id,
        serde_json::json!({
            "tool": tool,
            "input_summary": summarize_input(input),
            "success": success,
        }),
    );
    writer.append_event(entry)
}

/// Log a Shell Gate decision to the event log.
pub fn log_shell_gate_decision(
    writer: &mut LogWriter,
    command: &str,
    risk_level: &str,
    decision: &str,
    reason: &str,
    trace_id: &str,
) -> Result<(), LoggingError> {
    let entry = EventLogEntry::new(
        "shell_gate_decision",
        trace_id,
        serde_json::json!({
            "command": command,
            "risk_level": risk_level,
            "decision": decision,
            "reason": reason,
        }),
    );
    writer.append_event(entry)
}

/// Summarize tool input to avoid logging excessive data.
fn summarize_input(input: &serde_json::Value) -> String {
    if let Some(obj) = input.as_object() {
        let keys: Vec<String> = obj.keys().take(5).cloned().collect();
        format!(
            "{{{}{}}}",
            keys.join(", "),
            if obj.len() > 5 { "..." } else { "" }
        )
    } else {
        input.to_string().chars().take(100).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use tempfile::TempDir;

    fn make_writer(dir: &Path) -> LogWriter {
        LogWriter::new(dir).unwrap()
    }

    #[test]
    fn test_audit_log_records_permission_deny() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        log_permission_decision(&mut writer, "denied", "shell:rm -rf /", "trace-1").unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("event-log.jsonl")).unwrap();
        assert!(content.contains("permission_decision"));
        assert!(content.contains("denied"));
        assert!(content.contains("trace-1"));
    }

    #[test]
    fn test_audit_log_records_tool_call() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        log_tool_invocation(
            &mut writer,
            "read_file",
            &serde_json::json!({"path": "/workspace/main.rs"}),
            true,
            "trace-2",
        )
        .unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("event-log.jsonl")).unwrap();
        assert!(content.contains("tool_invoked"));
        assert!(content.contains("read_file"));
    }

    #[test]
    fn test_audit_log_records_shell_gate_decision() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());
        log_shell_gate_decision(
            &mut writer,
            "rm -rf /",
            "Critical",
            "Deny",
            "dangerous command",
            "trace-3",
        )
        .unwrap();
        writer.flush().unwrap();

        let content = std::fs::read_to_string(dir.path().join("event-log.jsonl")).unwrap();
        assert!(content.contains("shell_gate_decision"));
        assert!(content.contains("Critical"));
        assert!(content.contains("Deny"));
    }

    #[test]
    fn test_event_log_is_append_only() {
        let dir = TempDir::new().unwrap();
        let mut writer = make_writer(dir.path());

        log_permission_decision(&mut writer, "allowed", "shell:ls", "t1").unwrap();
        writer.flush().unwrap();
        let size1 = std::fs::metadata(dir.path().join("event-log.jsonl"))
            .unwrap()
            .len();

        log_permission_decision(&mut writer, "allowed", "shell:cat", "t2").unwrap();
        writer.flush().unwrap();
        let size2 = std::fs::metadata(dir.path().join("event-log.jsonl"))
            .unwrap()
            .len();

        assert!(size2 > size1);
    }
}
