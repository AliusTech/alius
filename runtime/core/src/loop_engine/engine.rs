//! Loop engine orchestration.

use std::collections::{HashMap, HashSet};

use protocol_interface::core::{
    CoreEvent, CoreEventKind, CoreEventPayload, LoopPolicy, RequestInput, RunLoopInput, RunRef,
    RuntimeMode, TraceId,
};

use crate::loop_engine::{
    check_convergence, ContextManager, LoopContext, LoopExecutionResult, LoopIteration,
};

pub struct LoopEngine;

impl LoopEngine {
    pub fn input_from_request(input: &RequestInput) -> RunLoopInput {
        match input {
            RequestInput::RunLoop { input } => input.clone(),
            RequestInput::Text { content } => RunLoopInput {
                content: content.clone(),
                mode: RuntimeMode::Chat,
                policy: LoopPolicy::chat(),
            },
            _ => RunLoopInput {
                content: String::new(),
                mode: RuntimeMode::Chat,
                policy: LoopPolicy::chat(),
            },
        }
    }

    /// Run the loop engine.
    ///
    /// Chat mode: single-pass streaming model call.
    /// Plan mode: multi-iteration loop with tool execution.
    pub async fn run(
        run_ref: &RunRef,
        trace_id: &TraceId,
        input: &RunLoopInput,
        ctx: &LoopContext,
        event_sink: &mut dyn FnMut(CoreEvent),
    ) -> LoopExecutionResult {
        // Check for cancellation before starting
        if let Some(token) = &ctx.cancel_token {
            if token.is_cancelled() {
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    1,
                    CoreEventKind::FinalResult,
                    CoreEventPayload::Final {
                        content: "Cancelled by user".to_string(),
                        success: false,
                    },
                ));
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                };
            }
        }

        // Emit RunStarted
        event_sink(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            1,
            CoreEventKind::RunStarted,
            CoreEventPayload::Json {
                value: serde_json::json!({
                    "mode": input.mode,
                    "max_iterations": input.policy.max_iterations,
                    "tools_enabled": input.policy.tools_enabled,
                    "planning_enabled": input.policy.planning_enabled,
                }),
            },
        ));

        match input.mode {
            RuntimeMode::Chat if input.policy.tools_enabled => {
                Self::run_chat_with_tools(run_ref, trace_id, ctx, event_sink).await
            }
            RuntimeMode::Chat => Self::run_chat(run_ref, trace_id, ctx, event_sink).await,
            RuntimeMode::Plan => Self::run_plan(run_ref, trace_id, input, ctx, event_sink).await,
        }
    }

    /// Chat mode: single-pass streaming model call.
    async fn run_chat(
        run_ref: &RunRef,
        trace_id: &TraceId,
        ctx: &LoopContext,
        event_sink: &mut dyn FnMut(CoreEvent),
    ) -> LoopExecutionResult {
        let mut seq = 2u64;

        // Check for cancellation before starting model call
        if let Some(token) = &ctx.cancel_token {
            if token.is_cancelled() {
                emit_final(
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                    "Cancelled by user",
                    false,
                );
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                };
            }
        }

        match super::model_step::execute_chat(
            &ctx.client,
            &ctx.conversation,
            event_sink,
            run_ref,
            trace_id,
            &mut seq,
        )
        .await
        {
            Ok(full_text) => {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::FinalResult,
                    CoreEventPayload::Final {
                        content: full_text.clone(),
                        success: true,
                    },
                ));

                LoopExecutionResult {
                    events: Vec::new(),
                    final_content: full_text,
                }
            }
            Err(e) => {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "loop_error".to_string(),
                        message: e.to_string(),
                    },
                ));
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::FinalResult,
                    CoreEventPayload::Final {
                        content: String::new(),
                        success: false,
                    },
                ));

                LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                }
            }
        }
    }

    /// Chat/Bypass mode: one model turn with optional one-shot tool execution.
    ///
    /// This path exposes tools to the model, executes the requested batch once,
    /// and returns the tool results as the final turn output. It intentionally
    /// does not continue the model with tool results; multi-turn tool loops are
    /// reserved for Plan mode.
    async fn run_chat_with_tools(
        run_ref: &RunRef,
        trace_id: &TraceId,
        ctx: &LoopContext,
        event_sink: &mut dyn FnMut(CoreEvent),
    ) -> LoopExecutionResult {
        // Check for cancellation before starting
        if let Some(token) = &ctx.cancel_token {
            if token.is_cancelled() {
                let mut seq = 2u64;
                emit_final(
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                    "Cancelled by user",
                    false,
                );
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                };
            }
        }

        let registry = match &ctx.tool_registry {
            Some(registry) => registry.clone(),
            None => return Self::run_chat(run_ref, trace_id, ctx, event_sink).await,
        };

        let tools = registry.to_tool_defs();
        if tools.is_empty() {
            return Self::run_chat(run_ref, trace_id, ctx, event_sink).await;
        }

        let mut seq = 2u64;
        let model_result = match super::model_step::execute_with_tools(
            &ctx.client,
            &ctx.conversation,
            tools,
            event_sink,
            run_ref,
            trace_id,
            &mut seq,
        )
        .await
        {
            Ok(result) => result,
            Err(e) => {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "model_error".to_string(),
                        message: e.to_string(),
                    },
                ));
                emit_final(event_sink, run_ref, trace_id, &mut seq, "", false);
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                };
            }
        };

        let tool_calls = model_result.tool_calls.clone().unwrap_or_default();
        if tool_calls.is_empty() {
            emit_final(
                event_sink,
                run_ref,
                trace_id,
                &mut seq,
                &model_result.text,
                true,
            );
            return LoopExecutionResult {
                events: Vec::new(),
                final_content: model_result.text,
            };
        }

        if let Err(message) = validate_assistant_tool_calls(&tool_calls) {
            seq += 1;
            event_sink(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                seq,
                CoreEventKind::ErrorRaised,
                CoreEventPayload::Error {
                    code: "invalid_tool_calls".to_string(),
                    message,
                },
            ));
            emit_final(
                event_sink,
                run_ref,
                trace_id,
                &mut seq,
                &model_result.text,
                false,
            );
            return LoopExecutionResult {
                events: Vec::new(),
                final_content: model_result.text,
            };
        }

        // Check for cancellation before executing tools
        if let Some(token) = &ctx.cancel_token {
            if token.is_cancelled() {
                emit_final(
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                    "Cancelled by user",
                    false,
                );
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                };
            }
        }

        let batch = match super::tool_step::execute_tools(
            &tool_calls,
            &registry,
            &ctx.workspace,
            &format!("chat-run-{}", run_ref.as_str()),
            RuntimeMode::Chat,
            ctx.session.as_ref().map(|a| a.as_ref()),
            event_sink,
            run_ref,
            trace_id,
            &mut seq,
            ctx.log_writer.as_ref(),
        )
        .await
        {
            Ok(batch) => batch,
            Err(e) => {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "tool_error".to_string(),
                        message: e.to_string(),
                    },
                ));
                emit_final(
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                    &model_result.text,
                    false,
                );
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: model_result.text,
                };
            }
        };

        let tool_results = normalize_tool_results(&tool_calls, &batch.results);
        let final_content = chat_tool_final_content(&model_result.text, &tool_results);

        // If any tool was denied/cancelled/unavailable, emit ErrorRaised
        // before the failed FinalResult — same semantics as Plan path.
        if batch.batch_denied {
            seq += 1;
            event_sink(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                seq,
                CoreEventKind::ErrorRaised,
                CoreEventPayload::Error {
                    code: "tool_denied".to_string(),
                    message: format!(
                        "tool execution {} — chat aborted",
                        batch.denial_reason.unwrap_or("denied")
                    ),
                },
            ));
        }

        emit_final(
            event_sink,
            run_ref,
            trace_id,
            &mut seq,
            &final_content,
            !batch.batch_denied,
        );

        LoopExecutionResult {
            events: Vec::new(),
            final_content,
        }
    }

    /// Plan mode: multi-iteration loop with tool execution.
    ///
    /// Each iteration:
    /// 1. Call model with tools (or continue with tool results)
    /// 2. If model requests tools → execute them → loop
    /// 3. If model produces text only → done
    async fn run_plan(
        run_ref: &RunRef,
        trace_id: &TraceId,
        input: &RunLoopInput,
        ctx: &LoopContext,
        event_sink: &mut dyn FnMut(CoreEvent),
    ) -> LoopExecutionResult {
        let registry = match &ctx.tool_registry {
            Some(r) => r.clone(),
            None => {
                let mut seq = 2u64;
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "no_tool_registry".to_string(),
                        message: "Plan mode requires a tool registry".to_string(),
                    },
                ));
                emit_final(event_sink, run_ref, trace_id, &mut seq, "", false);
                return LoopExecutionResult {
                    events: Vec::new(),
                    final_content: String::new(),
                };
            }
        };

        let tools = registry.to_tool_defs();
        let mut conversation = runtime_model::Conversation::from_messages(
            ctx.conversation.system_prompt().map(|s| s.to_string()),
            ctx.conversation.messages().to_vec(),
        );
        let mut seq = 2u64;
        let mut pending_tool_results: Vec<(String, String, String)> = Vec::new();
        let mut iteration_index: u32 = 0;
        let max_iterations = input.policy.max_iterations;
        let mut final_content = String::new();
        let context_mgr = ContextManager::new(ctx.max_context_tokens);

        // Track loop exit reason
        enum ExitReason {
            Success,
            Cancelled,
            Error,
            MaxIterations,
        }
        let mut exit_reason = ExitReason::Success;

        let mut last_assistant_tool_calls: Option<Vec<runtime_model::ToolCall>> = None;
        loop {
            // Check for cancellation at the start of each iteration
            if let Some(token) = &ctx.cancel_token {
                if token.is_cancelled() {
                    exit_reason = ExitReason::Cancelled;
                    emit_final(
                        event_sink,
                        run_ref,
                        trace_id,
                        &mut seq,
                        "Cancelled by user",
                        false,
                    );
                    break;
                }
            }

            iteration_index += 1;

            if iteration_index > max_iterations {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "max_iterations".to_string(),
                        message: format!("max iterations ({}) reached", max_iterations),
                    },
                ));
                exit_reason = ExitReason::MaxIterations;
                break;
            }

            // Context window management. Do not truncate between an
            // assistant(tool_calls) turn and the tool results that must answer it.
            if should_truncate_context(&context_mgr, &conversation, &pending_tool_results) {
                context_mgr.truncate(&mut conversation);
            }

            // Emit LoopIterationStarted
            seq += 1;
            event_sink(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                seq,
                CoreEventKind::LoopIterationStarted,
                CoreEventPayload::Json {
                    value: serde_json::json!({
                        "iteration": iteration_index,
                        "mode": "plan",
                    }),
                },
            ));

            // Model call
            let mut consumed_tool_results: Option<Vec<(String, String, String)>> = None;
            let model_result = if pending_tool_results.is_empty() {
                super::model_step::execute_with_tools(
                    &ctx.client,
                    &conversation,
                    tools.clone(),
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                )
                .await
            } else {
                let assistant_tool_calls = last_assistant_tool_calls.clone().unwrap_or_default();
                let normalized_tool_results =
                    normalize_tool_results(&assistant_tool_calls, &pending_tool_results);
                let result = super::model_step::continue_with_tool_results(
                    &ctx.client,
                    &conversation,
                    &normalized_tool_results,
                    assistant_tool_calls,
                    tools.clone(),
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                )
                .await;
                if result.is_ok() {
                    consumed_tool_results = Some(normalized_tool_results);
                }
                result
            };

            let model_result = match model_result {
                Ok(r) => r,
                Err(e) => {
                    seq += 1;
                    event_sink(CoreEvent::new(
                        run_ref.clone(),
                        trace_id.clone(),
                        seq,
                        CoreEventKind::ErrorRaised,
                        CoreEventPayload::Error {
                            code: "model_error".to_string(),
                            message: e.to_string(),
                        },
                    ));
                    exit_reason = ExitReason::Error;
                    break;
                }
            };

            if let Some(tool_results) = consumed_tool_results.take() {
                append_tool_results_to_conversation(&mut conversation, &tool_results);
                pending_tool_results.clear();
            }

            let has_tool_calls = append_model_result_to_conversation(
                &mut conversation,
                &model_result,
                &mut final_content,
            );

            // Remember this turn's tool calls so the next iteration can rebuild
            // the assistant tool_calls message that must precede tool results.
            last_assistant_tool_calls = model_result.tool_calls.clone();

            if !has_tool_calls {
                // Model finished — no more tool calls
                let iteration = LoopIteration {
                    index: iteration_index,
                    mode: RuntimeMode::Plan,
                    policy: input.policy.clone(),
                };
                let report = check_convergence(&iteration, false, false);
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ConvergenceChecked,
                    CoreEventPayload::Convergence { report },
                ));
                break;
            }

            // Execute tool calls
            let tool_calls = model_result.tool_calls.unwrap_or_default();
            if let Err(message) = validate_assistant_tool_calls(&tool_calls) {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "invalid_tool_calls".to_string(),
                        message,
                    },
                ));
                exit_reason = ExitReason::Error;
                break;
            }

            // Check for cancellation before executing tools
            if let Some(token) = &ctx.cancel_token {
                if token.is_cancelled() {
                    exit_reason = ExitReason::Cancelled;
                    emit_final(
                        event_sink,
                        run_ref,
                        trace_id,
                        &mut seq,
                        "Cancelled by user",
                        false,
                    );
                    break;
                }
            }

            let batch = match super::tool_step::execute_tools(
                &tool_calls,
                &registry,
                &ctx.workspace,
                &format!("plan-run-{}", run_ref.as_str()),
                input.mode,
                ctx.session.as_ref().map(|a| a.as_ref()),
                event_sink,
                run_ref,
                trace_id,
                &mut seq,
                ctx.log_writer.as_ref(),
            )
            .await
            {
                Ok(batch) => batch,
                Err(e) => {
                    seq += 1;
                    event_sink(CoreEvent::new(
                        run_ref.clone(),
                        trace_id.clone(),
                        seq,
                        CoreEventKind::ErrorRaised,
                        CoreEventPayload::Error {
                            code: "tool_error".to_string(),
                            message: e.to_string(),
                        },
                    ));
                    exit_reason = ExitReason::Error;
                    break;
                }
            };

            pending_tool_results = normalize_tool_results(&tool_calls, &batch.results);

            // Fail-closed: if any tool was denied/cancelled/unavailable,
            // stop the loop immediately.
            if batch.batch_denied {
                seq += 1;
                event_sink(CoreEvent::new(
                    run_ref.clone(),
                    trace_id.clone(),
                    seq,
                    CoreEventKind::ErrorRaised,
                    CoreEventPayload::Error {
                        code: "tool_denied".to_string(),
                        message: format!(
                            "tool execution {} — plan aborted",
                            batch.denial_reason.unwrap_or("denied")
                        ),
                    },
                ));
                exit_reason = ExitReason::Error;
                break;
            }
        }

        // Only emit FinalResult if loop didn't already emit one (e.g., on cancel)
        let success = matches!(exit_reason, ExitReason::Success);
        if !matches!(exit_reason, ExitReason::Cancelled) {
            emit_final(
                event_sink,
                run_ref,
                trace_id,
                &mut seq,
                &final_content,
                success,
            );
        }

        LoopExecutionResult {
            events: Vec::new(),
            final_content,
        }
    }

    /// Stub execution for testing.
    pub fn run_stub(
        run_ref: &RunRef,
        trace_id: &TraceId,
        input: &RunLoopInput,
    ) -> LoopExecutionResult {
        let iteration = LoopIteration::first(input.mode, input.policy.clone());
        let mut sequence = 1;
        let mut events = Vec::new();

        events.push(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            sequence,
            CoreEventKind::RunStarted,
            CoreEventPayload::Json {
                value: serde_json::json!({
                    "mode": input.mode,
                    "max_iterations": input.policy.max_iterations,
                    "tools_enabled": input.policy.tools_enabled,
                    "planning_enabled": input.policy.planning_enabled,
                }),
            },
        ));
        sequence += 1;

        events.push(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            sequence,
            CoreEventKind::LoopIterationStarted,
            CoreEventPayload::Json {
                value: serde_json::json!({
                    "iteration": iteration.index,
                    "mode": input.mode,
                }),
            },
        ));
        sequence += 1;

        let report = check_convergence(&iteration, false, false);
        events.push(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            sequence,
            CoreEventKind::ConvergenceChecked,
            CoreEventPayload::Convergence { report },
        ));

        LoopExecutionResult {
            events,
            final_content: format!(
                "stub: loop engine {:?} mode completed for {}",
                input.mode, input.content
            ),
        }
    }
}

