//! Core Runtime protocol contract types.
//!
//! These types define the stable boundary between product entrypoints,
//! the Protocol Interface Layer, and the Core Runtime.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

use crate::SessionId;

/// Protocol version used by the first Core Runtime contract.
pub const PROTOCOL_VERSION: &str = "1.0";

macro_rules! id_type {
    ($name:ident) => {
        #[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4().to_string())
            }

            pub fn from_existing(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}", self.0)
            }
        }
    };
}

id_type!(RequestId);
id_type!(CommandId);
id_type!(EventId);
id_type!(TraceId);
id_type!(SessionRef);
id_type!(TurnRef);
id_type!(RunRef);

impl From<SessionId> for SessionRef {
    fn from(value: SessionId) -> Self {
        Self::from_existing(value.to_string())
    }
}

impl From<&SessionId> for SessionRef {
    fn from(value: &SessionId) -> Self {
        Self::from_existing(value.to_string())
    }
}

/// Workspace boundary for a single Alius-managed project directory.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceRef {
    pub root: PathBuf,
}

impl WorkspaceRef {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

/// Unified envelope used by all product-to-Core messages.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProtocolEnvelope<T> {
    pub protocol_version: String,
    pub origin: Origin,
    pub capability_scope: CapabilityScope,
    pub workspace_root: Option<PathBuf>,
    pub session_ref: Option<SessionRef>,
    pub run_ref: Option<RunRef>,
    pub trace_id: TraceId,
    pub payload: T,
}

impl<T> ProtocolEnvelope<T> {
    pub fn new(origin: Origin, capability_scope: CapabilityScope, payload: T) -> Self {
        Self {
            protocol_version: PROTOCOL_VERSION.to_string(),
            origin,
            capability_scope,
            workspace_root: None,
            session_ref: None,
            run_ref: None,
            trace_id: TraceId::new(),
            payload,
        }
    }

    pub fn with_workspace_root(mut self, workspace_root: impl Into<PathBuf>) -> Self {
        self.workspace_root = Some(workspace_root.into());
        self
    }

    pub fn with_session_ref(mut self, session_ref: SessionRef) -> Self {
        self.session_ref = Some(session_ref);
        self
    }

    pub fn with_run_ref(mut self, run_ref: RunRef) -> Self {
        self.run_ref = Some(run_ref);
        self
    }

    pub fn with_trace_id(mut self, trace_id: TraceId) -> Self {
        self.trace_id = trace_id;
        self
    }

    pub fn validate_protocol_version(&self) -> Result<(), ProtocolError> {
        if self.protocol_version == PROTOCOL_VERSION {
            Ok(())
        } else {
            Err(ProtocolError::UnsupportedVersion {
                expected: PROTOCOL_VERSION.to_string(),
                actual: self.protocol_version.clone(),
            })
        }
    }
}

/// Product or adapter identity that submitted the message.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Origin {
    LocalCli,
    LocalTui,
    EmbeddedSdk,
    IdeExtension,
    Desktop,
    RemoteA2A,
    PluginRpc,
    JsonRpc,
    Test,
}

/// Capability upper bound supplied by the product or adapter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct CapabilityScope {
    pub capabilities: Vec<Capability>,
    pub allow_external_workspace_paths: bool,
    pub requires_human_approval: bool,
}

impl CapabilityScope {
    pub fn local_cli() -> Self {
        Self {
            capabilities: vec![
                Capability::ReadWorkspace,
                Capability::WriteWorkspace,
                Capability::UseModel,
                Capability::UseTools,
                Capability::UseShell,
                Capability::UseMcp,
                Capability::ReadMemory,
                Capability::WriteMemory,
                Capability::ReadConfig,
                Capability::WriteConfig,
            ],
            allow_external_workspace_paths: false,
            requires_human_approval: true,
        }
    }

    pub fn local_tui() -> Self {
        Self::local_cli()
    }

    pub fn embedded_sdk() -> Self {
        Self {
            capabilities: vec![
                Capability::UseModel,
                Capability::ReadMemory,
                Capability::ReadConfig,
            ],
            allow_external_workspace_paths: false,
            requires_human_approval: true,
        }
    }

