//! Tool step — executes tool calls requested by the model.
//!
//! Each tool call is dispatched through the ToolRegistry, with CoreEvent
//! emission for start/completion. When a tool's `preview_confirmation` returns
//! true (Plan mode + risky op, e.g. high-risk shell or file write), the step
//! pauses: emits `ToolConfirmationRequired`, awaits the user's yes/no on a
//! oneshot held by SessionManager, and resumes (or denies) accordingly.
//!
//! On denial, cancellation, or unavailable session the entire batch is
//! aborted immediately — remaining tool calls are NOT executed (fail-fast).

use std::path::Path;
use std::sync::{Arc, Mutex};

use protocol_interface::core::*;
use runtime_model::ToolCall;
use runtime_tools::{AliusTool, ToolContext, ToolRegistry};

use crate::logging::audit;
use crate::logging::LogWriter;
use crate::SessionManager;

/// Outcome of a confirmation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmationDecision {
    /// User approved the tool execution.
    Approved,
    /// User explicitly denied the tool execution.
    Denied,
    /// The run was cancelled while waiting for confirmation.
    Cancelled,
    /// No session available to receive the confirmation (fail-closed).
    Unavailable,
}

impl ConfirmationDecision {
    /// Returns `true` only for explicit user approval.
    pub fn is_approved(&self) -> bool {
        matches!(self, ConfirmationDecision::Approved)
    }

    /// Human-readable reason for audit / error messages.
    pub fn reason(&self) -> &'static str {
        match self {
            ConfirmationDecision::Approved => "approved",
            ConfirmationDecision::Denied => "denied_by_user",
            ConfirmationDecision::Cancelled => "cancelled",
            ConfirmationDecision::Unavailable => "no_session",
        }
    }
}

/// Execute a batch of tool calls and return their results.
///
/// **Fail-fast**: once any tool confirmation is denied, cancelled, or
/// unavailable, the remaining tool calls in the batch are skipped and
/// filled with error placeholders.
#[allow(clippy::too_many_arguments)]
pub async fn execute_tools(
    tool_calls: &[ToolCall],
    registry: &ToolRegistry,
    workspace: &Path,
    session_id: &str,
    mode: RuntimeMode,
    session: Option<&SessionManager>,
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
    log_writer: Option<&Arc<Mutex<LogWriter>>>,
) -> Result<Vec<(String, String, String)>, ProtocolError> {
    let mut results = Vec::with_capacity(tool_calls.len());
    let mut batch_aborted = false;

    for call in tool_calls {
        // Emit ToolCallStarted
        *sequence += 1;
        event_sink(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            *sequence,
            CoreEventKind::ToolCallStarted,
            CoreEventPayload::Json {
                value: serde_json::json!({
                    "id": call.id,
                    "name": call.name,
                    "args": call.args,
                }),
            },
        ));

        // If the batch was already aborted by a prior denial, skip execution.
        if batch_aborted {
            let output = format!(
                "error: tool '{}' skipped — batch aborted by prior denial",
                call.name
            );
            emit_tool_completed(event_sink, run_ref, trace_id, sequence, call, &output, None);
            results.push((call.id.clone(), call.name.clone(), output));
            continue;
        }

        // Stage B: if this call needs confirmation, pause for user yes/no.
        let decision = if let Some(tool) = registry.get(&call.name) {
            if tool.preview_confirmation(&call.args, mode) {
                confirm_and_await(
                    &call.name, &call.id, &call.args, session, run_ref, trace_id, sequence,
                    event_sink, log_writer,
                )
                .await?
            } else {
                ConfirmationDecision::Approved // no confirmation needed
            }
        } else {
            ConfirmationDecision::Approved // unknown tool → execute_single_tool will error
        };

        if !decision.is_approved() {
            // Fail-fast: abort the entire batch.
            batch_aborted = true;
            let output = format!(
                "error: tool '{}' {} — batch aborted",
                call.name,
                decision.reason()
            );
            emit_tool_completed(
                event_sink,
                run_ref,
                trace_id,
                sequence,
                call,
                &output,
                Some(decision.reason()),
            );
            results.push((call.id.clone(), call.name.clone(), output));
            continue;
        }

        let output = match execute_single_tool(call, registry, workspace, session_id, mode).await {
            Ok(result) => result,
            Err(e) => e,
        };

        emit_tool_completed(event_sink, run_ref, trace_id, sequence, call, &output, None);
        results.push((call.id.clone(), call.name.clone(), output));
    }

    Ok(results)
}

