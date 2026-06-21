//! Protocol Interface Layer.
//!
//! This crate provides the minimal Direct Rust API gateway between product
//! entrypoints and a `CoreRuntimeApi` implementation. It validates protocol
//! envelopes, enforces origin capability ceilings, delegates to Core Runtime,
//! and wraps Core events back into protocol envelopes.

use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use crate::core::*;

/// Lightweight context for non-start protocol operations.
#[derive(Debug, Clone)]
pub struct ProtocolContext {
    pub origin: Origin,
    pub capability_scope: CapabilityScope,
    pub workspace_root: Option<std::path::PathBuf>,
}

/// Stored protocol context for a run started through this interface.
#[derive(Debug, Clone)]
pub struct ProtocolRunContext {
    pub origin: Origin,
    pub capability_scope: CapabilityScope,
    pub workspace_root: Option<std::path::PathBuf>,
    pub session_ref: Option<SessionRef>,
    pub trace_id: TraceId,
}

impl ProtocolRunContext {
    fn from_request_envelope(envelope: &ProtocolEnvelope<CoreRequest>) -> Self {
        Self {
            origin: envelope.origin.clone(),
            capability_scope: envelope.capability_scope.clone(),
            workspace_root: envelope.workspace_root.clone(),
            session_ref: envelope.session_ref.clone(),
            trace_id: envelope.trace_id.clone(),
        }
    }
}

/// Minimal Protocol Interface gateway for Direct Rust API callers.
pub struct ProtocolInterface<R> {
    runtime: Arc<R>,
    runs: RwLock<HashMap<String, ProtocolRunContext>>,
}

impl<R> ProtocolInterface<R> {
    /// Create a protocol interface around an owned Core Runtime implementation.
    pub fn new(runtime: R) -> Self {
        Self::from_shared(Arc::new(runtime))
    }

    /// Create a protocol interface around a shared Core Runtime implementation.
    pub fn from_shared(runtime: Arc<R>) -> Self {
        Self {
            runtime,
            runs: RwLock::new(HashMap::new()),
        }
    }

    /// Access the wrapped runtime for integration code and tests.
    pub fn runtime(&self) -> Arc<R> {
        self.runtime.clone()
    }
}

