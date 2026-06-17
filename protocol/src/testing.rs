//! Test utilities for protocol-interface.
//!
//! Provides [`TestRuntime`] — a minimal [`CoreRuntimeApi`] implementation
//! for testing protocol gateway behavior without a real runtime.

use std::collections::HashMap;
use std::sync::RwLock;

use crate::core::*;

/// A minimal [`CoreRuntimeApi`] implementation for testing.
///
/// Records commands sent through it and returns pre-built event sequences
/// for each `start()` call. All other methods return no-op or empty results.
#[derive(Default)]
pub struct TestRuntime {
    /// Events keyed by run_ref, returned from `subscribe()`.
    pub events: RwLock<HashMap<String, Vec<CoreEvent>>>,
    /// All command envelopes received via `send()`.
    pub commands: RwLock<Vec<ProtocolEnvelope<CoreCommand>>>,
}

impl TestRuntime {
    /// Create a new TestRuntime with empty state.
    pub fn new() -> Self {
        Self::default()
    }
}

impl CoreRuntimeApi for TestRuntime {
    type EventStream = Vec<CoreEvent>;

    fn start(&self, envelope: ProtocolEnvelope<CoreRequest>) -> Result<RunRef, ProtocolError> {
        let run_ref = RunRef::new();
        let trace_id = envelope.trace_id;
        let events = vec![
            CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                0,
                CoreEventKind::TurnStarted,
                CoreEventPayload::Empty,
            ),
            CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                1,
                CoreEventKind::RunStarted,
                CoreEventPayload::Empty,
            ),
            CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                2,
                CoreEventKind::LoopIterationStarted,
                CoreEventPayload::Empty,
            ),
            CoreEvent::new(
                run_ref.clone(),
                trace_id.clone(),
                3,
                CoreEventKind::ConvergenceChecked,
                CoreEventPayload::Empty,
            ),
            CoreEvent::final_result(run_ref.clone(), trace_id, "done"),
        ];

        self.events
            .write()
            .unwrap()
            .insert(run_ref.as_str().to_string(), events);
        Ok(run_ref)
    }

    fn send(&self, envelope: ProtocolEnvelope<CoreCommand>) -> Result<(), ProtocolError> {
        self.commands.write().unwrap().push(envelope);
        Ok(())
    }

    fn subscribe(&self, run_ref: &RunRef) -> Result<Self::EventStream, ProtocolError> {
        Ok(self
            .events
            .read()
            .unwrap()
            .get(run_ref.as_str())
            .cloned()
            .unwrap_or_default())
    }

    fn inspect(&self, _session_ref: &SessionRef) -> Result<SessionSnapshot, ProtocolError> {
        Err(ProtocolError::Internal(
            "not implemented in test".to_string(),
        ))
    }

    fn list_sessions(
        &self,
        _workspace_ref: &WorkspaceRef,
    ) -> Result<Vec<SessionSummary>, ProtocolError> {
        Ok(Vec::new())
    }

    fn query_logs(&self, _query: LogQuery) -> Result<Vec<LogRecord>, ProtocolError> {
        Ok(Vec::new())
    }

    fn close_session(&self, _session_ref: &SessionRef) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn clear_conversation(&self, _session_ref: &SessionRef) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn config_read(&self) -> Result<ConfigSnapshot, ProtocolError> {
        Ok(ConfigSnapshot {
            provider: "openai".to_string(),
            model: "gpt-4o".to_string(),
            base_url: None,
            soul: None,
            has_api_key: true,
        })
    }

    fn config_validate(&self) -> Result<ValidationResult, ProtocolError> {
        Err(ProtocolError::Internal(
            "not implemented in test".to_string(),
        ))
    }

    fn config_update(&self, _key: &str, _value: serde_json::Value) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn model_list(&self) -> Result<Vec<ModelInfo>, ProtocolError> {
        Ok(Vec::new())
    }

    fn memory_save(&self, _text: &str, _tags: Vec<String>) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn memory_list(&self) -> Result<Vec<MemoryEntry>, ProtocolError> {
        Ok(Vec::new())
    }

    fn memory_clear(&self) -> Result<(), ProtocolError> {
        Ok(())
    }

    fn tool_list(&self) -> Result<Vec<ToolInfo>, ProtocolError> {
        Ok(Vec::new())
    }

    fn review_start(&self, _session_ref: &SessionRef) -> Result<RunRef, ProtocolError> {
        Ok(RunRef::new())
    }

    fn health_check(&self) -> Result<HealthReport, ProtocolError> {
        Ok(HealthReport {
            config_ok: true,
            api_reachable: true,
            workspace_ok: true,
            errors: Vec::new(),
        })
    }
}
