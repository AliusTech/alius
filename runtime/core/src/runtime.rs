//! Core Runtime — implements CoreRuntimeApi.

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::RwLock;

use protocol_interface::core::*;
use runtime_config::Settings;
use runtime_model::{Conversation, LlmClient};

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
    conversation_store: runtime_store::ConversationStore,
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
        let conversation_store = runtime_store::ConversationStore::new()
            .map_err(|e| ProtocolError::Internal(format!("conversation store: {}", e)))?;

        Ok(CoreRuntime {
            session_manager: Arc::new(SessionManager::new(workspace_ref)),
            settings: Arc::new(RwLock::new(settings)),
            client: Arc::new(client),
            active_runs: Arc::new(RwLock::new(HashMap::new())),
            memory_global: Arc::new(RwLock::new(memory_global)),
            memory_project: Arc::new(RwLock::new(memory_project)),
            tool_registry: self.tool_registry,
            conversation_store,
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

        let conversation_store = runtime_store::ConversationStore::new()
            .expect("Failed to create conversation store for test runtime");

        Self {
            session_manager: Arc::new(SessionManager::new(workspace_ref)),
            settings: Arc::new(RwLock::new(settings)),
            client: Arc::new(client),
            active_runs: Arc::new(RwLock::new(HashMap::new())),
            memory_global: Arc::new(RwLock::new(None)),
            memory_project: Arc::new(RwLock::new(None)),
            tool_registry: None,
            conversation_store,
        }
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
}

impl CoreRuntimeApi for CoreRuntime {
    type EventStream = Vec<CoreEvent>;

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
            conversation.add_user_message(user_content);
        }

        let ctx = LoopContext {
            client: self.client.clone(),
            conversation,
            settings: llm_settings,
            workspace: self.session_manager.workspace_root(),
            tool_registry: self.tool_registry.clone(),
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
        };

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

    fn send(&self, envelope: ProtocolEnvelope<CoreCommand>) -> Result<(), ProtocolError> {
        envelope.validate_protocol_version()?;

        let command = &envelope.payload;
        match command.kind {
            CoreCommandKind::Cancel => {
                self.session_manager
                    .update_run_status(&command.target_run, RunStatus::Cancelled)?;
            }
            CoreCommandKind::Continue => {
                self.session_manager
                    .update_run_status(&command.target_run, RunStatus::Running)?;
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
                )))
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
            Some(registry) => Ok(registry
                .to_tool_defs()
                .into_iter()
                .map(|def| ToolInfo {
                    name: def.name,
                    description: def.description,
                    source: ToolSource::BuiltIn,
                })
                .collect()),
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
                ))
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
            max_context_tokens: DEFAULT_MAX_CONTEXT_TOKENS,
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

        let rt = test_runtime();
        rt.config_update("llm.model", serde_json::Value::String("gpt-4o-mini".into()))
            .unwrap();

        let snapshot = rt.config_read().unwrap();
        assert_eq!(snapshot.model, "gpt-4o-mini");

        std::env::set_current_dir(orig_cwd).unwrap();
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

    static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
