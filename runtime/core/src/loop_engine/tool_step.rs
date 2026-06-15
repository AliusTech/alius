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
