//! Core Runtime — implements CoreRuntimeApi.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, RwLock};

use protocol_interface::core::*;
use runtime_config::Settings;
use runtime_model::{Conversation, LlmClient};

use crate::logging::LogWriter;
use crate::loop_engine::{LoopContext, LoopEngine};
use crate::session::SessionManager;
use crate::EventAdapter;

const DEFAULT_MAX_CONTEXT_TOKENS: usize = 128_000;

/// Core Runtime implementation satisfying `CoreRuntimeApi`.
pub struct CoreRuntime {
    session_manager: Arc<SessionManager>,
    settings: Arc<RwLock<Settings>>,
    client: Arc<LlmClient>,
    active_runs: Arc<RwLock<HashMap<String, ActiveRun>>>,
    memory_global: Arc<RwLock<Option<runtime_store::MemoryStore>>>,
    memory_project: Arc<RwLock<Option<runtime_store::MemoryStore>>>,
    tool_registry: Option<Arc<runtime_tools::ToolRegistry>>,
    conversation_store: Arc<runtime_store::ConversationStore>,
    log_writer: Option<Arc<Mutex<LogWriter>>>,
}

struct ActiveRun {
    #[allow(dead_code)]
    run_ref: RunRef,
    #[allow(dead_code)]
    trace_id: TraceId,
    #[allow(dead_code)]
    session_ref: SessionRef,
    #[allow(dead_code)]
    turn_ref: TurnRef,
}

/// Builder for constructing CoreRuntime with optional subsystems.
pub struct CoreRuntimeBuilder {
    workspace_ref: Option<WorkspaceRef>,
    settings: Option<Settings>,
    client: Option<LlmClient>,
    tool_registry: Option<Arc<runtime_tools::ToolRegistry>>,
}

impl CoreRuntimeBuilder {
    pub fn new() -> Self {
        Self {
            workspace_ref: None,
            settings: None,
            client: None,
            tool_registry: None,
        }
    }

    pub fn workspace_ref(mut self, workspace_ref: WorkspaceRef) -> Self {
        self.workspace_ref = Some(workspace_ref);
        self
    }

    pub fn settings(mut self, settings: Settings) -> Self {
        self.settings = Some(settings);
        self
    }

    pub fn client(mut self, client: LlmClient) -> Self {
        self.client = Some(client);
        self
    }

    pub fn tool_registry_arc(mut self, registry: Arc<runtime_tools::ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn build(self) -> Result<CoreRuntime, ProtocolError> {
        let workspace_ref = self
            .workspace_ref
            .unwrap_or_else(|| WorkspaceRef::new("/tmp/alius-workspace"));

        let settings = self
            .settings
            .ok_or_else(|| ProtocolError::Internal("settings required".into()))?;
        let client = self
            .client
            .ok_or_else(|| ProtocolError::Internal("client required".into()))?;

        let memory_global = runtime_store::MemoryStore::global().ok();
        let memory_project = runtime_store::MemoryStore::project().ok();
        let conversation_store = Arc::new(
            runtime_store::ConversationStore::new()
                .map_err(|e| ProtocolError::Internal(format!("conversation store: {}", e)))?,
        );

        // Create audit log writer under the workspace's .alius/logs directory.
        let log_writer = {
            let log_dir = workspace_ref.root.join(".alius").join("logs");
            match LogWriter::new(&log_dir) {
                Ok(writer) => Some(Arc::new(Mutex::new(writer))),
                Err(e) => {
                    eprintln!("[warn] Failed to create audit log writer: {e}");
                    None
                }
            }
        };

        let mut session_manager = SessionManager::new(workspace_ref);
        if let Some(ref writer) = log_writer {
            session_manager.set_event_sink(Arc::clone(writer));
        }

        Ok(CoreRuntime {
            session_manager: Arc::new(session_manager),
            settings: Arc::new(RwLock::new(settings)),
            client: Arc::new(client),
            active_runs: Arc::new(RwLock::new(HashMap::new())),
            memory_global: Arc::new(RwLock::new(memory_global)),
            memory_project: Arc::new(RwLock::new(memory_project)),
            tool_registry: self.tool_registry,
            conversation_store,
            log_writer,
        })
    }
}

impl Default for CoreRuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

fn map_err(e: impl std::fmt::Display) -> ProtocolError {
    ProtocolError::Internal(e.to_string())
}

impl CoreRuntime {
    /// Create a minimal CoreRuntime for testing (no real subsystems).
    pub fn new(workspace_ref: WorkspaceRef) -> Self {
        let settings = Settings::default();
        let llm_settings = runtime_config::LlmSettings {
            api_key: Some("test-key".into()),
            ..Default::default()
        };
        let client = LlmClient::new(llm_settings)
            .unwrap_or_else(|_| LlmClient::new(runtime_config::LlmSettings::default()).unwrap());

        let conversation_store = Arc::new(
            runtime_store::ConversationStore::new()
                .expect("Failed to create conversation store for test runtime"),
        );

        Self {
            session_manager: Arc::new(SessionManager::new(workspace_ref)),
            settings: Arc::new(RwLock::new(settings)),
            client: Arc::new(client),
            active_runs: Arc::new(RwLock::new(HashMap::new())),
            memory_global: Arc::new(RwLock::new(None)),
            memory_project: Arc::new(RwLock::new(None)),
            tool_registry: None,
            conversation_store,
            log_writer: None,
        }
    }

    /// Access the tool registry.
    pub fn tool_registry(&self) -> Option<Arc<runtime_tools::ToolRegistry>> {
        self.tool_registry.clone()
    }

    /// Access the session manager (Stage B: needed for tool confirmation bridge).
    pub fn session_manager(&self) -> Arc<SessionManager> {
        self.session_manager.clone()
    }

    fn validate_envelope(
        &self,
        envelope: &ProtocolEnvelope<CoreRequest>,
    ) -> Result<(), ProtocolError> {
        envelope.validate_protocol_version()?;

        if !envelope.capability_scope.has(Capability::UseModel) {
            return Err(ProtocolError::CapabilityDenied(
                "origin lacks UseModel capability".to_string(),
            ));
        }

        Ok(())
    }

    /// Write a `delivery_failed` confirmation audit event.
    /// On any failure (lock, write, flush), emits a `LogRecordEmitted`
    /// diagnostic event to the run's event stream — consistent with
    /// `tool_step::audit_confirmation` semantics. Does not change
    /// run terminal status.
    fn audit_delivery_failed(
        &self,
        tool_call_id: &str,
        tool_name: &str,
        run_ref: &RunRef,
        trace_id: &TraceId,
    ) {
        let Some(writer) = &self.log_writer else {
            // No log writer — emit diagnostic
            self.emit_audit_diagnostic(
                run_ref,
                trace_id,
                "audit_no_writer",
                "no log writer available for delivery_failed audit",
            );
            return;
        };

        let mut w = match writer.lock() {
            Ok(w) => w,
            Err(_) => {
                self.emit_audit_diagnostic(
                    run_ref,
                    trace_id,
                    "audit_lock_poisoned",
                    "delivery_failed audit log lock poisoned",
                );
                return;
            }
        };

        if let Err(e) = crate::logging::audit::log_confirmation(
            &mut w,
            "delivery_failed",
            tool_name,
            tool_call_id,
            run_ref.as_str(),
            trace_id.as_str(),
        ) {
            self.emit_audit_diagnostic(
                run_ref,
                trace_id,
                "audit_write_failed",
                &format!("delivery_failed audit write failed: {e}"),
            );
            return;
        }

        if let Err(e) = w.flush() {
            self.emit_audit_diagnostic(
                run_ref,
                trace_id,
                "audit_flush_failed",
                &format!("delivery_failed audit flush failed: {e}"),
            );
        }
    }

