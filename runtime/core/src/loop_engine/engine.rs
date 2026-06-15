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

        let tool_results = match super::tool_step::execute_tools(
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
        )
        .await
        {
            Ok(results) => normalize_tool_results(&tool_calls, &results),
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

        let final_content = chat_tool_final_content(&model_result.text, &tool_results);
        emit_final(
            event_sink,
            run_ref,
            trace_id,
            &mut seq,
            &final_content,
            true,
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

        let mut last_assistant_tool_calls: Option<Vec<runtime_model::ToolCall>> = None;
        loop {
            // Check for cancellation at the start of each iteration
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
                break;
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
                    break;
                }
            }

            let tool_results = match super::tool_step::execute_tools(
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
            )
            .await
            {
                Ok(results) => results,
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
                    break;
                }
            };

            pending_tool_results = normalize_tool_results(&tool_calls, &tool_results);
        }

        emit_final(
            event_sink,
            run_ref,
            trace_id,
            &mut seq,
            &final_content,
            true,
        );

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
}