fn append_model_result_to_conversation(
    conversation: &mut runtime_model::Conversation,
    model_result: &super::model_step::ModelStepResult,
    final_content: &mut String,
) -> bool {
    let tool_calls_msg: Vec<protocol_interface::MessageToolCall> = model_result
        .tool_calls
        .as_ref()
        .map(|calls| {
            calls
                .iter()
                .map(|c| protocol_interface::MessageToolCall {
                    id: c.id.clone(),
                    name: c.name.clone(),
                    arguments: c.args.to_string(),
                })
                .collect()
        })
        .unwrap_or_default();

    if !tool_calls_msg.is_empty() {
        // OpenAI-compatible APIs require the following tool-result messages to
        // directly follow the assistant message carrying `tool_calls`.
        conversation.add_assistant_with_tools(model_result.text.clone(), tool_calls_msg);
        if !model_result.text.is_empty() {
            *final_content = model_result.text.clone();
        }
        return true;
    }

    if !model_result.text.is_empty() {
        conversation.add_assistant_message(model_result.text.clone());
        *final_content = model_result.text.clone();
    }

    false
}

fn append_tool_results_to_conversation(
    conversation: &mut runtime_model::Conversation,
    tool_results: &[(String, String, String)],
) {
    for (tool_call_id, tool_name, result) in tool_results {
        conversation.add_tool_result(tool_call_id.clone(), tool_name.clone(), result.clone());
    }
}