/// Emit `ToolConfirmationRequired`, register an oneshot on the session, and
/// await the user's response. Returns a structured `ConfirmationDecision`.
///
/// - `Approved`: user explicitly approved.
/// - `Denied`: user explicitly denied.
/// - `Cancelled`: run was cancelled while waiting (sender dropped).
/// - `Unavailable`: no session to receive the confirmation (fail-closed).
///
/// Status is only restored to `Running` on `Approved` and only if still in
/// `WaitingForApproval`. Audit events are logged when a `LogWriter` is
/// available.
#[allow(clippy::too_many_arguments)]
async fn confirm_and_await(
    tool_name: &str,
    tool_call_id: &str,
    args: &serde_json::Value,
    session: Option<&SessionManager>,
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
    event_sink: &mut dyn FnMut(CoreEvent),
    log_writer: Option<&Arc<Mutex<LogWriter>>>,
) -> Result<ConfirmationDecision, ProtocolError> {
    let session = match session {
        Some(s) => s,
        None => {
            // Fail-closed: no session → cannot confirm → unavailable.
            audit_confirmation(
                log_writer,
                "unavailable",
                tool_name,
                tool_call_id,
                run_ref,
                trace_id,
            );
            return Ok(ConfirmationDecision::Unavailable);
        }
    };

    let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
    session.store_confirmation_sender(run_ref, tool_call_id, tx)?;

    *sequence += 1;
    event_sink(CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        *sequence,
        CoreEventKind::ToolConfirmationRequired,
        CoreEventPayload::ToolConfirmation {
            tool_call_id: tool_call_id.to_string(),
            tool_name: tool_name.to_string(),
            details: serde_json::to_string(args).unwrap_or_default(),
        },
    ));

    let _ = session.update_run_status(run_ref, RunStatus::WaitingForApproval);
    audit_confirmation(
        log_writer,
        "requested",
        tool_name,
        tool_call_id,
        run_ref,
        trace_id,
    );

    // Await the user's response.
    let decision = match rx.await {
        Ok(true) => ConfirmationDecision::Approved,
        Ok(false) => ConfirmationDecision::Denied,
        Err(_) => ConfirmationDecision::Cancelled, // sender dropped (cancel or error)
    };

    // Audit the outcome.
    audit_confirmation(
        log_writer,
        decision.reason(),
        tool_name,
        tool_call_id,
        run_ref,
        trace_id,
    );

    // Only restore to Running on explicit approval, and only if still waiting.
    if decision.is_approved() {
        if let Ok(current) = session.get_run_status(run_ref) {
            if current == RunStatus::WaitingForApproval {
                let _ = session.update_run_status(run_ref, RunStatus::Running);
            }
        }
    }

    Ok(decision)
}

/// Emit `ToolCallCompleted` with optional denial metadata.
fn emit_tool_completed(
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
    call: &ToolCall,
    output: &str,
    denial_reason: Option<&str>,
) {
    let mut payload = serde_json::json!({
        "id": call.id,
        "name": call.name,
        "args": call.args,
        "success": !output.starts_with("error:"),
        "output": output,
    });
    if let Some(reason) = denial_reason {
        payload["denied"] = serde_json::json!(true);
        payload["denial_reason"] = serde_json::json!(reason);
    }
    *sequence += 1;
    event_sink(CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        *sequence,
        CoreEventKind::ToolCallCompleted,
        CoreEventPayload::Json { value: payload },
    ));
}

/// Write a confirmation audit event if a log writer is available.
fn audit_confirmation(
    log_writer: Option<&Arc<Mutex<LogWriter>>>,
    action: &str,
    tool_name: &str,
    tool_call_id: &str,
    run_ref: &RunRef,
    trace_id: &TraceId,
) {
    if let Some(writer) = log_writer {
        if let Ok(mut w) = writer.lock() {
            let _ = audit::log_confirmation(
                &mut w,
                action,
                tool_name,
                tool_call_id,
                run_ref.as_str(),
                trace_id.as_str(),
            );
            let _ = w.flush();
        }
    }
}

async fn execute_single_tool(
    call: &ToolCall,
    registry: &ToolRegistry,
    workspace: &Path,
    session_id: &str,
    mode: RuntimeMode,
) -> Result<String, String> {
    let ctx = ToolContext::new(workspace.to_path_buf(), session_id.to_string(), mode);

    match registry.get(&call.name) {
        Some(tool) => tool
            .execute(call.args.clone(), ctx)
            .await
            .map(|r| r.output)
            .map_err(|e| format!("error: {}", e)),
        None => Err(format!("error: unknown tool '{}'", call.name)),
    }
}