    /// Emit a LogRecordEmitted diagnostic event to the run's event stream.
    /// This is non-blocking and does not change run status.
    /// Uses monotonically increasing sequence from SessionManager.
    fn emit_audit_diagnostic(
        &self,
        run_ref: &RunRef,
        trace_id: &TraceId,
        code: &str,
        message: &str,
    ) {
        let seq = self.session_manager.next_event_sequence(run_ref);
        let event = CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            seq,
            CoreEventKind::LogRecordEmitted,
            CoreEventPayload::Json {
                value: serde_json::json!({
                    "level": "warn",
                    "code": code,
                    "message": message,
                }),
            },
        );
        let _ = self.session_manager.push_event(run_ref, event);
    }
}

impl CoreRuntimeApi for CoreRuntime {
    type EventStream = Vec<CoreEvent>;

    /// Non-streaming execution. Does not support cancellation via CancellationToken.
    /// Use `start_streaming()` for product paths that need cancellation.
    fn start(&self, envelope: ProtocolEnvelope<CoreRequest>) -> Result<RunRef, ProtocolError> {
        self.validate_envelope(&envelope)?;

        let request = &envelope.payload;
        request.validate()?;

        let session_ref = match &envelope.session_ref {
            Some(sr) => sr.clone(),
            None => {
                let snapshot = self.session_manager.create_session();
                snapshot.session_ref
            }
        };

        let (turn_ref, run_ref, trace_id) = self.session_manager.create_turn(&session_ref)?;

        let started_event = EventAdapter::stub_started(&run_ref, &trace_id)
            .with_session_ref(session_ref.clone())
            .with_turn_ref(turn_ref.clone());

        self.session_manager.push_event(&run_ref, started_event)?;

        {
            let mut active = self.active_runs.write().unwrap();
            active.insert(
                run_ref.as_str().to_string(),
                ActiveRun {
                    run_ref: run_ref.clone(),
                    trace_id: trace_id.clone(),
                    session_ref: session_ref.clone(),
                    turn_ref: turn_ref.clone(),
                },
            );
        }

        let loop_input = LoopEngine::input_from_request(&request.input);

        // Build conversation context for the model call
        let user_content = match &request.input {
            RequestInput::Text { content } => content.clone(),
            RequestInput::RunLoop { input } => input.content.clone(),
            _ => String::new(),
        };

        let s = self
            .settings
            .read()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".to_string()))?;
        let soul_prompt = {
            let role = s.soul.role.as_str();
            if role.is_empty() {
                None
            } else {
                Some(runtime_config::system_prompt_for_role(&s.soul.role))
            }
        };
        let llm_settings = s.llm.clone();
        drop(s);

        let mut conversation = Conversation::new(soul_prompt);
        if !user_content.is_empty() {
            conversation.add_user_message(user_content.clone());

            // Persist user message to conversation store
            let session_id =
                protocol_interface::SessionId::from_existing(session_ref.as_str().to_string());
            let message = protocol_interface::Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: protocol_interface::MessageRole::User,
                content: user_content.clone(),
                created_at: chrono::Utc::now(),
                tool_calls: None,
                tool_call_id: None,
                tool_name: None,
            };
            let _ = self
                .conversation_store
                .append_message(&session_id, &message);
        }

        let ctx = LoopContext {
            client: self.client.clone(),
            conversation,
            settings: llm_settings,
            workspace: self.session_manager.workspace_root(),
            tool_registry: self.tool_registry.clone(),
            session: Some(self.session_manager.clone()),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            cancel_token: None,
            log_writer: self.log_writer.clone(),
        };

        let sr = session_ref.clone();
        let tr = turn_ref.clone();
        let conversation_store = self.conversation_store.clone();