    pub fn remote_a2a() -> Self {
        Self {
            capabilities: vec![Capability::UseModel, Capability::ReadMemory],
            allow_external_workspace_paths: false,
            requires_human_approval: true,
        }
    }

    pub fn has(&self, capability: Capability) -> bool {
        self.capabilities.contains(&capability)
    }
}

/// Individual capability flags used before policy evaluation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    ReadWorkspace,
    WriteWorkspace,
    UseModel,
    UseTools,
    UseShell,
    UseMcp,
    ReadMemory,
    WriteMemory,
    ReadConfig,
    WriteConfig,
    RemoteA2A,
}

/// Request submitted to the Core Public API.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoreRequest {
    pub request_id: RequestId,
    pub kind: CoreRequestKind,
    pub input: RequestInput,
    pub metadata: RequestMetadata,
}

impl CoreRequest {
    pub fn run_loop(
        content: impl Into<String>,
        mode: RuntimeMode,
        policy: LoopPolicy,
    ) -> Result<Self, ProtocolError> {
        let content = content.into();
        if content.trim().is_empty() {
            return Err(ProtocolError::InvalidMessage(
                "run loop input cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            request_id: RequestId::new(),
            kind: CoreRequestKind::RunLoop,
            input: RequestInput::RunLoop {
                input: RunLoopInput {
                    content,
                    mode,
                    policy,
                },
            },
            metadata: RequestMetadata::default(),
        })
    }

    pub fn start_turn(text: impl Into<String>) -> Result<Self, ProtocolError> {
        let content = text.into();
        if content.trim().is_empty() {
            return Err(ProtocolError::InvalidMessage(
                "start turn input cannot be empty".to_string(),
            ));
        }

        Ok(Self {
            request_id: RequestId::new(),
            kind: CoreRequestKind::StartTurn,
            input: RequestInput::Text { content },
            metadata: RequestMetadata::default(),
        })
    }

    pub fn open_session(session_name: Option<String>, purpose: SessionPurpose) -> Self {
        Self {
            request_id: RequestId::new(),
            kind: CoreRequestKind::OpenSession,
            input: RequestInput::None,
            metadata: RequestMetadata {
                session_name,
                purpose,
                ..RequestMetadata::default()
            },
        }
    }

    pub fn inspect_session() -> Self {
        Self {
            request_id: RequestId::new(),
            kind: CoreRequestKind::InspectSession,
            input: RequestInput::None,
            metadata: RequestMetadata::default(),
        }
    }

    pub fn list_sessions() -> Self {
        Self {
            request_id: RequestId::new(),
            kind: CoreRequestKind::ListSessions,
            input: RequestInput::None,
            metadata: RequestMetadata::default(),
        }
    }

    pub fn validate(&self) -> Result<(), ProtocolError> {
        match (&self.kind, &self.input) {
            (
                CoreRequestKind::RunLoop,
                RequestInput::RunLoop {
                    input: RunLoopInput { content, .. },
                },
            ) if !content.trim().is_empty() => Ok(()),
            (CoreRequestKind::RunLoop, _) => Err(ProtocolError::InvalidMessage(
                "RunLoop requires non-empty run loop input".to_string(),
            )),
            (CoreRequestKind::StartTurn, RequestInput::Text { content })
                if !content.trim().is_empty() =>
            {
                Ok(())
            }
            (CoreRequestKind::StartTurn, _) => Err(ProtocolError::InvalidMessage(
                "StartTurn requires non-empty text input".to_string(),
            )),
            _ => Ok(()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CoreRequestKind {
    InitProject,
    RunLoop,
    StartTurn,
    OpenSession,
    InspectSession,
    ListSessions,
    ToolQuery,
    CloseSession,
    ClearConversation,
    ConfigRead,
    ConfigValidate,
    ConfigUpdate,
    ModelList,
    MemorySave,
    MemoryList,
    MemoryClear,
    ReviewStart,
    ReviewToggle,
    ConfirmToggle,
    HealthCheck,
    SessionCommand,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RequestInput {
    None,
    Text {
        content: String,
    },
    RunLoop {
        input: RunLoopInput,
    },
    Json {
        value: serde_json::Value,
    },
    ConfigUpdate {
        key: String,
        value: serde_json::Value,
    },
    MemoryContent {
        text: String,
        tags: Vec<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunLoopInput {
    pub content: String,
    pub mode: RuntimeMode,
    pub policy: LoopPolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RuntimeMode {
    Chat,
    Plan,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LoopPolicy {
    pub max_iterations: u32,
    pub tools_enabled: bool,
    pub planning_enabled: bool,
    pub require_convergence_check: bool,
    pub require_approval_for_tools: bool,
}

impl LoopPolicy {
    pub fn chat() -> Self {
        Self {
            max_iterations: 1,
            tools_enabled: true,
            planning_enabled: false,
            require_convergence_check: true,
            require_approval_for_tools: false,
        }
    }

    pub fn plan() -> Self {
        Self {
            max_iterations: 20,
            tools_enabled: true,
            planning_enabled: true,
            require_convergence_check: true,
            require_approval_for_tools: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ConvergenceDecision {
    Continue,
    Completed,
    NeedUserInput,
    NeedApproval,
    Failed,
    MaxIterationsReached,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConvergenceReport {
    pub iteration: u32,
    pub decision: ConvergenceDecision,
    pub reason: String,
    pub remaining_steps: Vec<String>,
    pub next_action: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RequestMetadata {
    pub created_at: DateTime<Utc>,
    pub execution_mode: CoreExecutionMode,
    pub session_name: Option<String>,
    pub purpose: SessionPurpose,
    pub model_override: Option<String>,
    pub labels: Vec<String>,
}

impl Default for RequestMetadata {
    fn default() -> Self {
        Self {
            created_at: Utc::now(),
            execution_mode: CoreExecutionMode::Plan,
            session_name: None,
            purpose: SessionPurpose::General,
            model_override: None,
            labels: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum CoreExecutionMode {
    #[default]
    Plan,
    Bypass,
    Command,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SessionPurpose {
    #[default]
    General,
    FeatureDevelopment,
    BugFix,
    Review,
    LongRunningTask,
    A2ATask,
}

/// Control command sent to a running Core run.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CoreCommand {
    pub command_id: CommandId,
    pub kind: CoreCommandKind,
    pub target_run: RunRef,
    pub metadata: CommandMetadata,
}

impl CoreCommand {
    pub fn cancel(target_run: RunRef, reason: Option<String>) -> Self {
        Self {
            command_id: CommandId::new(),
            kind: CoreCommandKind::Cancel,
            target_run,
            metadata: CommandMetadata {
                reason,
                ..CommandMetadata::default()
            },
        }
    }

    pub fn approve(target_run: RunRef, approval_id: impl Into<String>) -> Self {
        Self {
            command_id: CommandId::new(),
            kind: CoreCommandKind::Approve,
            target_run,
            metadata: CommandMetadata {
                approval_id: Some(approval_id.into()),
                ..CommandMetadata::default()
            },
        }
    }

    pub fn deny(target_run: RunRef, reason: impl Into<String>) -> Self {
        Self {
            command_id: CommandId::new(),
            kind: CoreCommandKind::Deny,
            target_run,
            metadata: CommandMetadata {
                reason: Some(reason.into()),
                ..CommandMetadata::default()
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CoreCommandKind {
    Cancel,
    Approve,
    Deny,
    Continue,
    Pause,
    ApprovePlan,
    RevisePlan,
    ExecuteSelected,
    ApproveReview,
    RequestRevision,
    SwitchModel,
    SwitchMode,
    /// User's yes/no response to a tool-confirmation request (Stage B).
    RespondToolConfirmation {
        tool_call_id: String,
        approved: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CommandMetadata {
    pub created_at: DateTime<Utc>,
    pub reason: Option<String>,
    pub approval_id: Option<String>,
    pub actor: Option<Origin>,
}

impl Default for CommandMetadata {
    fn default() -> Self {
        Self {
            created_at: Utc::now(),
            reason: None,
            approval_id: None,
            actor: None,
        }
    }
}

/// Event emitted by the Core Runtime event stream.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CoreEvent {
    pub event_id: EventId,
    pub trace_id: TraceId,
    pub session_ref: Option<SessionRef>,
    pub turn_ref: Option<TurnRef>,
    pub run_ref: RunRef,
    pub sequence: u64,
    pub kind: CoreEventKind,
    pub payload: CoreEventPayload,
    pub created_at: DateTime<Utc>,
}

impl CoreEvent {
    pub fn new(
        run_ref: RunRef,
        trace_id: TraceId,
        sequence: u64,
        kind: CoreEventKind,
        payload: CoreEventPayload,
    ) -> Self {
        Self {
            event_id: EventId::new(),
            trace_id,
            session_ref: None,
            turn_ref: None,
            run_ref,
            sequence,
            kind,
            payload,
            created_at: Utc::now(),
        }
    }

    pub fn final_result(run_ref: RunRef, trace_id: TraceId, content: impl Into<String>) -> Self {
        Self::new(
            run_ref,
            trace_id,
            0,
            CoreEventKind::FinalResult,
            CoreEventPayload::Final {
                content: content.into(),
                success: true,
            },
        )
    }

    pub fn with_session_ref(mut self, session_ref: SessionRef) -> Self {
        self.session_ref = Some(session_ref);
        self
    }

    pub fn with_turn_ref(mut self, turn_ref: TurnRef) -> Self {
        self.turn_ref = Some(turn_ref);
        self
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum CoreEventKind {
    RunStarted,
    LoopIterationStarted,
    SessionOpened,
    TurnStarted,
    ModelDelta,
    ToolCallRequested,
    ToolCallStarted,
    ToolCallCompleted,
    ConvergenceChecked,
    NeedApproval,
    NeedUserInput,
    PolicyDecision,
    BudgetDecision,
    MemoryRetrieved,
    MemoryWritten,
    LogRecordEmitted,
    ErrorRaised,
    RunCancelled,
    FinalResult,
    SessionClosed,
    ConversationCleared,
    ConfigChanged,
    ModelListResult,
    HealthCheckResult,
    PlanProposed,
    PlanStepStarted,
    PlanStepCompleted,
    PlanCompleted,
    ReviewStarted,
    ReviewDelta,
    ReviewCompleted,
    MemoryListResult,
    MemoryCleared,
    ToolListResult,
    ToolConfirmationRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum CoreEventPayload {
    Empty,
    Text {
        text: String,
    },
    Json {
        value: serde_json::Value,
    },
    Convergence {
        report: ConvergenceReport,
    },
    Error {
        code: String,
        message: String,
    },
    Final {
        content: String,
        success: bool,
    },
    /// A tool operation needs user confirmation before executing (Stage B).
    /// Emitted with CoreEventKind::ToolConfirmationRequired.
    ToolConfirmation {
        tool_call_id: String,
        tool_name: String,
        details: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Started,
    Running,
    WaitingForApproval,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSnapshot {
    pub session_ref: SessionRef,
    pub workspace_ref: WorkspaceRef,
    pub status: SessionStatus,
    pub runs: Vec<RunSummary>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum SessionStatus {
    Open,
    Suspended,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SessionSummary {
    pub session_ref: SessionRef,
    pub workspace_ref: WorkspaceRef,
    pub name: Option<String>,
    pub purpose: SessionPurpose,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RunSummary {
    pub run_ref: RunRef,
    pub trace_id: TraceId,
    pub status: RunStatus,
    pub started_at: DateTime<Utc>,
    pub finished_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogQuery {
    pub workspace_ref: Option<WorkspaceRef>,
    pub session_ref: Option<SessionRef>,
    pub run_ref: Option<RunRef>,
    pub trace_id: Option<TraceId>,
    pub min_level: Option<LogLevel>,
    pub limit: usize,
}

impl Default for LogQuery {
    fn default() -> Self {
        Self {
            workspace_ref: None,
            session_ref: None,
            run_ref: None,
            trace_id: None,
            min_level: None,
            limit: 100,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LogRecord {
    pub trace_id: TraceId,
    pub session_ref: Option<SessionRef>,
    pub run_ref: Option<RunRef>,
    pub level: LogLevel,
    pub message: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}

/// Snapshot of current runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ConfigSnapshot {
    pub provider: String,
    pub model: String,
    pub base_url: Option<String>,
    pub soul: Option<String>,
    pub has_api_key: bool,
}

/// Configuration validation result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
}

/// Model info from provider.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelInfo {
    pub id: String,
    pub name: String,
}

/// Memory entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MemoryEntry {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Tool source origin.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum ToolSource {
    RustWasm,
    Mcp,
    Plugin,
}

/// Tool info.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolInfo {
    pub name: String,
    pub description: String,
    pub source: ToolSource,
}

/// Health check report.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthReport {
    pub config_ok: bool,
    pub api_reachable: bool,
    pub workspace_ok: bool,
    pub errors: Vec<String>,
}

/// Core Public API contract exposed behind the Protocol Interface Layer.
pub trait CoreRuntimeApi {
    type EventStream;

    // === Turn execution ===
    fn start(&self, envelope: ProtocolEnvelope<CoreRequest>) -> Result<RunRef, ProtocolError>;

    /// Start streaming execution — returns a channel that yields CoreEvents in real time.
    /// Default implementation falls back to start() (no streaming — returns empty channel).
    fn start_streaming(
        &self,
        envelope: ProtocolEnvelope<CoreRequest>,
    ) -> Result<(RunRef, tokio::sync::mpsc::UnboundedReceiver<CoreEvent>), ProtocolError> {
        let run_ref = self.start(envelope)?;
        let (_, rx) = tokio::sync::mpsc::unbounded_channel();
        Ok((run_ref, rx))
    }

    fn send(&self, envelope: ProtocolEnvelope<CoreCommand>) -> Result<(), ProtocolError>;

    fn subscribe(&self, run_ref: &RunRef) -> Result<Self::EventStream, ProtocolError>;

    // === Session management ===
    fn inspect(&self, session_ref: &SessionRef) -> Result<SessionSnapshot, ProtocolError>;

    fn list_sessions(
        &self,
        workspace_ref: &WorkspaceRef,
    ) -> Result<Vec<SessionSummary>, ProtocolError>;

    fn close_session(&self, session_ref: &SessionRef) -> Result<(), ProtocolError>;

    fn clear_conversation(&self, session_ref: &SessionRef) -> Result<(), ProtocolError>;

    // === Config ===
    fn config_read(&self) -> Result<ConfigSnapshot, ProtocolError>;

    fn config_validate(&self) -> Result<ValidationResult, ProtocolError>;

    fn config_update(&self, key: &str, value: serde_json::Value) -> Result<(), ProtocolError>;

    fn model_list(&self) -> Result<Vec<ModelInfo>, ProtocolError>;

    // === Memory ===
    fn memory_save(&self, text: &str, tags: Vec<String>) -> Result<(), ProtocolError>;

    fn memory_list(&self) -> Result<Vec<MemoryEntry>, ProtocolError>;

    fn memory_clear(&self) -> Result<(), ProtocolError>;

    // === Tool ===
    fn tool_list(&self) -> Result<Vec<ToolInfo>, ProtocolError>;

    // === Review ===
    fn review_start(&self, session_ref: &SessionRef) -> Result<RunRef, ProtocolError>;

    // === Health ===
    fn health_check(&self) -> Result<HealthReport, ProtocolError>;

    // === Logging ===
    fn query_logs(&self, query: LogQuery) -> Result<Vec<LogRecord>, ProtocolError>;
}

#[derive(thiserror::Error, Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    #[error("unsupported protocol version: expected {expected}, got {actual}")]
    UnsupportedVersion { expected: String, actual: String },

    #[error("invalid protocol message: {0}")]
    InvalidMessage(String),

    #[error("capability denied: {0}")]
    CapabilityDenied(String),

    #[error("run not found: {0}")]
    RunNotFound(RunRef),

    #[error("session not found: {0}")]
    SessionNotFound(SessionRef),

    #[error("workspace mismatch: {0}")]
    WorkspaceMismatch(String),

    #[error("conflicting command: {0}")]
    Conflict(String),

    #[error("internal protocol error: {0}")]
    Internal(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_turn_rejects_empty_text() {
        let error = CoreRequest::start_turn("   ").unwrap_err();
        assert!(matches!(error, ProtocolError::InvalidMessage(_)));
    }

    #[test]
    fn envelope_sets_protocol_version_and_trace() {
        let request = CoreRequest::start_turn("Summarize this project").unwrap();
        let envelope =
            ProtocolEnvelope::new(Origin::LocalTui, CapabilityScope::local_tui(), request)
                .with_workspace_root("/tmp/project");

        assert_eq!(envelope.protocol_version, PROTOCOL_VERSION);
        assert!(!envelope.trace_id.as_str().is_empty());
        assert!(envelope.validate_protocol_version().is_ok());
        assert!(envelope.capability_scope.has(Capability::WriteWorkspace));
    }

    #[test]
    fn command_targets_run_ref() {
        let run_ref = RunRef::new();
        let command = CoreCommand::cancel(run_ref.clone(), Some("user cancelled".to_string()));

        assert_eq!(command.target_run, run_ref);
        assert_eq!(command.kind, CoreCommandKind::Cancel);
        assert_eq!(command.metadata.reason.as_deref(), Some("user cancelled"));
    }

    #[test]
    fn final_event_preserves_run_and_trace() {
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();
        let event = CoreEvent::final_result(run_ref.clone(), trace_id.clone(), "done");

        assert_eq!(event.run_ref, run_ref);
        assert_eq!(event.trace_id, trace_id);
        assert_eq!(event.kind, CoreEventKind::FinalResult);
        assert!(matches!(
            event.payload,
            CoreEventPayload::Final { success: true, .. }
        ));
    }

    #[test]
    fn request_round_trips_through_json() {
        let request = CoreRequest::open_session(
            Some("feature/protocol-baseline".to_string()),
            SessionPurpose::FeatureDevelopment,
        );

        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: CoreRequest = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded.kind, CoreRequestKind::OpenSession);
        assert_eq!(
            decoded.metadata.session_name.as_deref(),
            Some("feature/protocol-baseline")
        );
        assert_eq!(decoded.metadata.purpose, SessionPurpose::FeatureDevelopment);
    }

    #[test]
    fn loop_policy_presets_match_runtime_modes() {
        let chat = LoopPolicy::chat();
        assert_eq!(chat.max_iterations, 1);
        assert!(chat.tools_enabled);
        assert!(!chat.planning_enabled);
        assert!(chat.require_convergence_check);
        assert!(!chat.require_approval_for_tools);

        let plan = LoopPolicy::plan();
        assert_eq!(plan.max_iterations, 20);
        assert!(plan.tools_enabled);
        assert!(plan.planning_enabled);
        assert!(plan.require_convergence_check);
        assert!(plan.require_approval_for_tools);
    }

    #[test]
    fn run_loop_request_round_trips_through_json() {
        let request =
            CoreRequest::run_loop("implement feature", RuntimeMode::Plan, LoopPolicy::plan())
                .unwrap();

        let encoded = serde_json::to_string(&request).unwrap();
        let decoded: CoreRequest = serde_json::from_str(&encoded).unwrap();

        assert_eq!(decoded.kind, CoreRequestKind::RunLoop);
        match decoded.input {
            RequestInput::RunLoop { input } => {
                assert_eq!(input.content, "implement feature");
                assert_eq!(input.mode, RuntimeMode::Plan);
                assert_eq!(input.policy.max_iterations, 20);
            }
            other => panic!("unexpected input: {:?}", other),
        }
    }
}