#[allow(dead_code)]
fn _assert_tool_trait_object_safe(_t: Box<dyn AliusTool>) {}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use protocol_interface::core::WorkspaceRef;
    use serde_json::json;
    use tempfile::TempDir;

    /// A tool that always requires confirmation in Plan mode.
    struct ConfirmRequiredTool;

    #[async_trait]
    impl AliusTool for ConfirmRequiredTool {
        fn name(&self) -> &'static str {
            "confirm_required"
        }
        fn description(&self) -> &'static str {
            "test tool requiring confirmation"
        }
        fn input_schema(&self) -> serde_json::Value {
            json!({})
        }
        fn preview_confirmation(&self, _args: &serde_json::Value, mode: RuntimeMode) -> bool {
            mode == RuntimeMode::Plan
        }
        async fn execute(
            &self,
            _args: serde_json::Value,
            _ctx: ToolContext,
        ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
            Ok(runtime_tools::ToolResult {
                output: "executed".to_string(),
                success: true,
                metadata: None,
            })
        }
    }

    /// A tool that never requires confirmation.
    struct NoConfirmTool;

    #[async_trait]
    impl AliusTool for NoConfirmTool {
        fn name(&self) -> &'static str {
            "no_confirm"
        }
        fn description(&self) -> &'static str {
            "test tool without confirmation"
        }
        fn input_schema(&self) -> serde_json::Value {
            json!({})
        }
        async fn execute(
            &self,
            _args: serde_json::Value,
            _ctx: ToolContext,
        ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
            Ok(runtime_tools::ToolResult {
                output: "executed".to_string(),
                success: true,
                metadata: None,
            })
        }
    }

    fn setup() -> (ToolRegistry, SessionManager, RunRef, TraceId) {
        let mut registry = ToolRegistry::new();
        registry
            .register(ConfirmRequiredTool)
            .expect("register confirm_required");
        registry
            .register(NoConfirmTool)
            .expect("register no_confirm");

        let mgr = SessionManager::new(WorkspaceRef::new("/tmp"));
        let session = mgr.create_session();
        let (_turn, run_ref, trace_id) = mgr.create_turn(&session.session_ref).unwrap();
        (registry, mgr, run_ref, trace_id)
    }

    fn make_call(id: &str, name: &str) -> ToolCall {
        ToolCall {
            id: id.to_string(),
            name: name.to_string(),
            args: json!({}),
        }
    }

    /// Plan mode: approved confirmation → tool executes.
    #[tokio::test]
    async fn plan_approved_executes_tool() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let mgr_clone = Arc::new(mgr);
        let mgr_spawn = mgr_clone.clone();
        let run_ref_clone = run_ref.clone();
        let approve_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = mgr_spawn.deliver_confirmation(&run_ref_clone, "c1", true);
        });

        let results = execute_tools(
            &[make_call("c1", "confirm_required")],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            Some(&mgr_clone),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        approve_handle.await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "executed");
        assert_eq!(
            mgr_clone.get_run_status(&run_ref).unwrap(),
            RunStatus::Running
        );

        let evts = events.lock().unwrap();
        assert!(evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired));
        let completed = evts
            .iter()
            .find(|e| e.kind == CoreEventKind::ToolCallCompleted)
            .unwrap();
        if let CoreEventPayload::Json { value } = &completed.payload {
            assert_eq!(value["success"], true);
            assert_eq!(value["output"], "executed");
        } else {
            panic!("expected Json payload");
        }
    }

    /// Plan mode: denied confirmation → tool NOT executed.
    #[tokio::test]
    async fn plan_denied_does_not_execute_tool() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let mgr_clone = Arc::new(mgr);
        let mgr_spawn = mgr_clone.clone();
        let run_ref_clone = run_ref.clone();
        let deny_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = mgr_spawn.deliver_confirmation(&run_ref_clone, "c1", false);
        });

        let results = execute_tools(
            &[make_call("c1", "confirm_required")],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            Some(&mgr_clone),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        deny_handle.await.unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].2.starts_with("error:"));
        assert!(results[0].2.contains("denied_by_user"));
        assert!(results[0].2.contains("batch aborted"));

        let evts = events.lock().unwrap();
        let completed = evts
            .iter()
            .find(|e| e.kind == CoreEventKind::ToolCallCompleted)
            .unwrap();
        if let CoreEventPayload::Json { value } = &completed.payload {
            assert_eq!(value["success"], false);
            assert_eq!(value["denied"], true);
            assert_eq!(value["denial_reason"], "denied_by_user");
        } else {
            panic!("expected Json payload");
        }
    }

    /// [P0] Denial fail-fast: first tool denied → second tool NOT executed.
    #[tokio::test]
    async fn denial_stops_remaining_tools_in_batch() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let mgr_clone = Arc::new(mgr);
        let mgr_spawn = mgr_clone.clone();
        let run_ref_clone = run_ref.clone();
        let deny_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = mgr_spawn.deliver_confirmation(&run_ref_clone, "c1", false);
        });

        // Batch: first requires confirmation, second is a normal tool.
        let results = execute_tools(
            &[
                make_call("c1", "confirm_required"),
                make_call("c2", "no_confirm"),
            ],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            Some(&mgr_clone),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        deny_handle.await.unwrap();

        assert_eq!(results.len(), 2);

        // First tool: denied.
        assert!(results[0].2.contains("denied_by_user"));
        assert!(results[0].2.contains("batch aborted"));

        // Second tool: skipped (NOT executed).
        assert!(results[1].2.contains("skipped"));
        assert!(results[1].2.contains("batch aborted"));
        assert!(!results[1].2.contains("executed")); // "executed" is NoConfirmTool's output

        // Verify only one ToolConfirmationRequired was emitted.
        let evts = events.lock().unwrap();
        let confirmations: Vec<_> = evts
            .iter()
            .filter(|e| e.kind == CoreEventKind::ToolConfirmationRequired)
            .collect();
        assert_eq!(confirmations.len(), 1);
    }

    /// Cancel during confirmation → batch aborted, status stays Cancelled.
    #[tokio::test]
    async fn plan_cancel_during_confirmation() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let mgr_clone = Arc::new(mgr);
        let mgr_spawn = mgr_clone.clone();
        let run_ref_clone = run_ref.clone();
        let cancel_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = mgr_spawn.cancel_run(&run_ref_clone);
        });

        let results = execute_tools(
            &[
                make_call("c1", "confirm_required"),
                make_call("c2", "no_confirm"),
            ],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            Some(&mgr_clone),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        cancel_handle.await.unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].2.contains("cancelled"));
        assert!(results[1].2.contains("skipped"));
        assert_eq!(
            mgr_clone.get_run_status(&run_ref).unwrap(),
            RunStatus::Cancelled
        );
    }

    /// No session → fail-closed, batch aborted.
    #[tokio::test]
    async fn no_session_fail_closed() {
        let (registry, _mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let results = execute_tools(
            &[
                make_call("c1", "confirm_required"),
                make_call("c2", "no_confirm"),
            ],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            None,
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 2);
        assert!(results[0].2.contains("no_session"));
        assert!(results[0].2.contains("batch aborted"));
        assert!(results[1].2.contains("skipped"));

        // No ToolConfirmationRequired emitted (no session).
        let evts = events.lock().unwrap();
        assert!(!evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired));
    }

    /// Chat mode: no confirmation needed → tool executes directly.
    #[tokio::test]
    async fn chat_mode_no_confirmation() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;
        let mgr = Arc::new(mgr);

        let results = execute_tools(
            &[make_call("c1", "confirm_required")],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Chat,
            Some(&mgr),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "executed");
        let evts = events.lock().unwrap();
        assert!(!evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired));
    }

    /// Audit logging: confirmation events are written to the log writer.
    #[tokio::test]
    async fn audit_confirmation_events_logged() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let tmp = TempDir::new().unwrap();
        let log_writer = Arc::new(Mutex::new(
            crate::logging::LogWriter::new(tmp.path()).unwrap(),
        ));
        let log_writer_clone = log_writer.clone();

        let mgr_clone = Arc::new(mgr);
        let mgr_spawn = mgr_clone.clone();
        let run_ref_clone = run_ref.clone();
        let deny_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = mgr_spawn.deliver_confirmation(&run_ref_clone, "c1", false);
        });

        let _results = execute_tools(
            &[make_call("c1", "confirm_required")],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            Some(&mgr_clone),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            Some(&log_writer_clone),
        )
        .await
        .unwrap();

        deny_handle.await.unwrap();

        // Flush and read the audit log.
        log_writer_clone.lock().unwrap().flush().unwrap();
        let content = std::fs::read_to_string(tmp.path().join("event-log.jsonl")).unwrap();

        // Must contain requested + denied events.
        assert!(
            content.contains("tool_confirmation"),
            "audit log missing confirmation entries"
        );
        assert!(
            content.contains("requested"),
            "audit log missing 'requested'"
        );
        assert!(
            content.contains("denied_by_user"),
            "audit log missing 'denied_by_user'"
        );
        assert!(
            content.contains("confirm_required"),
            "audit log missing tool name"
        );
        assert!(
            content.contains(run_ref.as_str()),
            "audit log missing run_ref"
        );
        // Must not contain sensitive args.
        assert!(!content.contains("secret"));
    }
}