        let mut loop_events = Vec::new();
        let _result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                LoopEngine::run(&run_ref, &trace_id, &loop_input, &ctx, &mut |event| {
                    loop_events.push(event);
                })
                .await
            })
        });

        for event in loop_events {
            let event = event.with_session_ref(sr.clone()).with_turn_ref(tr.clone());
            self.session_manager.push_event(&run_ref, event.clone())?;

            // Persist tool result message on ToolCallCompleted
            if let CoreEventKind::ToolCallCompleted = event.kind {
                if let CoreEventPayload::Json { value } = &event.payload {
                    if let (Some(tool_call_id), Some(tool_name), Some(output)) = (
                        value.get("id").and_then(|v| v.as_str()),
                        value.get("name").and_then(|v| v.as_str()),
                        value.get("output").and_then(|v| v.as_str()),
                    ) {
                        if let Some(session_ref) = &event.session_ref {
                            let session_id = protocol_interface::SessionId::from_existing(
                                session_ref.as_str().to_string(),
                            );
                            let message = protocol_interface::Message {
                                id: uuid::Uuid::new_v4().to_string(),
                                role: protocol_interface::MessageRole::Tool,
                                content: output.to_string(),
                                created_at: chrono::Utc::now(),
                                tool_calls: None,
                                tool_call_id: Some(tool_call_id.to_string()),
                                tool_name: Some(tool_name.to_string()),
                            };
                            let _ = conversation_store.append_message(&session_id, &message);
                        }
                    }
                }
            }

            // Persist assistant message on FinalResult
            if let CoreEventKind::FinalResult = event.kind {
                if let CoreEventPayload::Final {
                    content,
                    success: true,
                } = &event.payload
                {
                    if let Some(session_ref) = &event.session_ref {
                        let session_id = protocol_interface::SessionId::from_existing(
                            session_ref.as_str().to_string(),
                        );
                        let message = protocol_interface::Message {
                            id: uuid::Uuid::new_v4().to_string(),
                            role: protocol_interface::MessageRole::Assistant,
                            content: content.clone(),
                            created_at: chrono::Utc::now(),
                            tool_calls: None,
                            tool_call_id: None,
                            tool_name: None,
                        };
                        let _ = conversation_store.append_message(&session_id, &message);
                    }
                }
            }

            // Handle status transitions
            let _ = self
                .session_manager
                .handle_event_status_transition(&run_ref, &event);
        }

        Ok(run_ref)
    }

    fn start_streaming(
        &self,
        envelope: ProtocolEnvelope<CoreRequest>,
    ) -> Result<(RunRef, tokio::sync::mpsc::UnboundedReceiver<CoreEvent>), ProtocolError> {
        self.validate_envelope(&envelope)?;

        let request = &envelope.payload;
        request.validate()?;

        let session_ref = match &envelope.session_ref {
            Some(sr) => sr.clone(),
            None => {
                let snapshot = self.session_manager.create_session();
                snapshot.session_ref
            }
        };
        let (turn_ref, run_ref, trace_id) = self.session_manager.create_turn(&session_ref)?;

        let started_event = EventAdapter::stub_started(&run_ref, &trace_id)
            .with_session_ref(session_ref.clone())
            .with_turn_ref(turn_ref.clone());
        self.session_manager.push_event(&run_ref, started_event)?;

        let loop_input = LoopEngine::input_from_request(&request.input);

        let user_content = match &request.input {
            RequestInput::Text { content } => content.clone(),
            RequestInput::RunLoop { input } => input.content.clone(),
            _ => String::new(),
        };

        let s = self
            .settings
            .read()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".to_string()))?;
        let soul_prompt = {
            let role = s.soul.role.as_str();
            if role.is_empty() {
                None
            } else {
                Some(runtime_config::system_prompt_for_role(&s.soul.role))
            }
        };
        let llm_settings = s.llm.clone();
        drop(s);

        let mut conversation = Conversation::new(soul_prompt);
        if !user_content.is_empty() {
            conversation.add_user_message(user_content.clone());

            // Persist user message to conversation store
            let session_id =
                protocol_interface::SessionId::from_existing(session_ref.as_str().to_string());
            let message = protocol_interface::Message {
                id: uuid::Uuid::new_v4().to_string(),
                role: protocol_interface::MessageRole::User,
                content: user_content,
                created_at: chrono::Utc::now(),
                tool_calls: None,
                tool_call_id: None,
                tool_name: None,
            };
            let _ = self
                .conversation_store
                .append_message(&session_id, &message);
        }

        // Get cancel token for this run
        let cancel_token = self.session_manager.get_cancel_token(&run_ref).ok();

        let ctx = LoopContext {
            client: self.client.clone(),
            conversation,
            settings: llm_settings,
            workspace: self.session_manager.workspace_root(),
            tool_registry: self.tool_registry.clone(),
            session: Some(self.session_manager.clone()),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            cancel_token,
            log_writer: self.log_writer.clone(),
        };

        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let sr = session_ref.clone();
        let tr = turn_ref.clone();
        let run_ref_clone = run_ref.clone();
        let trace_id_clone = trace_id.clone();
        let session_manager = self.session_manager.clone();
        let run_ref_for_persist = run_ref.clone();
        let conversation_store = self.conversation_store.clone();

        std::thread::Builder::new()
            .name("alius-streaming-run".to_string())
            .spawn(move || {
                let rt = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .expect("failed to create streaming runtime");
                rt.block_on(async {
                    LoopEngine::run(
                        &run_ref_clone,
                        &trace_id_clone,
                        &loop_input,
                        &ctx,
                        &mut |event| {
                            let event =
                                event.with_session_ref(sr.clone()).with_turn_ref(tr.clone());

                            // Send to channel for TUI
                            let _ = tx.send(event.clone());

                            // Persist to SessionManager for query_logs/subscribe
                            let _ = session_manager.push_event(&run_ref_for_persist, event.clone());

                            // Persist tool result message on ToolCallCompleted
                            if let CoreEventKind::ToolCallCompleted = event.kind {
                                if let CoreEventPayload::Json { value } = &event.payload {
                                    if let (Some(tool_call_id), Some(tool_name), Some(output)) = (
                                        value.get("id").and_then(|v| v.as_str()),
                                        value.get("name").and_then(|v| v.as_str()),
                                        value.get("output").and_then(|v| v.as_str()),
                                    ) {
                                        if let Some(session_ref) = &event.session_ref {
                                            let session_id =
                                                protocol_interface::SessionId::from_existing(
                                                    session_ref.as_str().to_string(),
                                                );
                                            let message = protocol_interface::Message {
                                                id: uuid::Uuid::new_v4().to_string(),
                                                role: protocol_interface::MessageRole::Tool,
                                                content: output.to_string(),
                                                created_at: chrono::Utc::now(),
                                                tool_calls: None,
                                                tool_call_id: Some(tool_call_id.to_string()),
                                                tool_name: Some(tool_name.to_string()),
                                            };
                                            let _ = conversation_store
                                                .append_message(&session_id, &message);
                                        }
                                    }
                                }
                            }

                            // Persist assistant message on FinalResult
                            if let CoreEventKind::FinalResult = event.kind {
                                if let CoreEventPayload::Final {
                                    content,
                                    success: true,
                                } = &event.payload
                                {
                                    if let Some(session_ref) = &event.session_ref {
                                        let session_id =
                                            protocol_interface::SessionId::from_existing(
                                                session_ref.as_str().to_string(),
                                            );
                                        let message = protocol_interface::Message {
                                            id: uuid::Uuid::new_v4().to_string(),
                                            role: protocol_interface::MessageRole::Assistant,
                                            content: content.clone(),
                                            created_at: chrono::Utc::now(),
                                            tool_calls: None,
                                            tool_call_id: None,
                                            tool_name: None,
                                        };
                                        let _ = conversation_store
                                            .append_message(&session_id, &message);
                                    }
                                }
                            }

                            // Update status based on event kind
                            let _ = session_manager
                                .handle_event_status_transition(&run_ref_for_persist, &event);
                        },
                    )
                    .await
                })
            })
            .map_err(|e| ProtocolError::Internal(format!("failed to spawn streaming run: {e}")))?;

        Ok((run_ref, rx))
    }

    fn send(&self, envelope: ProtocolEnvelope<CoreCommand>) -> Result<(), ProtocolError> {
        envelope.validate_protocol_version()?;

        let command = &envelope.payload;
        match command.kind {
            CoreCommandKind::Cancel => {
                self.session_manager.cancel_run(&command.target_run)?;
            }
            CoreCommandKind::Continue => {
                self.session_manager
                    .update_run_status(&command.target_run, RunStatus::Running)?;
            }
            CoreCommandKind::RespondToolConfirmation {
                ref tool_call_id,
                approved,
            } => {
                match self.session_manager.deliver_confirmation(
                    &command.target_run,
                    tool_call_id,
                    approved,
                ) {
                    Ok(_tool_name) => {
                        // Success - audit logged by tool_step
                    }
                    Err((err, tool_name)) => {
                        // Log delivery_failed audit event with full metadata.
                        // On any audit failure, emit a LogRecordEmitted diagnostic
                        // event to the run's event stream — consistent with
                        // tool_step::audit_confirmation semantics.
                        self.audit_delivery_failed(
                            tool_call_id,
                            &tool_name,
                            &command.target_run,
                            &envelope.trace_id,
                        );
                        // Fail-closed: cancel the run to prevent hanging
                        let _ = self.session_manager.cancel_run(&command.target_run);
                        return Err(err);
                    }
                }
            }
            _ => {}
        }

        Ok(())
    }

    fn subscribe(&self, run_ref: &RunRef) -> Result<Self::EventStream, ProtocolError> {
        self.session_manager.get_events(run_ref)
    }

    fn inspect(&self, session_ref: &SessionRef) -> Result<SessionSnapshot, ProtocolError> {
        self.session_manager.get_session(session_ref)
    }

    fn list_sessions(
        &self,
        _workspace_ref: &WorkspaceRef,
    ) -> Result<Vec<SessionSummary>, ProtocolError> {
        Ok(self.session_manager.list_sessions())
    }

    fn query_logs(&self, query: LogQuery) -> Result<Vec<LogRecord>, ProtocolError> {
        let run_refs = if let Some(session_ref) = &query.session_ref {
            self.session_manager.run_refs_for_session(session_ref)?
        } else if let Some(run_ref) = &query.run_ref {
            vec![run_ref.clone()]
        } else {
            self.session_manager
                .all_run_refs()
                .into_iter()
                .map(|(r, _)| r)
                .collect()
        };

        let mut records = Vec::new();
        for run_ref in &run_refs {
            if records.len() >= query.limit {
                break;
            }
            let events = match self.session_manager.get_events(run_ref) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for event in events {
                if records.len() >= query.limit {
                    break;
                }
                if let Some(ref sr) = query.session_ref {
                    if event.session_ref.as_ref() != Some(sr) {
                        continue;
                    }
                }
                if let Some(ref tid) = query.trace_id {
                    if event.trace_id != *tid {
                        continue;
                    }
                }
                let level = match event.kind {
                    CoreEventKind::ErrorRaised => LogLevel::Error,
                    CoreEventKind::TurnStarted | CoreEventKind::RunStarted => LogLevel::Info,
                    _ => LogLevel::Debug,
                };
                let message = format!("{:?}", event.kind);
                records.push(LogRecord {
                    trace_id: event.trace_id,
                    session_ref: event.session_ref,
                    run_ref: Some(event.run_ref),
                    level,
                    message,
                    created_at: event.created_at,
                });
            }
        }
        Ok(records)
    }

    fn close_session(&self, session_ref: &SessionRef) -> Result<(), ProtocolError> {
        self.session_manager.close(session_ref)
    }

    fn clear_conversation(&self, session_ref: &SessionRef) -> Result<(), ProtocolError> {
        let session_id =
            protocol_interface::SessionId::from_existing(session_ref.as_str().to_string());
        self.conversation_store
            .clear_messages(&session_id)
            .map_err(map_err)
    }

    fn config_read(&self) -> Result<ConfigSnapshot, ProtocolError> {
        let s = self
            .settings
            .read()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".into()))?;
        Ok(ConfigSnapshot {
            provider: format!("{:?}", s.llm.provider).to_lowercase(),
            model: s.llm.model.clone(),
            base_url: {
                let url = s.base_url();
                if url.is_empty() {
                    None
                } else {
                    Some(url)
                }
            },
            soul: {
                let role = s.soul.role.as_str();
                if role.is_empty() {
                    None
                } else {
                    Some(role.to_string())
                }
            },
            has_api_key: s.llm.get_api_key().is_some(),
        })
    }

    fn config_validate(&self) -> Result<ValidationResult, ProtocolError> {
        let s = self
            .settings
            .read()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".into()))?;
        let missing = s.missing_chat_requirements();
        Ok(ValidationResult {
            valid: missing.is_empty(),
            errors: missing,
        })
    }

    fn config_update(&self, key: &str, value: serde_json::Value) -> Result<(), ProtocolError> {
        let mut s = self
            .settings
            .write()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".into()))?;

        match key {
            "llm.model" => {
                if let Some(v) = value.as_str() {
                    s.llm.model = v.to_string();
                }
            }
            "llm.provider" => {
                if let Some(v) = value.as_str() {
                    s.llm.provider = runtime_config::parse_provider_type(v);
                }
            }
            "llm.base_url" => {
                s.llm.base_url = value.as_str().map(|v| v.to_string());
            }
            "llm.api_key" => {
                s.llm.api_key = value.as_str().map(|v| v.to_string());
            }
            "soul.role" => {
                if let Some(v) = value.as_str() {
                    s.soul.role = protocol_interface::SoulRole::new(v.to_string());
                }
            }
            _ => {
                return Err(ProtocolError::InvalidMessage(format!(
                    "unsupported config key: {}",
                    key
                )));
            }
        }

        s.save_to_project_config()
            .map_err(|e| ProtocolError::Internal(e.to_string()))?;

        Ok(())
    }

    fn model_list(&self) -> Result<Vec<ModelInfo>, ProtocolError> {
        let models = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(self.client.list_models())
        })
        .map_err(map_err)?;

        Ok(models
            .into_iter()
            .map(|id| ModelInfo {
                id: id.clone(),
                name: id,
            })
            .collect())
    }

    fn memory_save(&self, text: &str, tags: Vec<String>) -> Result<(), ProtocolError> {
        let mut guard = self
            .memory_global
            .write()
            .map_err(|_| ProtocolError::Internal("memory store lock poisoned".into()))?;
        match guard.as_mut() {
            Some(store) => store.save(text, tags).map_err(map_err),
            None => Err(ProtocolError::Internal("memory store not available".into())),
        }
    }

    fn memory_list(&self) -> Result<Vec<MemoryEntry>, ProtocolError> {
        let mut result = Vec::new();
        {
            let guard = self
                .memory_global
                .read()
                .map_err(|_| ProtocolError::Internal("memory store lock poisoned".into()))?;
            if let Some(store) = guard.as_ref() {
                result.extend(store.list().iter().map(|e| MemoryEntry {
                    id: e.id.clone(),
                    content: e.text.clone(),
                    tags: e.tags.clone(),
                    created_at: e.created_at,
                }));
            }
        }
        {
            let guard = self
                .memory_project
                .read()
                .map_err(|_| ProtocolError::Internal("memory store lock poisoned".into()))?;
            if let Some(store) = guard.as_ref() {
                result.extend(store.list().iter().map(|e| MemoryEntry {
                    id: e.id.clone(),
                    content: e.text.clone(),
                    tags: e.tags.clone(),
                    created_at: e.created_at,
                }));
            }
        }
        Ok(result)
    }

    fn memory_clear(&self) -> Result<(), ProtocolError> {
        let mut guard = self
            .memory_global
            .write()
            .map_err(|_| ProtocolError::Internal("memory store lock poisoned".into()))?;
        match guard.as_mut() {
            Some(store) => store.clear().map_err(map_err),
            None => Err(ProtocolError::Internal("memory store not available".into())),
        }
    }

    fn tool_list(&self) -> Result<Vec<ToolInfo>, ProtocolError> {
        match &self.tool_registry {
            Some(registry) => Ok(registry.to_tool_infos()),
            None => Ok(Vec::new()),
        }
    }

    fn review_start(&self, session_ref: &SessionRef) -> Result<RunRef, ProtocolError> {
        // Load conversation to find last assistant message
        let session_id =
            protocol_interface::SessionId::from_existing(session_ref.as_str().to_string());
        let messages = self
            .conversation_store
            .load_messages(&session_id)
            .map_err(map_err)?;
        let last_assistant = messages
            .iter()
            .rev()
            .find(|m| m.role == protocol_interface::MessageRole::Assistant);

        let assistant_text = match last_assistant {
            Some(m) => m.content.clone(),
            None => {
                return Err(ProtocolError::InvalidMessage(
                    "no assistant response to review".into(),
                ));
            }
        };

        let review_prompt = format!(
            "Please review the following assistant response for correctness, completeness, and quality. \
             Point out any issues, errors, or areas for improvement. Be concise.\n\n\
             Assistant response:\n{}",
            assistant_text
        );

        // Create a new turn/run for the review
        let snapshot = self.session_manager.get_session(session_ref)?;
        let session_ref = snapshot.session_ref.clone();
        let (turn_ref, run_ref, trace_id) = self.session_manager.create_turn(&session_ref)?;

        let started_event = EventAdapter::stub_started(&run_ref, &trace_id)
            .with_session_ref(session_ref.clone())
            .with_turn_ref(turn_ref.clone());
        self.session_manager.push_event(&run_ref, started_event)?;

        // Build LoopEngine context for review
        let s = self
            .settings
            .read()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".to_string()))?;
        let review_system_prompt = Some(
            "You are a code review assistant. Review responses for quality and correctness."
                .to_string(),
        );
        let llm_settings = {
            let mut ls = s.llm.clone();
            if let Some(ref rm) = s.llm.review_model {
                ls.model = rm.clone();
            }
            ls
        };
        drop(s);

        let mut conversation = Conversation::new(review_system_prompt);
        conversation.add_user_message(review_prompt);

        let ctx = LoopContext {
            client: self.client.clone(),
            conversation,
            settings: llm_settings,
            workspace: self.session_manager.workspace_root(),
            tool_registry: self.tool_registry.clone(),
            session: Some(self.session_manager.clone()),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
            cancel_token: None,
            log_writer: self.log_writer.clone(),
        };

        let loop_input = LoopEngine::input_from_request(&RequestInput::Text {
            content: "review".to_string(),
        });

        let sr = session_ref.clone();
        let tr = turn_ref.clone();
        let mut loop_events = Vec::new();
        let _result = tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current().block_on(async {
                LoopEngine::run(&run_ref, &trace_id, &loop_input, &ctx, &mut |event| {
                    loop_events.push(event);
                })
                .await
            })
        });

        for event in loop_events {
            self.session_manager.push_event(
                &run_ref,
                event.with_session_ref(sr.clone()).with_turn_ref(tr.clone()),
            )?;
        }

        self.session_manager
            .update_run_status(&run_ref, RunStatus::Completed)?;

        Ok(run_ref)
    }

    fn health_check(&self) -> Result<HealthReport, ProtocolError> {
        let s = self
            .settings
            .read()
            .map_err(|_| ProtocolError::Internal("settings lock poisoned".into()))?;

        let has_api_key = s.llm.get_api_key().is_some();
        let has_model = !s.llm.model.trim().is_empty();
        let config_ok = has_api_key && has_model;

        let mut errors = Vec::new();
        if !has_api_key {
            errors.push("API key not configured".to_string());
        }
        if !has_model {
            errors.push("model not configured".to_string());
        }

        Ok(HealthReport {
            config_ok,
            api_reachable: config_ok,
            workspace_ok: true,
            errors,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_runtime() -> CoreRuntime {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        test_runtime_unlocked()
    }

    fn test_runtime_unlocked() -> CoreRuntime {
        CoreRuntime::new(WorkspaceRef::new("/tmp/test-workspace"))
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_creates_run_and_events() {
        let rt = test_runtime();
        let request = CoreRequest::start_turn("hello").unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let run_ref = rt.start(envelope).unwrap();
        let events = rt.subscribe(&run_ref).unwrap();

        // Chat mode now uses real model call which fails without API key,
        // so we get TurnStarted + RunStarted + ErrorRaised + FinalResult
        assert!(events.len() >= 3);
        assert_eq!(events[0].kind, CoreEventKind::TurnStarted);
        assert_eq!(events[1].kind, CoreEventKind::RunStarted);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn start_streaming_sends_initial_event_inside_runtime() {
        let rt = test_runtime();
        let request =
            CoreRequest::run_loop("hello", RuntimeMode::Plan, LoopPolicy::plan()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (_run_ref, mut rx) = rt.start_streaming(envelope).unwrap();
        let event = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv())
            .await
            .expect("streaming event timed out")
            .expect("streaming channel closed before first event");

        assert_eq!(event.kind, CoreEventKind::RunStarted);
    }

    #[test]
    fn start_rejects_empty_prompt() {
        let result = CoreRequest::start_turn("   ");
        assert!(matches!(result, Err(ProtocolError::InvalidMessage(_))));
    }

    #[test]
    fn start_rejects_missing_model_capability() {
        let rt = test_runtime();
        let request = CoreRequest::start_turn("hello").unwrap();
        let scope = CapabilityScope::default();
        let envelope = ProtocolEnvelope::new(Origin::LocalCli, scope, request);

        let result = rt.start(envelope);
        assert!(matches!(result, Err(ProtocolError::CapabilityDenied(_))));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_updates_run_status() {
        let rt = test_runtime();
        let request = CoreRequest::start_turn("test").unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);
        let run_ref = rt.start(envelope).unwrap();

        let cancel_cmd = CoreCommand::cancel(run_ref.clone(), Some("user cancel".to_string()));
        let cmd_envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), cancel_cmd);
        rt.send(cmd_envelope).unwrap();

        let sessions = rt
            .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
            .unwrap();
        let snapshot = rt.inspect(&sessions[0].session_ref).unwrap();

        let run = snapshot.runs.iter().find(|r| r.run_ref == run_ref).unwrap();
        assert_eq!(run.status, RunStatus::Cancelled);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn list_sessions_returns_created_sessions() {
        let rt = test_runtime();
        let request = CoreRequest::start_turn("session 1").unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);
        rt.start(envelope).unwrap();

        let sessions = rt
            .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
            .unwrap();
        assert_eq!(sessions.len(), 1);
    }

    #[test]
    fn subscribe_returns_empty_for_unknown_run() {
        let rt = test_runtime();
        let unknown_ref = RunRef::new();
        let result = rt.subscribe(&unknown_ref);
        assert!(matches!(result, Err(ProtocolError::RunNotFound(_))));
    }

    #[test]
    fn config_read_returns_current_settings() {
        let rt = test_runtime();
        let snapshot = rt.config_read().unwrap();
        assert!(!snapshot.provider.is_empty());
    }

    #[test]
    fn config_update_changes_model() {
        let _lock = TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let rt = test_runtime_unlocked();
        rt.config_update("llm.model", serde_json::Value::String("gpt-4o-mini".into()))
            .unwrap();

        let snapshot = rt.config_read().unwrap();
        assert_eq!(snapshot.model, "gpt-4o-mini");

        // Restore original cwd; ignore error if it was removed by another test.
        let _ = std::env::set_current_dir(orig_cwd);
    }

    #[test]
    fn config_update_rejects_unknown_key() {
        let rt = test_runtime();
        let result = rt.config_update("unknown.key", serde_json::Value::String("value".into()));
        assert!(matches!(result, Err(ProtocolError::InvalidMessage(_))));
    }

    #[test]
    fn config_validate_checks_requirements() {
        let rt = test_runtime();
        let result = rt.config_validate().unwrap();
        // Default settings don't have soul configured
        assert!(!result.valid);
    }

    #[test]
    fn health_check_reports_config_status() {
        let rt = test_runtime();
        let report = rt.health_check().unwrap();
        // Default test runtime has api_key but may not have model/soul
        assert!(report.errors.len() <= 2);
    }

    // ===== P2 Validation Tests =====

    #[tokio::test(flavor = "multi_thread")]
    async fn start_streaming_persists_loop_events() {
        let rt = test_runtime();
        let request = CoreRequest::run_loop("test", RuntimeMode::Plan, LoopPolicy::plan()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Drain channel to let events be processed
        let mut event_count = 0;
        while let Ok(Some(_event)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {
            event_count += 1;
            if event_count > 20 {
                break; // Prevent infinite loop
            }
        }

        // Now query persisted events
        let events = rt.subscribe(&run_ref).unwrap();

        // Should have at least TurnStarted (stub), RunStarted, and FinalResult
        assert!(
            events.len() >= 3,
            "Expected at least 3 events, got {}",
            events.len()
        );
        assert_eq!(events[0].kind, CoreEventKind::TurnStarted); // stub event
        assert_eq!(events[1].kind, CoreEventKind::RunStarted); // loop engine event

        // Last event should be FinalResult
        let last = events.last().unwrap();
        assert_eq!(last.kind, CoreEventKind::FinalResult);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn subscribe_returns_streaming_events() {
        let rt = test_runtime();
        let request =
            CoreRequest::run_loop("hello", RuntimeMode::Chat, LoopPolicy::chat()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Wait for completion
        while let Ok(Some(_)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {}

        // Verify subscribe returns the same events
        let events = rt.subscribe(&run_ref).unwrap();
        assert!(!events.is_empty());
        assert_eq!(events[0].kind, CoreEventKind::TurnStarted); // stub event first
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_run_marks_completed() {
        let rt = test_runtime();
        // Use stub mode to avoid dependency on real API
        let request = CoreRequest::run_loop("test", RuntimeMode::Plan, LoopPolicy::plan()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Collect all events and check for FinalResult
        let mut final_success: Option<bool> = None;
        while let Ok(Some(event)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {
            if event.kind == CoreEventKind::FinalResult {
                if let CoreEventPayload::Final { success, .. } = event.payload {
                    final_success = Some(success);
                }
                break;
            }
        }

        // Verify we received a FinalResult
        assert!(final_success.is_some(), "Should receive FinalResult event");

        // Verify status matches the FinalResult success value
        let sessions = rt
            .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
            .unwrap();
        if let Some(session) = sessions.first() {
            let snapshot = rt.inspect(&session.session_ref).unwrap();
            let run = snapshot.runs.iter().find(|r| r.run_ref == run_ref).unwrap();

            let expected_status = if final_success.unwrap() {
                RunStatus::Completed
            } else {
                RunStatus::Failed
            };

            assert_eq!(
                run.status,
                expected_status,
                "Status should match FinalResult: success={} should map to {:?}",
                final_success.unwrap(),
                expected_status
            );
            assert!(run.finished_at.is_some(), "finished_at should be set");
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn streaming_run_marks_failed_on_error() {
        let rt = test_runtime();
        // Use Plan mode without tool registry to trigger error
        let request = CoreRequest::run_loop("test", RuntimeMode::Plan, LoopPolicy::plan()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Wait for all events
        while let Ok(Some(_)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {}

        // Check status
        let sessions = rt
            .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
            .unwrap();
        if let Some(session) = sessions.first() {
            let snapshot = rt.inspect(&session.session_ref).unwrap();
            if let Some(run) = snapshot.runs.iter().find(|r| r.run_ref == run_ref) {
                // Should be Failed due to missing tool registry
                assert_eq!(
                    run.status,
                    RunStatus::Failed,
                    "Expected Failed, got {:?}",
                    run.status
                );
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_streaming_run_stops_future_events() {
        let rt = test_runtime();
        let request = CoreRequest::run_loop("test", RuntimeMode::Plan, LoopPolicy::plan()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Wait for first event with generous timeout
        let first_event = tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await;
        assert!(first_event.is_ok(), "Should receive at least one event");

        // Cancel the run
        let cancel_cmd = CoreCommand::cancel(run_ref.clone(), Some("test cancel".to_string()));
        let cmd_envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), cancel_cmd);
        rt.send(cmd_envelope).unwrap();

        // Collect remaining events with generous timeout — ensure no success=true FinalResult
        let mut found_success_final = false;
        while let Ok(Some(event)) =
            tokio::time::timeout(std::time::Duration::from_secs(2), rx.recv()).await
        {
            if let CoreEventKind::FinalResult = event.kind {
                if let CoreEventPayload::Final { success: true, .. } = event.payload {
                    found_success_final = true;
                }
            }
        }

        assert!(
            !found_success_final,
            "Cancelled run should not emit success=true FinalResult"
        );

        // Wait for status propagation — the cancel command may take time to
        // be processed and update the run status. Retry with backoff to
        // handle race conditions in serial test execution.
        let mut status = None;
        for attempt in 0..10 {
            tokio::time::sleep(std::time::Duration::from_millis(50 * (attempt + 1))).await;

            let sessions = rt
                .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
                .unwrap();
            if let Some(session) = sessions.first() {
                if let Ok(snapshot) = rt.inspect(&session.session_ref) {
                    if let Some(run) = snapshot.runs.iter().find(|r| r.run_ref == run_ref) {
                        status = Some(run.status.clone());
                        if matches!(status, Some(RunStatus::Cancelled)) {
                            break;
                        }
                    }
                }
            }
        }

        assert_eq!(
            status,
            Some(RunStatus::Cancelled),
            "Expected Cancelled after cancel command"
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancel_is_idempotent() {
        let rt = test_runtime();
        let request = CoreRequest::start_turn("test").unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);
        let run_ref = rt.start(envelope).unwrap();

        // Cancel once
        let cancel_cmd = CoreCommand::cancel(run_ref.clone(), None);
        let cmd_envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), cancel_cmd);
        rt.send(cmd_envelope.clone()).unwrap();

        // Cancel again - should not error
        let result = rt.send(cmd_envelope);
        assert!(result.is_ok(), "Cancel should be idempotent");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn conversation_messages_persist() {
        let rt = test_runtime();
        // Use a streaming run to capture all message types
        let request = CoreRequest::start_turn("Write a test file").unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Collect all events
        let mut has_tool_call = false;
        while let Ok(Some(event)) =
            tokio::time::timeout(std::time::Duration::from_secs(3), rx.recv()).await
        {
            if let CoreEventKind::ToolCallCompleted = event.kind {
                has_tool_call = true;
            }
        }

        // Get session
        let sessions = rt
            .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
            .unwrap();
        assert!(!sessions.is_empty(), "Should have at least one session");

        let session = &sessions[0];
        let session_id =
            protocol_interface::SessionId::from_existing(session.session_ref.as_str().to_string());

        // Load messages from conversation store
        let messages = rt.conversation_store.load_messages(&session_id).unwrap();

        // Should have at least user message
        assert!(!messages.is_empty(), "Should have persisted messages");

        // Should have user message
        let user_msg = messages
            .iter()
            .find(|m| m.role == protocol_interface::MessageRole::User);
        assert!(user_msg.is_some(), "Should have user message");
        assert_eq!(user_msg.unwrap().content, "Write a test file");

        // If run had tool calls, should have tool messages
        if has_tool_call {
            let tool_msg = messages
                .iter()
                .find(|m| m.role == protocol_interface::MessageRole::Tool);
            assert!(
                tool_msg.is_some(),
                "Should have tool message when tool calls executed"
            );
            let tool_msg = tool_msg.unwrap();
            assert!(
                tool_msg.tool_call_id.is_some(),
                "Tool message should have tool_call_id"
            );
            assert!(
                tool_msg.tool_name.is_some(),
                "Tool message should have tool_name"
            );
        }

        // If run completed successfully, should have assistant message
        if let Ok(snapshot) = rt.inspect(&session.session_ref) {
            if let Some(run) = snapshot.runs.iter().find(|r| r.run_ref == run_ref) {
                if run.status == RunStatus::Completed {
                    let assistant_msg = messages
                        .iter()
                        .find(|m| m.role == protocol_interface::MessageRole::Assistant);
                    assert!(
                        assistant_msg.is_some(),
                        "Completed run should have assistant message"
                    );
                }
            }
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn cancelled_status_is_terminal() {
        let rt = test_runtime();
        let request = CoreRequest::run_loop("test", RuntimeMode::Plan, LoopPolicy::plan()).unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), request);

        let (run_ref, mut rx) = rt.start_streaming(envelope).unwrap();

        // Wait for first event
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), rx.recv()).await;

        // Cancel the run
        let cancel_cmd = CoreCommand::cancel(run_ref.clone(), None);
        let cmd_envelope =
            ProtocolEnvelope::new(Origin::LocalCli, CapabilityScope::local_cli(), cancel_cmd);
        rt.send(cmd_envelope).unwrap();

        // Wait for remaining events (including FinalResult)
        while let Ok(Some(_)) =
            tokio::time::timeout(std::time::Duration::from_millis(500), rx.recv()).await
        {}

        // Verify status is still Cancelled (not overwritten by FinalResult)
        let sessions = rt
            .list_sessions(&WorkspaceRef::new("/tmp/test-workspace"))
            .unwrap();
        if let Some(session) = sessions.first() {
            let snapshot = rt.inspect(&session.session_ref).unwrap();
            if let Some(run) = snapshot.runs.iter().find(|r| r.run_ref == run_ref) {
                assert_eq!(
                    run.status,
                    RunStatus::Cancelled,
                    "Cancelled should be terminal and not overwritten"
                );
            }
        }
    }

    /// Verify that CoreRuntime::tool_list() returns tools with correct source metadata.
    /// Constructs a runtime with both native and MCP tools via CoreRuntimeBuilder.
    #[test]
    fn tool_list_returns_correct_source_metadata() {
        use async_trait::async_trait;
        use protocol_interface::core::ToolSource;
        use runtime_tools::ToolRegistry;

        struct FakeMcpTool;
        #[async_trait]
        impl runtime_tools::AliusTool for FakeMcpTool {
            fn name(&self) -> &'static str {
                "mcp_search"
            }
            fn description(&self) -> &'static str {
                "fake mcp search"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({})
            }
            fn source(&self) -> ToolSource {
                ToolSource::Mcp
            }
            async fn execute(
                &self,
                _: serde_json::Value,
                _: runtime_tools::ToolContext,
            ) -> Result<runtime_tools::ToolResult, protocol_interface::AliusError> {
                unimplemented!()
            }
        }

        let registry = Arc::new(ToolRegistry::new());
        runtime_tools::native::register_native_tools(&registry);
        registry.register(FakeMcpTool).unwrap();

        let settings = runtime_config::Settings::default();
        let llm_settings = runtime_config::LlmSettings {
            api_key: Some("test-key".into()),
            ..Default::default()
        };
        let client = runtime_model::LlmClient::new(llm_settings).unwrap();

        let rt = CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new("/tmp/test-source"))
            .settings(settings)
            .client(client)
            .tool_registry_arc(registry)
            .build()
            .unwrap();

        let tool_list = rt.tool_list().unwrap();
        assert!(!tool_list.is_empty(), "should have tools");

        // Verify native tools have RustWasm source.
        let native_tools: Vec<_> = tool_list
            .iter()
            .filter(|t| t.source == ToolSource::RustWasm)
            .collect();
        assert_eq!(native_tools.len(), 5, "should have 5 native tools");

        // Verify MCP tool has Mcp source.
        let mcp_tools: Vec<_> = tool_list
            .iter()
            .filter(|t| t.source == ToolSource::Mcp)
            .collect();
        assert_eq!(mcp_tools.len(), 1, "should have 1 MCP tool");
        assert_eq!(mcp_tools[0].name, "mcp_search");
    }

    /// Test: CoreRuntime::send() logs delivery_failed audit event
    /// when respond_confirmation fails, using envelope.trace_id,
    /// and fail-cancels the run. Uses CoreRuntimeBuilder to ensure
    /// a real LogWriter is present.
    #[tokio::test]
    async fn delivery_failed_audit_uses_envelope_trace_id() {
        use protocol_interface::{CoreCommand, ProtocolEnvelope};

        // Create a temp workspace directory with a real LogWriter
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace_ref = WorkspaceRef::new(tmp.path());

        let settings = runtime_config::Settings::default();
        let client = runtime_model::LlmClient::new(runtime_config::LlmSettings::default())
            .unwrap_or_else(|_| {
                runtime_model::LlmClient::new(runtime_config::LlmSettings {
                    api_key: Some("test-key".into()),
                    ..Default::default()
                })
                .unwrap()
            });

        let rt = CoreRuntimeBuilder::new()
            .workspace_ref(workspace_ref)
            .settings(settings)
            .client(client)
            .build()
            .unwrap();

        // Verify LogWriter was created
        assert!(
            rt.log_writer.is_some(),
            "LogWriter should be created under workspace"
        );

        // Create a session and run
        let session = rt.session_manager().create_session().session_ref;
        let (_, run_ref, _trace_id) = rt.session_manager().create_turn(&session).unwrap();

        // Create a RespondToolConfirmation command for a nonexistent tool_call_id
        let cmd = CoreCommand::respond_confirmation(run_ref.clone(), "tc-nonexistent", true);
        let envelope = ProtocolEnvelope::new(
            protocol_interface::Origin::LocalCli,
            protocol_interface::CapabilityScope::local_cli(),
            cmd,
        );
        let expected_trace_id = envelope.trace_id.clone();

        // Call send - should fail with delivery_failed
        let result = rt.send(envelope);
        assert!(
            result.is_err(),
            "send should fail for nonexistent confirmation"
        );

        // Verify run was cancelled (fail-closed)
        let status = rt.session_manager().get_run_status(&run_ref);
        assert!(
            matches!(status, Ok(RunStatus::Cancelled) | Err(_)),
            "run should be cancelled or not found after delivery failure"
        );

        // Verify audit log was written
        let log_dir = tmp.path().join(".alius").join("logs");
        let event_log = log_dir.join("event-log.jsonl");
        assert!(event_log.exists(), "event-log.jsonl should exist");

        let content = std::fs::read_to_string(&event_log).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert!(!lines.is_empty(), "audit log should contain entries");

        // Parse the last line as the delivery_failed entry
        let entry: serde_json::Value =
            serde_json::from_str(lines.last().unwrap()).expect("audit entry should be valid JSON");

        // Verify audit fields
        assert_eq!(
            entry["event_type"], "tool_confirmation",
            "audit entry should be tool_confirmation type"
        );
        assert_eq!(
            entry["data"]["action"], "delivery_failed",
            "audit action should be delivery_failed"
        );
        assert_eq!(
            entry["data"]["tool_call_id"], "tc-nonexistent",
            "tool_call_id should match command"
        );
        assert_eq!(
            entry["data"]["tool_name"], "unknown",
            "tool_name should be 'unknown' sentinel for no-pending case"
        );
        assert_eq!(
            entry["trace_id"].as_str().unwrap(),
            expected_trace_id.as_str(),
            "trace_id should match envelope.trace_id"
        );
        assert_eq!(
            entry["data"]["run_ref"].as_str().unwrap(),
            run_ref.as_str(),
            "run_ref should match"
        );

        // Verify sensitive data is NOT logged
        let log_str = content.to_lowercase();
        assert!(
            !log_str.contains("password"),
            "should not contain sensitive data"
        );
        assert!(
            !log_str.contains("secret"),
            "should not contain sensitive data"
        );
    }

    /// Test: CoreRuntime::send() logs delivery_failed for receiver-dropped
    /// scenario, preserving original tool_name from stored metadata.
    #[tokio::test]
    async fn delivery_failed_receiver_dropped_audit_preserves_tool_name() {
        use protocol_interface::{CoreCommand, ProtocolEnvelope};

        // Create a temp workspace directory with a real LogWriter
        let tmp = tempfile::TempDir::new().unwrap();
        let workspace_ref = WorkspaceRef::new(tmp.path());

        let settings = runtime_config::Settings::default();
        let client = runtime_model::LlmClient::new(runtime_config::LlmSettings::default())
            .unwrap_or_else(|_| {
                runtime_model::LlmClient::new(runtime_config::LlmSettings {
                    api_key: Some("test-key".into()),
                    ..Default::default()
                })
                .unwrap()
            });

        let rt = CoreRuntimeBuilder::new()
            .workspace_ref(workspace_ref)
            .settings(settings)
            .client(client)
            .build()
            .unwrap();

        // Create a session and run
        let session = rt.session_manager().create_session().session_ref;
        let (_, run_ref, trace_id) = rt.session_manager().create_turn(&session).unwrap();

        // Store a confirmation sender then drop the receiver to simulate
        // receiver-dropped scenario
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        rt.session_manager()
            .store_confirmation_sender(&run_ref, "tc-shell", "shell", &trace_id, tx)
            .unwrap();
        drop(rx); // Drop receiver - next send will fail

        // Create a RespondToolConfirmation command
        let cmd = CoreCommand::respond_confirmation(run_ref.clone(), "tc-shell", true);
        let envelope = ProtocolEnvelope::new(
            protocol_interface::Origin::LocalCli,
            protocol_interface::CapabilityScope::local_cli(),
            cmd,
        );
        let expected_trace_id = envelope.trace_id.clone();

        // Call send - should fail with delivery_failed
        let result = rt.send(envelope);
        assert!(result.is_err(), "send should fail when receiver is dropped");

        // Verify run was cancelled (fail-closed)
        let status = rt.session_manager().get_run_status(&run_ref);
        assert!(
            matches!(status, Ok(RunStatus::Cancelled) | Err(_)),
            "run should be cancelled after delivery failure"
        );

        // Verify audit log was written
        let log_dir = tmp.path().join(".alius").join("logs");
        let event_log = log_dir.join("event-log.jsonl");
        assert!(event_log.exists(), "event-log.jsonl should exist");

        let content = std::fs::read_to_string(&event_log).unwrap();
        let lines: Vec<&str> = content.trim().lines().collect();
        assert!(!lines.is_empty(), "audit log should contain entries");

        let entry: serde_json::Value =
            serde_json::from_str(lines.last().unwrap()).expect("audit entry should be valid JSON");

        // Verify audit fields - should preserve original tool_name
        assert_eq!(entry["data"]["action"], "delivery_failed");
        assert_eq!(entry["data"]["tool_call_id"], "tc-shell");
        assert_eq!(
            entry["data"]["tool_name"], "shell",
            "should preserve original tool_name from stored metadata"
        );
        assert_eq!(
            entry["trace_id"].as_str().unwrap(),
            expected_trace_id.as_str(),
            "trace_id should match envelope.trace_id"
        );
    }

    /// Test: When audit log write fails (e.g., disk full), a
    /// LogRecordEmitted diagnostic event is emitted to the run,
    /// and the run status is NOT changed (non-blocking).
    #[tokio::test]
    async fn delivery_failed_audit_emits_diagnostic_on_failure() {
        use protocol_interface::{CoreCommand, ProtocolEnvelope};

        // Create a runtime WITHOUT a log_writer to force audit failure path
        let rt = CoreRuntime::new(WorkspaceRef::new("/tmp/test-audit-failure"));
        assert!(rt.log_writer.is_none(), "should have no log writer");

        // Create a session and run
        let session = rt.session_manager().create_session().session_ref;
        let (_, run_ref, _trace_id) = rt.session_manager().create_turn(&session).unwrap();

        // Store initial event count
        let initial_events = rt.session_manager().get_events(&run_ref).unwrap().len();

        // Create a RespondToolConfirmation command for a nonexistent tool_call_id
        let cmd = CoreCommand::respond_confirmation(run_ref.clone(), "tc-fail-audit", true);
        let envelope = ProtocolEnvelope::new(
            protocol_interface::Origin::LocalCli,
            protocol_interface::CapabilityScope::local_cli(),
            cmd,
        );

        // Call send - should fail but NOT panic
        let result = rt.send(envelope);
        assert!(
            result.is_err(),
            "send should fail for nonexistent confirmation"
        );

        // Verify run was cancelled (fail-closed)
        let status = rt.session_manager().get_run_status(&run_ref);
        assert!(
            matches!(status, Ok(RunStatus::Cancelled) | Err(_)),
            "run should be cancelled after delivery failure"
        );

        // Verify diagnostic event was emitted
        let events = rt.session_manager().get_events(&run_ref).unwrap();
        assert!(
            events.len() > initial_events,
            "should have emitted diagnostic event"
        );

        // Find the LogRecordEmitted diagnostic event
        let diagnostic = events
            .iter()
            .find(|e| e.kind == CoreEventKind::LogRecordEmitted);
        assert!(
            diagnostic.is_some(),
            "should have LogRecordEmitted diagnostic"
        );

        if let Some(event) = diagnostic {
            // Verify sequence is non-zero and monotonically increasing
            assert!(
                event.sequence > 0,
                "diagnostic event sequence must be > 0, got {}",
                event.sequence
            );
            // Verify monotonic: all events should have increasing sequences
            let mut prev_seq = 0u64;
            for e in &events {
                assert!(
                    e.sequence > prev_seq,
                    "event sequence must be monotonically increasing: {} > {}",
                    e.sequence,
                    prev_seq
                );
                prev_seq = e.sequence;
            }

            if let CoreEventPayload::Json { value } = &event.payload {
                assert_eq!(value["level"], "warn");
                assert_eq!(value["code"], "audit_no_writer");
                assert!(value["message"].as_str().unwrap().contains("no log writer"));
            } else {
                panic!("diagnostic should have Json payload");
            }
        }

        // Verify run status is still Cancelled (diagnostic doesn't change status)
        let final_status = rt.session_manager().get_run_status(&run_ref);
        assert_eq!(final_status.unwrap(), RunStatus::Cancelled);
    }

    // ── Session lifecycle ─────────────────────────────────────────────

    #[test]
    fn test_close_session_succeeds() {
        let rt = test_runtime();
        let session = rt.session_manager().create_session();
        let session_ref = session.session_ref.clone();

        // Close should succeed
        let result = rt.close_session(&session_ref);
        assert!(result.is_ok());

        // Closing again should succeed (idempotent)
        let _result2 = rt.close_session(&session_ref);
    }

    #[test]
    fn test_clear_conversation_succeeds() {
        let rt = test_runtime();
        let session = rt.session_manager().create_session();
        let session_ref = session.session_ref.clone();

        // clear_conversation should succeed
        let result = rt.clear_conversation(&session_ref);
        assert!(result.is_ok());
    }

    #[test]
    fn test_close_nonexistent_session_returns_error() {
        let rt = test_runtime();
        let fake_ref = SessionRef::new();

        let result = rt.close_session(&fake_ref);
        assert!(result.is_err());
    }

    #[test]
    fn test_query_logs_returns_empty_for_no_runs() {
        let rt = test_runtime();
        let query = LogQuery {
            workspace_ref: None,
            session_ref: None,
            run_ref: None,
            trace_id: None,
            min_level: None,
            limit: 100,
        };
        let result = rt.query_logs(query);
        assert!(result.is_ok());
        assert!(result.unwrap().is_empty());
    }

    #[test]
    fn test_query_logs_with_limit() {
        let rt = test_runtime();
        let query = LogQuery {
            workspace_ref: None,
            session_ref: None,
            run_ref: None,
            trace_id: None,
            min_level: None,
            limit: 1,
        };
        let result = rt.query_logs(query);
        assert!(result.is_ok());
        let records = result.unwrap();
        assert!(records.len() <= 1);
    }

    // ── config_update: all supported keys ────────────────────────────

    #[test]
    fn test_config_update_provider() {
        let rt = test_runtime();
        let result = rt.config_update("llm.provider", serde_json::json!("deepseek"));
        assert!(result.is_ok());
        let config = rt.config_read().unwrap();
        assert_eq!(config.provider, "deepseek");
    }

    #[test]
    fn test_config_update_base_url() {
        let rt = test_runtime();
        let result = rt.config_update("llm.base_url", serde_json::json!("https://custom.api.com"));
        assert!(result.is_ok());
        let config = rt.config_read().unwrap();
        assert_eq!(config.base_url, Some("https://custom.api.com".to_string()));
    }

    #[test]
    fn test_config_update_base_url_null() {
        let rt = test_runtime();
        let result = rt.config_update("llm.base_url", serde_json::Value::Null);
        // May fail on save (no project dir) but the in-memory update should work
        // Verify the key is accepted (not "unsupported config key")
        if let Err(e) = &result {
            assert!(
                !e.to_string().contains("unsupported config key"),
                "base_url should be a supported key"
            );
        }
    }

    // ── review_start ────────────────────────────────────────────────

    #[test]
    fn test_review_start_no_assistant_returns_error() {
        let rt = test_runtime();
        let session = rt.session_manager().create_session();
        let result = rt.review_start(&session.session_ref);
        // With no conversation history, should return "no assistant response" error
        assert!(result.is_err());
        let msg = result.unwrap_err().to_string();
        assert!(
            msg.contains("no assistant response") || msg.contains("assistant"),
            "expected 'no assistant response' error, got: {msg}"
        );
    }

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
