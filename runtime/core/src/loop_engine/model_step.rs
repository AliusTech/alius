//! Model step — executes model calls and emits streaming events.

use futures::StreamExt;

use protocol_interface::core::*;
use protocol_interface::ToolDef;
use runtime_model::{ChatEvent, Conversation, LlmClient, ToolCall};

/// Result of a model step that may include tool calls.
pub struct ModelStepResult {
    pub text: String,
    pub tool_calls: Option<Vec<ToolCall>>,
}

/// Execute a Chat-mode model call with streaming (no tools).
pub async fn execute_chat(
    client: &LlmClient,
    conversation: &Conversation,
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
) -> Result<String, ProtocolError> {
    let mut stream = client
        .chat_stream(conversation)
        .await
        .map_err(|e| ProtocolError::Internal(e.to_string()))?;

    let mut full_text = String::new();

    while let Some(result) = stream.next().await {
        match result {
            Ok(ChatEvent::Delta { text }) => {
                if text.is_empty() {
                    continue;
                }
                full_text.push_str(&text);
                *sequence += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    *sequence,
                    CoreEventKind::ModelDelta,
                    CoreEventPayload::Text { text },
                ));
            }
            Ok(ChatEvent::Done { .. }) => {}
            Ok(ChatEvent::Error { message }) => {
                *sequence += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    *sequence,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "chat_error".to_string(),
                        message,
                    },
                ));
            }
            Err(e) => {
                *sequence += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    *sequence,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "stream_error".to_string(),
                        message: e.to_string(),
                    },
                ));
            }
        }
    }

    Ok(full_text)
}

/// Execute a model call with tool definitions. Returns response text
/// and any tool calls the model requested.
pub async fn execute_with_tools(
    client: &LlmClient,
    conversation: &Conversation,
    tools: Vec<ToolDef>,
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
) -> Result<ModelStepResult, ProtocolError> {
    let (stream, tool_calls) = client
        .chat_stream_with_tools(conversation, tools)
        .await
        .map_err(|e| ProtocolError::Internal(e.to_string()))?;

    let text = collect_stream(stream, event_sink, run_ref, trace_id, sequence).await?;

    Ok(ModelStepResult { text, tool_calls })
}

/// Continue the conversation after tool execution, sending tool results
/// back to the model.
#[allow(clippy::too_many_arguments)]
pub async fn continue_with_tool_results(
    client: &LlmClient,
    conversation: &Conversation,
    tool_results: &[(String, String, String)],
    assistant_tool_calls: Vec<runtime_model::ToolCall>,
    tools: Vec<ToolDef>,
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
) -> Result<ModelStepResult, ProtocolError> {
    let (stream, tool_calls) = client
        .continue_with_tool_results(
            conversation,
            tool_results.to_vec(),
            assistant_tool_calls,
            tools,
        )
        .await
        .map_err(|e| ProtocolError::Internal(e.to_string()))?;

    let text = collect_stream(stream, event_sink, run_ref, trace_id, sequence).await?;

    Ok(ModelStepResult { text, tool_calls })
}

/// Collect streaming text from a ChatStream, emitting ModelDelta events.
async fn collect_stream(
    stream: runtime_model::ChatStream,
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    sequence: &mut u64,
) -> Result<String, ProtocolError> {
    let mut full_text = String::new();
    let mut stream = Box::pin(stream);

    while let Some(result) = stream.next().await {
        match result {
            Ok(ChatEvent::Delta { text }) => {
                if text.is_empty() {
                    continue;
                }
                full_text.push_str(&text);
                *sequence += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    *sequence,
                    CoreEventKind::ModelDelta,
                    CoreEventPayload::Text { text },
                ));
            }
            Ok(ChatEvent::Done { .. }) => {}
            Ok(ChatEvent::Error { message }) => {
                *sequence += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    *sequence,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "chat_error".to_string(),
                        message,
                    },
                ));
            }
            Err(e) => {
                *sequence += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    *sequence,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "stream_error".to_string(),
                        message: e.to_string(),
                    },
                ));
            }
        }
    }

    Ok(full_text)
}
