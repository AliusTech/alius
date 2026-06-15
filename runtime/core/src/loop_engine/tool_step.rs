//! Tool step — executes tool calls requested by the model.
//!
//! Each tool call is dispatched through the ToolRegistry, with CoreEvent
//! emission for start/completion. When a tool's `preview_confirmation` returns
//! true (Plan mode + risky op, e.g. high-risk shell or file write), the step
//! pauses: emits `ToolConfirmationRequired`, awaits the user's yes/no on a
//! oneshot held by SessionManager, and resumes (or denies) accordingly.

use std::path::Path;

use protocol_interface::core::*;
use runtime_model::ToolCall;
use runtime_tools::{AliusTool, ToolContext, ToolRegistry};

use crate::SessionManager;

/// Execute a batch of tool calls and return their results.
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
) -> Result<Vec<(String, String, String)>, ProtocolError> {
    let mut results = Vec::with_capacity(tool_calls.len());

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

        // Stage B: if this call needs confirmation, pause for user yes/no.
        let (denied, denial_reason) = if let Some(tool) = registry.get(&call.name) {
            if tool.preview_confirmation(&call.args, mode) {
                let approved = confirm_and_await(
                    &call.name, &call.id, &call.args, session, run_ref, trace_id, sequence,
                    event_sink,
                )
                .await?;
                if approved {
                    (false, None)
                } else {
                    (true, Some("denied_by_user"))
                }
            } else {
                (false, None)
            }
        } else {
            (false, None)
        };

        let output = if denied {
            format!("error: tool '{}' denied by user", call.name)
        } else {
            match execute_single_tool(call, registry, workspace, session_id, mode).await {
                Ok(result) => result,
                Err(e) => e,
            }
        };

        // Emit ToolCallCompleted
        *sequence += 1;
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
        event_sink(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            *sequence,
            CoreEventKind::ToolCallCompleted,
            CoreEventPayload::Json { value: payload },
        ));

        results.push((call.id.clone(), call.name.clone(), output));
    }

    Ok(results)
}

/// Emit `ToolConfirmationRequired`, register an oneshot on the session, and
/// await the user's response. Returns `true` if approved, `false` if denied,
/// cancelled, or no session is available (fail-closed).
///
/// On cancel the SessionManager drops the sender → `rx.await` returns `Err`.
/// The status is NOT restored to `Running` if the run already reached a
/// terminal state (`Cancelled`, `Failed`, `Completed`).
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
) -> Result<bool, ProtocolError> {
    let session = match session {
        Some(s) => s,
        None => {
            // Fail-closed: no session → cannot confirm → deny.
            return Ok(false);
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

    // Await the user's response. On cancel the sender is dropped → Err.
    let approved = rx.await.unwrap_or(false);

    // Only restore to Running if the run is still in WaitingForApproval.
    // If it was cancelled or otherwise terminal, do NOT overwrite.
    if let Ok(current) = session.get_run_status(run_ref) {
        if current == RunStatus::WaitingForApproval {
            let _ = session.update_run_status(run_ref, RunStatus::Running);
        }
    }

    Ok(approved)
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
    use std::sync::{Arc, Mutex};

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

    /// Plan mode: approved confirmation → tool executes, outputs "executed".
    #[tokio::test]
    async fn plan_approved_executes_tool() {
        let (registry, mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        // Spawn a task that delivers approval after a short delay.
        let mgr_clone = Arc::new(mgr);
        let mgr_spawn = mgr_clone.clone();
        let run_ref_clone = run_ref.clone();
        let approve_handle = tokio::spawn(async move {
            // Wait for confirmation sender to be stored.
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
        )
        .await
        .unwrap();

        approve_handle.await.unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "executed"); // tool was executed
        assert_eq!(
            mgr_clone.get_run_status(&run_ref).unwrap(),
            RunStatus::Running
        );

        // Verify ToolConfirmationRequired was emitted.
        let evts = events.lock().unwrap();
        assert!(evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired));
        // Verify ToolCallCompleted(success=true) was emitted.
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

    /// Plan mode: denied confirmation → tool NOT executed, output is error.
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
        )
        .await
        .unwrap();

        deny_handle.await.unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].2.starts_with("error:"));
        assert!(results[0].2.contains("denied by user"));

        // Verify ToolCallCompleted has denied=true.
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

    /// Cancel during confirmation → tool NOT executed, status stays Cancelled.
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
        )
        .await
        .unwrap();

        cancel_handle.await.unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].2.starts_with("error:"));
        // Status must be Cancelled, not restored to Running.
        assert_eq!(
            mgr_clone.get_run_status(&run_ref).unwrap(),
            RunStatus::Cancelled
        );
    }

    /// No session → fail-closed, tool NOT executed.
    #[tokio::test]
    async fn no_session_fail_closed() {
        let (registry, _mgr, run_ref, trace_id) = setup();
        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let results = execute_tools(
            &[make_call("c1", "confirm_required")],
            &registry,
            Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            None, // no session
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);
        assert!(results[0].2.starts_with("error:"));
        assert!(results[0].2.contains("denied by user"));

        // No ToolConfirmationRequired emitted (no session to store sender).
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
        )
        .await
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].2, "executed");
        // No ToolConfirmationRequired in Chat mode.
        let evts = events.lock().unwrap();
        assert!(!evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired));
    }
}