impl<R> ProtocolInterface<R>
where
    R: CoreRuntimeApi,
    R::EventStream: IntoIterator<Item = CoreEvent>,
{
    /// Start a Core request after protocol envelope validation.
    pub fn start(&self, envelope: ProtocolEnvelope<CoreRequest>) -> Result<RunRef, ProtocolError> {
        self.validate_request_envelope(&envelope)?;

        let context = ProtocolRunContext::from_request_envelope(&envelope);
        let run_ref = self.runtime.start(envelope)?;

        let mut runs = self
            .runs
            .write()
            .map_err(|_| ProtocolError::Internal("protocol run context poisoned".to_string()))?;
        runs.insert(run_ref.as_str().to_string(), context);

        Ok(run_ref)
    }

    /// Send a command to a running Core run after protocol envelope validation.
    pub fn send(&self, envelope: ProtocolEnvelope<CoreCommand>) -> Result<(), ProtocolError> {
        self.validate_command_envelope(&envelope)?;
        self.runtime.send(envelope)
    }

    /// Start streaming execution — delegates to runtime's start_streaming.
    pub fn start_streaming(
        &self,
        envelope: ProtocolEnvelope<CoreRequest>,
    ) -> Result<(RunRef, tokio::sync::mpsc::UnboundedReceiver<CoreEvent>), ProtocolError> {
        self.validate_request_envelope(&envelope)?;

        let context = ProtocolRunContext::from_request_envelope(&envelope);
        let (run_ref, rx) = self.runtime.start_streaming(envelope)?;

        let mut runs = self
            .runs
            .write()
            .map_err(|_| ProtocolError::Internal("protocol run context poisoned".to_string()))?;
        runs.insert(run_ref.as_str().to_string(), context);

        Ok((run_ref, rx))
    }

    /// Subscribe to a run and wrap Core events back into protocol envelopes.
    pub fn subscribe(
        &self,
        run_ref: &RunRef,
    ) -> Result<Vec<ProtocolEnvelope<CoreEvent>>, ProtocolError> {
        let context = self.context_for_run(run_ref)?;
        let events = self.runtime.subscribe(run_ref)?;

        Ok(events
            .into_iter()
            .map(|event| self.wrap_event(&context, event))
            .collect())
    }

    /// Convenience command for cancelling a run started through this interface.
    pub fn cancel(&self, run_ref: &RunRef, reason: Option<String>) -> Result<(), ProtocolError> {
        let context = self.context_for_run(run_ref)?;
        let command = CoreCommand::cancel(run_ref.clone(), reason);
        let envelope = self.command_envelope(&context, command);
        self.send(envelope)
    }

    /// Convenience command for responding to a tool confirmation request.
    pub fn respond_confirmation(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        approved: bool,
    ) -> Result<(), ProtocolError> {
        let context = self.context_for_run(run_ref)?;
        let command = CoreCommand::respond_confirmation(run_ref.clone(), tool_call_id, approved);
        let envelope = self.command_envelope(&context, command);
        self.send(envelope)
    }

    /// Return stored protocol context for a run.
    pub fn run_context(&self, run_ref: &RunRef) -> Result<ProtocolRunContext, ProtocolError> {
        self.context_for_run(run_ref)
    }

    // === Non-start delegation methods ===

    /// Read current runtime configuration.
    pub fn config_read(&self, ctx: &ProtocolContext) -> Result<ConfigSnapshot, ProtocolError> {
        require_capability(ctx, Capability::ReadConfig)?;
        self.runtime.config_read()
    }

    /// Update a configuration key.
    pub fn config_update(
        &self,
        ctx: &ProtocolContext,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), ProtocolError> {
        require_capability(ctx, Capability::WriteConfig)?;
        self.runtime.config_update(key, value)
    }

    /// List available models from the current provider.
    pub fn model_list(&self, ctx: &ProtocolContext) -> Result<Vec<ModelInfo>, ProtocolError> {
        require_capability(ctx, Capability::UseModel)?;
        self.runtime.model_list()
    }

    /// Run a health check on the runtime.
    pub fn health_check(&self, _ctx: &ProtocolContext) -> Result<HealthReport, ProtocolError> {
        self.runtime.health_check()
    }

    /// Close a session, preventing new turns.
    pub fn close_session(
        &self,
        ctx: &ProtocolContext,
        session_ref: &SessionRef,
    ) -> Result<(), ProtocolError> {
        require_capability(ctx, Capability::WriteWorkspace)?;
        self.runtime.close_session(session_ref)
    }

    /// Clear conversation history for a session.
    pub fn clear_conversation(
        &self,
        ctx: &ProtocolContext,
        session_ref: &SessionRef,
    ) -> Result<(), ProtocolError> {
        require_capability(ctx, Capability::WriteWorkspace)?;
        self.runtime.clear_conversation(session_ref)
    }

    /// Save a memory entry.
    pub fn memory_save(
        &self,
        ctx: &ProtocolContext,
        text: &str,
        tags: Vec<String>,
    ) -> Result<(), ProtocolError> {
        require_capability(ctx, Capability::WriteMemory)?;
        self.runtime.memory_save(text, tags)
    }

    /// List all memory entries.
    pub fn memory_list(&self, ctx: &ProtocolContext) -> Result<Vec<MemoryEntry>, ProtocolError> {
        require_capability(ctx, Capability::ReadMemory)?;
        self.runtime.memory_list()
    }

    /// Clear all memory entries.
    pub fn memory_clear(&self, ctx: &ProtocolContext) -> Result<(), ProtocolError> {
        require_capability(ctx, Capability::WriteMemory)?;
        self.runtime.memory_clear()
    }

    /// List available tools.
    pub fn tool_list(&self, ctx: &ProtocolContext) -> Result<Vec<ToolInfo>, ProtocolError> {
        require_capability(ctx, Capability::UseTools)?;
        self.runtime.tool_list()
    }

    /// Start a code review run for a session.
    pub fn review_start(
        &self,
        ctx: &ProtocolContext,
        session_ref: &SessionRef,
    ) -> Result<RunRef, ProtocolError> {
        require_capability(ctx, Capability::UseModel)?;
        self.runtime.review_start(session_ref)
    }

    /// Query log records.
    pub fn query_logs(
        &self,
        ctx: &ProtocolContext,
        query: LogQuery,
    ) -> Result<Vec<LogRecord>, ProtocolError> {
        require_capability(ctx, Capability::ReadConfig)?;
        self.runtime.query_logs(query)
    }

    fn validate_request_envelope(
        &self,
        envelope: &ProtocolEnvelope<CoreRequest>,
    ) -> Result<(), ProtocolError> {
        envelope.validate_protocol_version()?;
        envelope.payload.validate()?;
        validate_origin_capability_ceiling(&envelope.origin, &envelope.capability_scope)
    }

    fn validate_command_envelope(
        &self,
        envelope: &ProtocolEnvelope<CoreCommand>,
    ) -> Result<(), ProtocolError> {
        envelope.validate_protocol_version()?;
        validate_origin_capability_ceiling(&envelope.origin, &envelope.capability_scope)?;

        if let Some(run_ref) = &envelope.run_ref {
            if run_ref != &envelope.payload.target_run {
                return Err(ProtocolError::InvalidMessage(format!(
                    "command envelope run_ref {} does not match target_run {}",
                    run_ref, envelope.payload.target_run
                )));
            }
        }

        Ok(())
    }

    fn context_for_run(&self, run_ref: &RunRef) -> Result<ProtocolRunContext, ProtocolError> {
        let runs = self
            .runs
            .read()
            .map_err(|_| ProtocolError::Internal("protocol run context poisoned".to_string()))?;

        runs.get(run_ref.as_str())
            .cloned()
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))
    }

    fn command_envelope(
        &self,
        context: &ProtocolRunContext,
        command: CoreCommand,
    ) -> ProtocolEnvelope<CoreCommand> {
        ProtocolEnvelope {
            protocol_version: PROTOCOL_VERSION.to_string(),
            origin: context.origin.clone(),
            capability_scope: context.capability_scope.clone(),
            workspace_root: context.workspace_root.clone(),
            session_ref: context.session_ref.clone(),
            run_ref: Some(command.target_run.clone()),
            trace_id: context.trace_id.clone(),
            payload: command,
        }
    }

    fn wrap_event(
        &self,
        context: &ProtocolRunContext,
        event: CoreEvent,
    ) -> ProtocolEnvelope<CoreEvent> {
        ProtocolEnvelope {
            protocol_version: PROTOCOL_VERSION.to_string(),
            origin: context.origin.clone(),
            capability_scope: context.capability_scope.clone(),
            workspace_root: context.workspace_root.clone(),
            session_ref: event
                .session_ref
                .clone()
                .or_else(|| context.session_ref.clone()),
            run_ref: Some(event.run_ref.clone()),
            trace_id: event.trace_id.clone(),
            payload: event,
        }
    }
}

