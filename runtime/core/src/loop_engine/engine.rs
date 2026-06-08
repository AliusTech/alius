//! Loop engine orchestration.

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

        if input.mode == RuntimeMode::Chat || !input.policy.tools_enabled {
            Self::run_chat(run_ref, trace_id, ctx, event_sink).await
        } else {
            Self::run_plan(run_ref, trace_id, input, ctx, event_sink).await
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

        loop {
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

            // Context window management
            if context_mgr.needs_truncation(&conversation) {
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
                super::model_step::continue_with_tool_results(
                    &ctx.client,
                    &conversation,
                    &pending_tool_results,
                    tools.clone(),
                    event_sink,
                    run_ref,
                    trace_id,
                    &mut seq,
                )
                .await
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

            // Add assistant text to conversation
            if !model_result.text.is_empty() {
                conversation.add_assistant_message(model_result.text.clone());
                final_content = model_result.text.clone();
            }

            // Check if model wants to call tools
            let has_tool_calls = model_result
                .tool_calls
                .as_ref()
                .is_some_and(|tc| !tc.is_empty());

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
            let tool_calls = model_result.tool_calls.unwrap();

            // Add assistant message with tool call intent to conversation
            let tool_names: Vec<&str> = tool_calls.iter().map(|tc| tc.name.as_str()).collect();
            let tool_call_text = format!("Calling tools: {}", tool_names.join(", "));
            if model_result.text.is_empty() {
                conversation.add_assistant_message(tool_call_text);
            }

            let tool_results = match super::tool_step::execute_tools(
                &tool_calls,
                &registry,
                &ctx.workspace,
                &format!("plan-run-{}", run_ref.as_str()),
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

            pending_tool_results = tool_results;
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