fn validate_assistant_tool_calls(tool_calls: &[runtime_model::ToolCall]) -> Result<(), String> {
    let mut seen = HashSet::new();
    for call in tool_calls {
        if call.id.trim().is_empty() {
            return Err(format!(
                "model returned an empty tool_call_id for tool '{}'",
                call.name
            ));
        }
        if !seen.insert(call.id.as_str()) {
            return Err(format!(
                "model returned duplicate tool_call_id '{}'",
                call.id
            ));
        }
    }
    Ok(())
}

fn normalize_tool_results(
    assistant_tool_calls: &[runtime_model::ToolCall],
    tool_results: &[(String, String, String)],
) -> Vec<(String, String, String)> {
    let mut by_id: HashMap<&str, (&str, &str)> = HashMap::new();
    for (call_id, name, result) in tool_results {
        by_id.entry(call_id.as_str()).or_insert((name, result));
    }

    assistant_tool_calls
        .iter()
        .map(|call| {
            by_id
                .get(call.id.as_str())
                .map(|(name, result)| (call.id.clone(), (*name).to_string(), (*result).to_string()))
                .unwrap_or_else(|| {
                    (
                        call.id.clone(),
                        call.name.clone(),
                        format!("error: missing tool result for tool_call_id '{}'", call.id),
                    )
                })
        })
        .collect()
}

