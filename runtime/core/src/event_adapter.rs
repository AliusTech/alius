//! Event adapter — maps model/agent events to CoreEvent.

use protocol_interface::core::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChatEventProjection {
    Delta { text: String },
    Done { full_response: String },
    Error { message: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AgentEventProjection {
    ToolCallStarted { id: String, name: String },
    ToolCallFinished { name: String, result: String },
    ModelDelta { text: String },
    ModelFinished { full_response: String },
    Error { message: String },
}

/// Maps provider and agent events to CoreEvent stream.
pub struct EventAdapter;

impl EventAdapter {
    /// Convert a ChatEvent into a CoreEvent.
    pub fn from_chat_event(
        event: &ChatEventProjection,
        run_ref: &RunRef,
        trace_id: &TraceId,
        sequence: u64,
    ) -> Option<CoreEvent> {
        match event {
            ChatEventProjection::Delta { text } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::ModelDelta,
                CoreEventPayload::Text { text: text.clone() },
            )),
            ChatEventProjection::Done { .. } => None,
            ChatEventProjection::Error { message } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::ErrorRaised,
                CoreEventPayload::Error {
                    code: "chat_error".to_string(),
                    message: message.clone(),
                },
            )),
        }
    }

    /// Convert an AgentEvent into a CoreEvent.
    pub fn from_agent_event(
        event: &AgentEventProjection,
        run_ref: &RunRef,
        trace_id: &TraceId,
        sequence: u64,
    ) -> Option<CoreEvent> {
        match event {
            AgentEventProjection::ToolCallStarted { id, name } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::ToolCallStarted,
                CoreEventPayload::Json {
                    value: serde_json::json!({
                        "tool_call_id": id,
                        "tool_name": name,
                    }),
                },
            )),
            AgentEventProjection::ToolCallFinished { name, result } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::ToolCallCompleted,
                CoreEventPayload::Text {
                    text: format!("{}: {}", name, result),
                },
            )),
            AgentEventProjection::ModelDelta { text } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::ModelDelta,
                CoreEventPayload::Text { text: text.clone() },
            )),
            AgentEventProjection::ModelFinished { full_response } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::FinalResult,
                CoreEventPayload::Final {
                    content: full_response.clone(),
                    success: true,
                },
            )),
            AgentEventProjection::Error { message } => Some(CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                sequence,
                CoreEventKind::ErrorRaised,
                CoreEventPayload::Error {
                    code: "agent_error".to_string(),
                    message: message.clone(),
                },
            )),
        }
    }

    /// Create a stub TurnCompleted event for testing.
    pub fn stub_completed(run_ref: &RunRef, trace_id: &TraceId, content: &str) -> CoreEvent {
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            0,
            CoreEventKind::FinalResult,
            CoreEventPayload::Final {
                content: content.to_string(),
                success: true,
            },
        )
    }

    /// Create a stub TurnStarted event for testing.
    pub fn stub_started(run_ref: &RunRef, trace_id: &TraceId) -> CoreEvent {
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            0,
            CoreEventKind::TurnStarted,
            CoreEventPayload::Empty,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_chat_event_delta() {
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();
        let event = ChatEventProjection::Delta {
            text: "hello".to_string(),
        };

        let core = EventAdapter::from_chat_event(&event, &run_ref, &trace_id, 1).unwrap();
        assert_eq!(core.kind, CoreEventKind::ModelDelta);
        assert_eq!(core.sequence, 1);
        assert_eq!(core.run_ref, run_ref);
    }

    #[test]
    fn from_chat_event_done_returns_none() {
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();
        let event = ChatEventProjection::Done {
            full_response: "done".to_string(),
        };
        assert!(EventAdapter::from_chat_event(&event, &run_ref, &trace_id, 2).is_none());
    }

    #[test]
    fn from_agent_event_tool_started() {
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();
        let event = AgentEventProjection::ToolCallStarted {
            id: "tc-1".to_string(),
            name: "read_file".to_string(),
        };

        let core = EventAdapter::from_agent_event(&event, &run_ref, &trace_id, 3).unwrap();
        assert_eq!(core.kind, CoreEventKind::ToolCallStarted);
    }

    #[test]
    fn from_agent_event_tool_finished() {
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();
        let event = AgentEventProjection::ToolCallFinished {
            name: "read_file".to_string(),
            result: "ok".to_string(),
        };

        let core = EventAdapter::from_agent_event(&event, &run_ref, &trace_id, 4).unwrap();
        assert_eq!(core.kind, CoreEventKind::ToolCallCompleted);
    }

    #[test]
    fn stub_events() {
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();

        let started = EventAdapter::stub_started(&run_ref, &trace_id);
        assert_eq!(started.kind, CoreEventKind::TurnStarted);

        let completed = EventAdapter::stub_completed(&run_ref, &trace_id, "done");
        assert_eq!(completed.kind, CoreEventKind::FinalResult);
    }
}
