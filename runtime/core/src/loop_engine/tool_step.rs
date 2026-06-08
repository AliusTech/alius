//! Tool step — executes tool calls requested by the model.
//!
//! Each tool call is dispatched through the ToolRegistry, with CoreEvent
//! emission for start/completion. When auto_confirm is true (Plan mode),
//! all tools execute without user confirmation.

use std::path::Path;

use protocol_interface::core::*;
use runtime_model::ToolCall;
use runtime_tools::{ToolContext, ToolRegistry};

/// Execute a batch of tool calls and return their results.
///
/// Returns `(tool_call_id, tool_name, output_text)` tuples for feeding
/// back to the model via `continue_with_tool_results`.
#[allow(clippy::too_many_arguments)]
pub async fn execute_tools(
    tool_calls: &[ToolCall],
    registry: &ToolRegistry,
    workspace: &Path,
    session_id: &str,
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

        let output = match execute_single_tool(call, registry, workspace, session_id).await {
            Ok(result) => result,
            Err(e) => e,
        };

        // Emit ToolCallCompleted
        *sequence += 1;
        event_sink(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            *sequence,
            CoreEventKind::ToolCallCompleted,
            CoreEventPayload::Json {
                value: serde_json::json!({
                    "id": call.id,
                    "name": call.name,
                    "success": !output.starts_with("error:"),
                    "output": output,
                }),
            },
        ));

        results.push((call.id.clone(), call.name.clone(), output));
    }

    Ok(results)
}

async fn execute_single_tool(
    call: &ToolCall,
    registry: &ToolRegistry,
    workspace: &Path,
    session_id: &str,
) -> Result<String, String> {
    let ctx = ToolContext::new(workspace.to_path_buf(), session_id.to_string());

    match registry.get(&call.name) {
        Some(tool) => tool
            .execute(call.args.clone(), ctx)
            .await
            .map(|r| r.output)
            .map_err(|e| format!("error: {}", e)),
        None => Err(format!("error: unknown tool '{}'", call.name)),
    }
}