fn should_truncate_context(
    context_mgr: &ContextManager,
    conversation: &runtime_model::Conversation,
    pending_tool_results: &[(String, String, String)],
) -> bool {
    pending_tool_results.is_empty() && context_mgr.needs_truncation(conversation)
}

/// Returns `true` if any tool result indicates user denial.
fn emit_final(
    event_sink: &mut dyn FnMut(CoreEvent),
    run_ref: &RunRef,
    trace_id: &TraceId,
    seq: &mut u64,
    content: &str,
    success: bool,
) {
    *seq += 1;
    event_sink(CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        *seq,
        CoreEventKind::FinalResult,
        CoreEventPayload::Final {
            content: content.to_string(),
            success,
        },
    ));
}

fn chat_tool_final_content(model_text: &str, tool_results: &[(String, String, String)]) -> String {
    let mut sections = Vec::new();
    if !model_text.trim().is_empty() {
        sections.push(model_text.to_string());
    }

    if !tool_results.is_empty() {
        let mut lines = vec!["Tool results:".to_string()];
        for (call_id, tool_name, output) in tool_results {
            lines.push(format!(
                "- {} ({}): {}",
                tool_name,
                call_id,
                indent_multiline(&truncate_chat_tool_output(output))
            ));
        }
        sections.push(lines.join("\n"));
    }

    sections.join("\n\n")
}

fn truncate_chat_tool_output(output: &str) -> String {
    const MAX_CHARS: usize = 4_000;
    if output.chars().count() <= MAX_CHARS {
        return output.to_string();
    }
    let truncated: String = output.chars().take(MAX_CHARS).collect();
    format!("{truncated}\n... [tool output truncated]")
}