fn require_capability(ctx: &ProtocolContext, cap: Capability) -> Result<(), ProtocolError> {
    if !ctx.capability_scope.has(cap) {
        return Err(ProtocolError::CapabilityDenied(format!(
            "origin {:?} lacks {:?} capability",
            ctx.origin, cap
        )));
    }
    Ok(())
}

fn validate_origin_capability_ceiling(
    origin: &Origin,
    scope: &CapabilityScope,
) -> Result<(), ProtocolError> {
    match origin {
        Origin::RemoteA2A => {
            deny_external_paths(origin, scope)?;
            deny_capabilities(
                origin,
                scope,
                &[
                    Capability::WriteWorkspace,
                    Capability::UseTools,
                    Capability::UseShell,
                    Capability::UseMcp,
                    Capability::WriteMemory,
                    Capability::WriteConfig,
                    Capability::RemoteA2A,
                ],
            )
        }
        _ => Ok(()),
    }
}

fn deny_external_paths(origin: &Origin, scope: &CapabilityScope) -> Result<(), ProtocolError> {
    if scope.allow_external_workspace_paths {
        return Err(ProtocolError::CapabilityDenied(format!(
            "{:?} cannot request external workspace path access",
            origin
        )));
    }
    Ok(())
}

fn deny_capabilities(
    origin: &Origin,
    scope: &CapabilityScope,
    denied: &[Capability],
) -> Result<(), ProtocolError> {
    for capability in denied {
        if scope.has(*capability) {
            return Err(ProtocolError::CapabilityDenied(format!(
                "{:?} cannot request {:?}",
                origin, capability
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRuntime;

    fn test_interface() -> ProtocolInterface<TestRuntime> {
        ProtocolInterface::new(TestRuntime::default())
    }

    fn start_envelope(input: &str) -> ProtocolEnvelope<CoreRequest> {
        ProtocolEnvelope::new(
            Origin::LocalTui,
            CapabilityScope::local_tui(),
            CoreRequest::start_turn(input).unwrap(),
        )
        .with_workspace_root("/tmp/protocol-interface-test")
    }

    #[test]
    fn start_delegates_to_core_and_subscribe_wraps_events() {
        let interface = test_interface();

        let run_ref = interface.start(start_envelope("hello")).unwrap();
        let events = interface.subscribe(&run_ref).unwrap();

        assert_eq!(events.len(), 5);
        assert!(events.iter().all(|event| event.origin == Origin::LocalTui));
        assert!(events
            .iter()
            .all(|event| event.run_ref.as_ref() == Some(&run_ref)));
        assert!(events
            .iter()
            .all(|event| event.trace_id == event.payload.trace_id));
        assert_eq!(events[0].payload.kind, CoreEventKind::TurnStarted);
        assert_eq!(events[1].payload.kind, CoreEventKind::RunStarted);
        assert_eq!(events[2].payload.kind, CoreEventKind::LoopIterationStarted);
        assert_eq!(events[3].payload.kind, CoreEventKind::ConvergenceChecked);
        assert_eq!(events[4].payload.kind, CoreEventKind::FinalResult);
    }

    #[test]
    fn start_rejects_unsupported_protocol_version() {
        let interface = test_interface();
        let mut envelope = start_envelope("hello");
        envelope.protocol_version = "0.9".to_string();

        let result = interface.start(envelope);

        assert!(matches!(
            result,
            Err(ProtocolError::UnsupportedVersion { .. })
        ));
    }

    #[test]
    fn start_rejects_remote_a2a_shell_capability() {
        let interface = test_interface();
        let mut scope = CapabilityScope::remote_a2a();
        scope.capabilities.push(Capability::UseShell);
        let envelope = ProtocolEnvelope::new(
            Origin::RemoteA2A,
            scope,
            CoreRequest::start_turn("hello").unwrap(),
        );

        let result = interface.start(envelope);

        assert!(matches!(result, Err(ProtocolError::CapabilityDenied(_))));
    }

    #[test]
    fn send_rejects_mismatched_run_ref() {
        let interface = test_interface();
        let command = CoreCommand::cancel(RunRef::new(), None);
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), command)
                .with_run_ref(RunRef::new());

        let result = interface.send(envelope);

        assert!(matches!(result, Err(ProtocolError::InvalidMessage(_))));
    }

    #[test]
    fn cancel_uses_stored_run_context() {
        let interface = test_interface();
        let run_ref = interface.start(start_envelope("hello")).unwrap();

        interface
            .cancel(&run_ref, Some("user cancelled".to_string()))
            .unwrap();

        let runtime = interface.runtime();
        let commands = runtime.commands.read().unwrap();

        assert_eq!(commands.len(), 1);
        assert_eq!(commands[0].origin, Origin::LocalTui);
        assert_eq!(commands[0].run_ref.as_ref(), Some(&run_ref));
        assert_eq!(commands[0].payload.target_run, run_ref);
    }

    fn local_tui_ctx() -> ProtocolContext {
        ProtocolContext {
            origin: Origin::LocalTui,
            capability_scope: CapabilityScope::local_tui(),
            workspace_root: Some("/tmp/test".into()),
        }
    }

    fn minimal_ctx() -> ProtocolContext {
        ProtocolContext {
            origin: Origin::RemoteA2A,
            capability_scope: CapabilityScope::remote_a2a(),
            workspace_root: None,
        }
    }

    #[test]
    fn config_read_delegates_to_runtime() {
        let interface = test_interface();
        let snapshot = interface.config_read(&local_tui_ctx()).unwrap();
        assert_eq!(snapshot.provider, "openai");
        assert_eq!(snapshot.model, "gpt-4o");
    }

    #[test]
    fn config_read_rejects_without_read_config() {
        let interface = test_interface();
        let result = interface.config_read(&minimal_ctx());
        assert!(matches!(result, Err(ProtocolError::CapabilityDenied(_))));
    }

    #[test]
    fn config_update_delegates_to_runtime() {
        let interface = test_interface();
        interface
            .config_update(
                &local_tui_ctx(),
                "llm.model",
                serde_json::Value::String("gpt-4o-mini".into()),
            )
            .unwrap();
    }

    #[test]
    fn config_update_rejects_without_write_config() {
        let interface = test_interface();
        let result = interface.config_update(
            &minimal_ctx(),
            "llm.model",
            serde_json::Value::String("gpt-4o-mini".into()),
        );
        assert!(matches!(result, Err(ProtocolError::CapabilityDenied(_))));
    }

    #[test]
    fn model_list_delegates_to_runtime() {
        let interface = test_interface();
        let models = interface.model_list(&local_tui_ctx()).unwrap();
        assert!(models.is_empty());
    }

    #[test]
    fn model_list_rejects_without_use_model() {
        let interface = test_interface();
        let mut scope = CapabilityScope::remote_a2a();
        scope.capabilities.retain(|c| *c != Capability::UseModel);
        let ctx = ProtocolContext {
            origin: Origin::RemoteA2A,
            capability_scope: scope,
            workspace_root: None,
        };
        let result = interface.model_list(&ctx);
        assert!(matches!(result, Err(ProtocolError::CapabilityDenied(_))));
    }

    #[test]
    fn health_check_works_without_capabilities() {
        let interface = test_interface();
        let report = interface.health_check(&minimal_ctx()).unwrap();
        assert!(report.config_ok);
    }
}