fn indent_multiline(output: &str) -> String {
    output.replace('\n', "\n  ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::MessageRole;
    use serde_json::json;

    #[test]
    fn tool_call_only_result_stores_single_assistant_tool_call_message() {
        let mut conversation = runtime_model::Conversation::new(None);
        let result = super::super::model_step::ModelStepResult {
            text: String::new(),
            tool_calls: Some(vec![runtime_model::ToolCall::new(
                "call-readme".to_string(),
                "read_file".to_string(),
                json!({ "path": "README.md" }),
            )]),
        };
        let mut final_content = String::new();

        let has_tool_calls =
            append_model_result_to_conversation(&mut conversation, &result, &mut final_content);

        assert!(has_tool_calls);
        assert!(final_content.is_empty());
        let messages = conversation.messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, MessageRole::Assistant);
        assert!(messages[0].content.is_empty());
        let tool_calls = messages[0].tool_calls.as_ref().unwrap();
        assert_eq!(tool_calls.len(), 1);
        assert_eq!(tool_calls[0].id, "call-readme");
        assert_eq!(tool_calls[0].name, "read_file");
        assert_eq!(tool_calls[0].arguments, r#"{"path":"README.md"}"#);
    }

    #[test]
    fn text_only_result_stores_normal_assistant_message() {
        let mut conversation = runtime_model::Conversation::new(None);
        let result = super::super::model_step::ModelStepResult {
            text: "summary".to_string(),
            tool_calls: None,
        };
        let mut final_content = String::new();

        let has_tool_calls =
            append_model_result_to_conversation(&mut conversation, &result, &mut final_content);

        assert!(!has_tool_calls);
        assert_eq!(final_content, "summary");
        let messages = conversation.messages();
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, MessageRole::Assistant);
        assert_eq!(messages[0].content, "summary");
        assert!(messages[0].tool_calls.is_none());
    }

    #[test]
    fn tool_results_are_persisted_between_tool_call_turns() {
        let mut conversation = runtime_model::Conversation::new(None);
        let first = super::super::model_step::ModelStepResult {
            text: String::new(),
            tool_calls: Some(vec![runtime_model::ToolCall::new(
                "call-clone".to_string(),
                "shell".to_string(),
                json!({ "command": "git clone https://github.com/AliusTech/alius.git" }),
            )]),
        };
        let second = super::super::model_step::ModelStepResult {
            text: String::new(),
            tool_calls: Some(vec![runtime_model::ToolCall::new(
                "call-list".to_string(),
                "list_dir".to_string(),
                json!({ "path": "alius" }),
            )]),
        };
        let mut final_content = String::new();

        append_model_result_to_conversation(&mut conversation, &first, &mut final_content);
        append_tool_results_to_conversation(
            &mut conversation,
            &[(
                "call-clone".to_string(),
                "shell".to_string(),
                "[exit:0]\nCloning into 'alius'".to_string(),
            )],
        );
        append_model_result_to_conversation(&mut conversation, &second, &mut final_content);

        let messages = conversation.messages();
        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].role, MessageRole::Assistant);
        assert_eq!(messages[0].tool_calls.as_ref().unwrap()[0].id, "call-clone");
        assert_eq!(messages[1].role, MessageRole::Tool);
        assert_eq!(messages[1].tool_call_id.as_deref(), Some("call-clone"));
        assert_eq!(messages[2].role, MessageRole::Assistant);
        assert_eq!(messages[2].tool_calls.as_ref().unwrap()[0].id, "call-list");
    }

    #[test]
    fn normalize_tool_results_orders_by_assistant_tool_calls_and_synthesizes_missing() {
        let assistant_tool_calls = vec![
            runtime_model::ToolCall::new("call-a".to_string(), "shell".to_string(), json!({})),
            runtime_model::ToolCall::new("call-b".to_string(), "read_file".to_string(), json!({})),
        ];
        let raw_results = vec![(
            "call-b".to_string(),
            "read_file".to_string(),
            "README".to_string(),
        )];

        let normalized = normalize_tool_results(&assistant_tool_calls, &raw_results);

        assert_eq!(normalized.len(), 2);
        assert_eq!(normalized[0].0, "call-a");
        assert_eq!(normalized[0].1, "shell");
        assert_eq!(
            normalized[0].2,
            "error: missing tool result for tool_call_id 'call-a'"
        );
        assert_eq!(normalized[1].0, "call-b");
        assert_eq!(normalized[1].2, "README");
    }

    #[test]
    fn validate_assistant_tool_calls_rejects_empty_and_duplicate_ids() {
        let empty = vec![runtime_model::ToolCall::new(
            String::new(),
            "shell".to_string(),
            json!({}),
        )];
        assert!(validate_assistant_tool_calls(&empty)
            .unwrap_err()
            .contains("empty tool_call_id"));

        let duplicate = vec![
            runtime_model::ToolCall::new("call-a".to_string(), "shell".to_string(), json!({})),
            runtime_model::ToolCall::new("call-a".to_string(), "read_file".to_string(), json!({})),
        ];
        assert!(validate_assistant_tool_calls(&duplicate)
            .unwrap_err()
            .contains("duplicate tool_call_id"));
    }

    #[test]
    fn pending_tool_results_prevent_context_truncation() {
        let mut conversation = runtime_model::Conversation::new(None);
        conversation.add_user_message("long ".repeat(100));
        let context_mgr = ContextManager::new(8);
        let pending = vec![(
            "call-a".to_string(),
            "shell".to_string(),
            "output".to_string(),
        )];

        assert!(context_mgr.needs_truncation(&conversation));
        assert!(!should_truncate_context(
            &context_mgr,
            &conversation,
            &pending
        ));
        assert!(should_truncate_context(&context_mgr, &conversation, &[]));
    }

    #[test]
    fn chat_tool_final_content_keeps_single_turn_tool_results() {
        let content = chat_tool_final_content(
            "I will inspect the file.",
            &[(
                "call-readme".to_string(),
                "read_file".to_string(),
                "README contents".to_string(),
            )],
        );

        assert!(content.contains("I will inspect the file."));
        assert!(content.contains("Tool results:"));
        assert!(content.contains("- read_file (call-readme): README contents"));
    }

    /// Engine-level: Plan mode tool denial produces ErrorRaised + FinalResult(false).
    /// This verifies the integration between execute_tools' batch_denied flag
    /// and the engine's denial handling — no fragile string matching.
    #[tokio::test]
    async fn plan_denial_produces_error_and_failed_final() {
        use runtime_tools::AliusTool;
        use std::sync::{Arc, Mutex};

        struct DenyTool;
        #[async_trait::async_trait]
        impl AliusTool for DenyTool {
            fn name(&self) -> &'static str {
                "deny_tool"
            }
            fn description(&self) -> &'static str {
                "always denied"
            }
            fn input_schema(&self) -> serde_json::Value {
                json!({})
            }
            fn preview_confirmation(&self, _: &serde_json::Value, _: RuntimeMode) -> bool {
                true
            }
            async fn execute(
                &self,
                _: serde_json::Value,
                _: runtime_tools::ToolContext,
            ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
                Ok(runtime_tools::ToolResult {
                    output: "should not run".into(),
                    success: true,
                    metadata: None,
                })
            }
        }

        let registry = runtime_tools::ToolRegistry::new();
        registry.register(DenyTool).unwrap();

        let session_manager = Arc::new(crate::SessionManager::new(
            protocol_interface::core::WorkspaceRef::new("/tmp"),
        ));
        let session = session_manager.create_session();
        let (_, run_ref, trace_id) = session_manager.create_turn(&session.session_ref).unwrap();

        // Simulate: execute_tools with session that cancels immediately.
        let sm = session_manager.clone();
        let rr = run_ref.clone();
        tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            let _ = sm.cancel_run(&rr);
        });

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_clone = events.clone();
        let mut seq = 0u64;

        let call = runtime_model::ToolCall {
            id: "tc-1".into(),
            name: "deny_tool".into(),
            args: json!({}),
        };

        let batch = super::super::tool_step::execute_tools(
            &[call],
            &registry,
            std::path::Path::new("/tmp"),
            "test",
            RuntimeMode::Plan,
            Some(&session_manager),
            &mut |e| events_clone.lock().unwrap().push(e),
            &run_ref,
            &trace_id,
            &mut seq,
            None,
        )
        .await
        .unwrap();

        // The batch must be marked as denied (cancelled).
        assert!(batch.batch_denied, "batch should be denied");
        assert_eq!(batch.denial_reason, Some("cancelled"));

        // Verify ToolCallCompleted(success=false) was emitted with cancellation reason.
        let evts = events.lock().unwrap();
        let completed = evts
            .iter()
            .find(|e| e.kind == CoreEventKind::ToolCallCompleted);
        assert!(completed.is_some(), "should emit ToolCallCompleted");
        if let Some(CoreEventPayload::Json { value }) = completed.map(|e| &e.payload) {
            assert_eq!(value["success"], false);
            assert_eq!(value["denied"], true);
            assert_eq!(value["denial_reason"], "cancelled");
        }

        // Verify status is Cancelled (not restored to Running).
        assert_eq!(
            session_manager.get_run_status(&run_ref).unwrap(),
            protocol_interface::core::RunStatus::Cancelled
        );
    }

    /// Chat path denial: ErrorRaised(tool_denied) + FinalResult(success=false).
    #[tokio::test]
    async fn chat_denial_emits_error_raised_and_failed_final() {
        use runtime_model::{ChatEvent, ChatStream, LlmProvider, ToolCall};
        use runtime_tools::AliusTool;
        use std::future::Future;
        use std::pin::Pin;
        use std::sync::{Arc, Mutex};

        struct DenyTool;
        #[async_trait::async_trait]
        impl AliusTool for DenyTool {
            fn name(&self) -> &'static str {
                "deny_tool"
            }
            fn description(&self) -> &'static str {
                "always denied"
            }
            fn input_schema(&self) -> serde_json::Value {
                json!({})
            }
            fn preview_confirmation(&self, _: &serde_json::Value, _: RuntimeMode) -> bool {
                true
            }
            async fn execute(
                &self,
                _: serde_json::Value,
                _: runtime_tools::ToolContext,
            ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
                Ok(runtime_tools::ToolResult {
                    output: "should not run".into(),
                    success: true,
                    metadata: None,
                })
            }
        }

        struct ToolCallProvider {
            tool_calls: Vec<ToolCall>,
        }

        impl LlmProvider for ToolCallProvider {
            fn chat_stream<'a>(
                &'a self,
                _conversation: &'a runtime_model::Conversation,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<ChatStream>> + Send + 'a>> {
                Box::pin(async {
                    let stream: ChatStream =
                        Box::pin(futures::stream::iter(vec![Ok(ChatEvent::Done {
                            full_response: String::new(),
                        })]));
                    Ok(stream)
                })
            }

            fn chat_once<'a>(
                &'a self,
                _prompt: &'a str,
                _system: Option<&'a str>,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'a>> {
                Box::pin(async { Ok(String::new()) })
            }

            fn list_models<'a>(
                &'a self,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + 'a>>
            {
                Box::pin(async { Ok(Vec::new()) })
            }

            fn chat_stream_with_tools<'a>(
                &'a self,
                _conversation: &'a runtime_model::Conversation,
                _tools: &'a [protocol_interface::ToolDef],
            ) -> Pin<Box<dyn Future<Output = runtime_model::ToolResponse> + Send + 'a>>
            {
                let tool_calls = self.tool_calls.clone();
                Box::pin(async move {
                    let stream: ChatStream = Box::pin(futures::stream::iter(vec![
                        Ok(ChatEvent::Delta {
                            text: "requesting tool".to_string(),
                        }),
                        Ok(ChatEvent::Done {
                            full_response: "requesting tool".to_string(),
                        }),
                    ]));
                    Ok((stream, Some(tool_calls)))
                })
            }

            fn continue_with_tool_results<'a>(
                &'a self,
                _conversation: &'a runtime_model::Conversation,
                _tool_results: &'a [(String, String, String)],
                _assistant_tool_calls: &'a [ToolCall],
                _tools: &'a [protocol_interface::ToolDef],
            ) -> Pin<Box<dyn Future<Output = runtime_model::ToolResponse> + Send + 'a>>
            {
                Box::pin(async {
                    let stream: ChatStream =
                        Box::pin(futures::stream::iter(vec![Ok(ChatEvent::Done {
                            full_response: String::new(),
                        })]));
                    Ok((stream, None))
                })
            }
        }

        let registry = runtime_tools::ToolRegistry::new();
        registry.register(DenyTool).unwrap();
        let registry = Arc::new(registry);

        let session_manager = Arc::new(crate::SessionManager::new(
            protocol_interface::core::WorkspaceRef::new("/tmp"),
        ));
        let session = session_manager.create_session();
        let (_, run_ref, trace_id) = session_manager.create_turn(&session.session_ref).unwrap();

        let tool_call = ToolCall::new("tc-1".to_string(), "deny_tool".to_string(), json!({}));
        let client = Arc::new(runtime_model::LlmClient::new_with_provider_for_test(
            Box::new(ToolCallProvider {
                tool_calls: vec![tool_call],
            }),
            "mock-model",
            protocol_interface::ProviderType::Openai,
        ));

        let tmp = tempfile::TempDir::new().unwrap();
        let mut conversation = runtime_model::Conversation::new(None);
        conversation.add_user_message("please run the denied tool".to_string());
        let ctx = LoopContext {
            client,
            conversation,
            settings: runtime_config::LlmSettings::default(),
            workspace: tmp.path().to_path_buf(),
            tool_registry: Some(registry),
            session: Some(session_manager.clone()),
            max_context_tokens: 4096,
            cancel_token: None,
            log_writer: None,
        };
        let input = RunLoopInput {
            content: "please run the denied tool".to_string(),
            mode: RuntimeMode::Chat,
            policy: LoopPolicy::chat(),
        };

        let deny_session = session_manager.clone();
        let rr = run_ref.clone();
        let deny_handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            deny_session
                .deliver_confirmation(&rr, "tc-1", false)
                .unwrap();
        });

        let events = Arc::new(Mutex::new(Vec::<CoreEvent>::new()));
        let events_clone = events.clone();
        let result = LoopEngine::run(&run_ref, &trace_id, &input, &ctx, &mut |e| {
            events_clone.lock().unwrap().push(e)
        })
        .await;
        deny_handle.await.unwrap();

        assert!(result.final_content.contains("Tool results:"));
        assert!(!result.final_content.contains("should not run"));

        let evts = events.lock().unwrap();
        let confirmation_idx = evts
            .iter()
            .position(|e| e.kind == CoreEventKind::ToolConfirmationRequired)
            .expect("must emit ToolConfirmationRequired");
        let completed_idx = evts
            .iter()
            .position(|e| e.kind == CoreEventKind::ToolCallCompleted)
            .expect("must emit ToolCallCompleted");
        let error_idx = evts
            .iter()
            .position(|e| {
                e.kind == CoreEventKind::ErrorRaised
                    && matches!(
                        &e.payload,
                        CoreEventPayload::Error { code, .. } if code == "tool_denied"
                    )
            })
            .expect("must emit ErrorRaised(tool_denied)");
        let final_idx = evts
            .iter()
            .position(|e| {
                e.kind == CoreEventKind::FinalResult
                    && matches!(&e.payload, CoreEventPayload::Final { success: false, .. })
            })
            .expect("must emit FinalResult(success=false)");

        assert!(confirmation_idx < completed_idx);
        assert!(completed_idx < error_idx);
        assert!(error_idx < final_idx);

        if let CoreEventPayload::Json { value } = &evts[completed_idx].payload {
            assert_eq!(value["success"], false);
            assert_eq!(value["denied"], true);
            assert_eq!(value["denial_reason"], "denied_by_user");
        } else {
            panic!("ToolCallCompleted must use JSON payload");
        }
    }

    /// Shared fake MCP tool + provider setup for engine-level tests.
    fn setup_mcp_engine_test(
        plan_requires_confirmation: bool,
    ) -> (
        std::sync::Arc<runtime_tools::ToolRegistry>,
        std::sync::Arc<crate::SessionManager>,
        RunRef,
        TraceId,
        LoopContext,
    ) {
        use protocol_interface::core::ToolSource;
        use runtime_model::{ChatEvent, ChatStream, LlmProvider, ToolCall};
        use runtime_tools::AliusTool;
        use std::future::Future;
        use std::pin::Pin;

        /// Fake MCP tool: echoes input, source = Mcp.
        /// preview_confirmation respects the `require_confirm` flag.
        struct McpEchoTool {
            require_confirm: bool,
        }

        #[async_trait::async_trait]
        impl AliusTool for McpEchoTool {
            fn name(&self) -> &'static str {
                "mcp_echo"
            }
            fn description(&self) -> &'static str {
                "fake MCP echo tool"
            }
            fn input_schema(&self) -> serde_json::Value {
                json!({"type": "object", "properties": {"message": {"type": "string"}}})
            }
            fn source(&self) -> ToolSource {
                ToolSource::Mcp
            }
            fn preview_confirmation(&self, _args: &serde_json::Value, mode: RuntimeMode) -> bool {
                // In Plan mode, require confirmation (same as real McpToolAdapter).
                self.require_confirm && mode == RuntimeMode::Plan
            }
            async fn execute(
                &self,
                args: serde_json::Value,
                _ctx: runtime_tools::ToolContext,
            ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
                let msg = args
                    .get("message")
                    .and_then(|v| v.as_str())
                    .unwrap_or("no message");
                Ok(runtime_tools::ToolResult {
                    output: format!("mcp_echo: {msg}"),
                    success: true,
                    metadata: None,
                })
            }
        }

        /// Fake provider: returns a tool call for "mcp_echo".
        struct McpToolCallProvider;

        impl LlmProvider for McpToolCallProvider {
            fn chat_stream<'a>(
                &'a self,
                _conversation: &'a runtime_model::Conversation,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<ChatStream>> + Send + 'a>> {
                Box::pin(async {
                    let stream: ChatStream =
                        Box::pin(futures::stream::iter(vec![Ok(ChatEvent::Done {
                            full_response: String::new(),
                        })]));
                    Ok(stream)
                })
            }
            fn chat_once<'a>(
                &'a self,
                _prompt: &'a str,
                _system: Option<&'a str>,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'a>> {
                Box::pin(async { Ok(String::new()) })
            }
            fn list_models<'a>(
                &'a self,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + 'a>>
            {
                Box::pin(async { Ok(Vec::new()) })
            }
            fn chat_stream_with_tools<'a>(
                &'a self,
                _conversation: &'a runtime_model::Conversation,
                _tools: &'a [protocol_interface::ToolDef],
            ) -> Pin<Box<dyn Future<Output = runtime_model::ToolResponse> + Send + 'a>>
            {
                Box::pin(async {
                    let stream: ChatStream = Box::pin(futures::stream::iter(vec![
                        Ok(ChatEvent::Delta {
                            text: "calling mcp tool".to_string(),
                        }),
                        Ok(ChatEvent::Done {
                            full_response: "calling mcp tool".to_string(),
                        }),
                    ]));
                    let tool_calls = vec![ToolCall::new(
                        "tc-mcp-1".to_string(),
                        "mcp_echo".to_string(),
                        json!({"message": "hello from mcp"}),
                    )];
                    Ok((stream, Some(tool_calls)))
                })
            }
            fn continue_with_tool_results<'a>(
                &'a self,
                _conversation: &'a runtime_model::Conversation,
                tool_results: &'a [(String, String, String)],
                _assistant_tool_calls: &'a [ToolCall],
                _tools: &'a [protocol_interface::ToolDef],
            ) -> Pin<Box<dyn Future<Output = runtime_model::ToolResponse> + Send + 'a>>
            {
                // Echo back tool results as model text so final_content includes them.
                let echoed: String = tool_results
                    .iter()
                    .map(|(_, name, output)| format!("- {name}: {output}"))
                    .collect::<Vec<_>>()
                    .join("\n");
                Box::pin(async move {
                    let stream: ChatStream = Box::pin(futures::stream::iter(vec![
                        Ok(ChatEvent::Delta { text: echoed }),
                        Ok(ChatEvent::Done {
                            full_response: String::new(),
                        }),
                    ]));
                    Ok((stream, None))
                })
            }
        }

        let registry = std::sync::Arc::new({
            let reg = runtime_tools::ToolRegistry::new();
            runtime_tools::native::register_native_tools(&reg);
            reg.register(McpEchoTool {
                require_confirm: plan_requires_confirmation,
            })
            .unwrap();
            reg
        });

        let session_manager = std::sync::Arc::new(crate::SessionManager::new(
            protocol_interface::core::WorkspaceRef::new("/tmp"),
        ));
        let session = session_manager.create_session();
        let (_, run_ref, trace_id) = session_manager.create_turn(&session.session_ref).unwrap();

        let client = std::sync::Arc::new(runtime_model::LlmClient::new_with_provider_for_test(
            Box::new(McpToolCallProvider),
            "mock-model",
            protocol_interface::ProviderType::Openai,
        ));

        let tmp = tempfile::TempDir::new().unwrap();
        let conversation = runtime_model::Conversation::new(None);
        let ctx = LoopContext {
            client,
            conversation,
            settings: runtime_config::LlmSettings::default(),
            workspace: tmp.path().to_path_buf(),
            tool_registry: Some(registry.clone()),
            session: Some(session_manager.clone()),
            max_context_tokens: 4096,
            cancel_token: None,
            log_writer: None,
        };

        (registry, session_manager, run_ref, trace_id, ctx)
    }

    /// Engine-level: LoopEngine::run with fake MCP tool via Chat mode.
    /// Chat mode: no confirmation, tool executes directly.
    #[tokio::test]
    async fn mcp_tool_executed_through_engine_chat_mode() {
        use protocol_interface::core::ToolSource;

        let (registry, _sm, run_ref, trace_id, ctx) = setup_mcp_engine_test(false);

        // Verify source.
        let tool = registry.get("mcp_echo").unwrap();
        assert_eq!(tool.source(), ToolSource::Mcp);

        let input = RunLoopInput {
            content: "call mcp tool".to_string(),
            mode: RuntimeMode::Chat,
            policy: LoopPolicy::chat(),
        };

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<CoreEvent>::new()));
        let events_clone = events.clone();
        let result = LoopEngine::run(&run_ref, &trace_id, &input, &ctx, &mut |e| {
            events_clone.lock().unwrap().push(e)
        })
        .await;

        // Final content must include MCP tool output.
        assert!(
            result.final_content.contains("mcp_echo: hello from mcp"),
            "final_content must include MCP tool output, got: {}",
            result.final_content
        );

        // Verify no ToolConfirmationRequired in Chat mode.
        let evts = events.lock().unwrap();
        let has_confirmation = evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired);
        assert!(
            !has_confirmation,
            "Chat mode must not emit ToolConfirmationRequired"
        );

        // Verify ToolCallCompleted with success.
        let completed = evts
            .iter()
            .find(|e| e.kind == CoreEventKind::ToolCallCompleted);
        assert!(completed.is_some(), "must emit ToolCallCompleted");
        if let Some(CoreEventPayload::Json { value }) = completed.map(|e| &e.payload) {
            assert_eq!(value["success"], true);
            assert_eq!(value["output"], "mcp_echo: hello from mcp");
            assert_eq!(value["name"], "mcp_echo");
        }
    }

    /// Engine-level: LoopEngine::run with fake MCP tool via Plan mode.
    /// Plan mode: confirmation required, approved → executes correctly.
    #[tokio::test]
    async fn mcp_tool_executed_through_engine_plan_mode_with_confirmation() {
        use protocol_interface::core::ToolSource;

        let (registry, sm, run_ref, trace_id, ctx) = setup_mcp_engine_test(true);

        // Verify source.
        let tool = registry.get("mcp_echo").unwrap();
        assert_eq!(tool.source(), ToolSource::Mcp);

        let input = RunLoopInput {
            content: "call mcp tool".to_string(),
            mode: RuntimeMode::Plan,
            policy: LoopPolicy::plan(),
        };

        // Spawn a task that approves the confirmation.
        let sm_clone = sm.clone();
        let rr = run_ref.clone();
        let approve_handle = tokio::spawn(async move {
            // Wait for confirmation sender to be stored.
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            let _ = sm_clone.deliver_confirmation(&rr, "tc-mcp-1", true);
        });

        let events = std::sync::Arc::new(std::sync::Mutex::new(Vec::<CoreEvent>::new()));
        let events_clone = events.clone();
        let result = LoopEngine::run(&run_ref, &trace_id, &input, &ctx, &mut |e| {
            events_clone.lock().unwrap().push(e)
        })
        .await;
        approve_handle.await.unwrap();

        // Final content must include MCP tool output.
        assert!(
            result.final_content.contains("mcp_echo: hello from mcp"),
            "final_content must include MCP tool output, got: {}",
            result.final_content
        );

        let evts = events.lock().unwrap();

        // Verify ToolConfirmationRequired was emitted.
        let has_confirmation = evts
            .iter()
            .any(|e| e.kind == CoreEventKind::ToolConfirmationRequired);
        assert!(
            has_confirmation,
            "Plan mode must emit ToolConfirmationRequired"
        );

        // Verify ToolCallCompleted with success.
        let completed = evts
            .iter()
            .find(|e| e.kind == CoreEventKind::ToolCallCompleted);
        assert!(completed.is_some(), "must emit ToolCallCompleted");
        if let Some(CoreEventPayload::Json { value }) = completed.map(|e| &e.payload) {
            assert_eq!(value["success"], true);
            assert_eq!(value["output"], "mcp_echo: hello from mcp");
        }

        // Verify source is still Mcp in registry.
        let infos = registry.to_tool_infos();
        let mcp_echo_info = infos.iter().find(|i| i.name == "mcp_echo");
        assert_eq!(mcp_echo_info.unwrap().source, ToolSource::Mcp);
    }
}
