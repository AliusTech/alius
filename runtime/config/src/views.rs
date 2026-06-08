//! Configuration view structures.
//!
//! This module defines all configuration view types that Config Manager
//! outputs for use by Core Runtime and other modules.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// =============================================================================
// Core Snapshots
// =============================================================================

/// Complete project configuration snapshot.
///
/// Contains all configuration data loaded from embedded defaults,
/// user config, project config, and environment overrides.
#[derive(Debug, Clone)]
pub struct ProjectConfigSnapshot {
    /// Project metadata.
    pub project: ProjectMeta,
    /// Runtime settings.
    pub runtime: RuntimeMeta,
    /// Model settings.
    pub model: ModelMeta,
    /// Plan/Execute/Review model assignment.
    pub model_assignment: ModelAssignmentConfig,
    /// Session settings.
    pub session: SessionMeta,
    /// Logging settings.
    pub logging: LoggingMeta,
    /// Compatibility settings.
    pub compat: CompatMeta,
    /// Provider configuration.
    pub providers: ProviderConfig,
    /// Tool configuration.
    pub tools: ToolConfig,
    /// Permission configuration.
    pub permissions: PermissionConfig,
    /// Protocol configuration.
    pub protocol: ProtocolConfig,
    /// Soul configuration.
    pub soul: SoulConfig,
}

/// Runtime configuration view for Core Runtime.
///
/// Resolved configuration ready for use by Core Runtime at startup.
#[derive(Debug, Clone)]
pub struct RuntimeConfigView {
    /// Resolved provider configuration.
    pub provider: ResolvedProviderConfig,
    /// Resolved tool configuration.
    pub tools: ResolvedToolConfig,
    /// Resolved permission configuration.
    pub permissions: ResolvedPermissionConfig,
    /// Shell Gate configuration.
    pub shell_gate: ShellGateConfig,
    /// Logging configuration.
    pub logging: LoggingConfig,
    /// Session configuration.
    pub session: SessionConfig,
    /// Resolved soul configuration.
    pub soul: ResolvedSoulConfig,
}

// =============================================================================
// Meta Structures (from config.toml)
// =============================================================================

/// Project metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectMeta {
    /// Project name.
    pub name: String,
    /// Configuration version.
    pub version: u32,
}

/// Runtime settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeMeta {
    /// Default runtime mode.
    pub default_mode: String,
    /// Enable TUI workspace.
    pub tui_workspace: bool,
    /// Legacy REPL environment variable.
    pub legacy_repl_env: String,
    /// Auto-review after changes.
    pub auto_review: bool,
}

/// Model settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelMeta {
    /// Default provider.
    pub default_provider: String,
    /// Default model.
    pub default_model: String,
    /// Router profile.
    pub router_profile: String,
}

// =============================================================================
// Model Assignment (from model.toml)
// =============================================================================

/// Project-local Plan/Execute/Review model assignment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelAssignmentConfig {
    /// Configuration schema version.
    pub schema_version: String,
    /// Assigned model ids for each execution role.
    pub assignment: ModelAssignment,
}

/// Assigned model ids for the Plan/Execute/Review model architecture.
#[derive(Debug, Clone, Serialize, Deserialize, Default, PartialEq, Eq)]
pub struct ModelAssignment {
    /// Model id used for planning.
    #[serde(default)]
    pub plan: String,
    /// Model id used for normal execution.
    #[serde(default)]
    pub execute: String,
    /// Model id used for review.
    #[serde(default)]
    pub review: String,
}

/// Model assignment role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModelAssignmentRole {
    Plan,
    Execute,
    Review,
}

impl Default for ModelAssignmentConfig {
    fn default() -> Self {
        Self {
            schema_version: "0.1".to_string(),
            assignment: ModelAssignment::default(),
        }
    }
}

impl ModelAssignmentConfig {
    pub fn get(&self, role: ModelAssignmentRole) -> &str {
        match role {
            ModelAssignmentRole::Plan => &self.assignment.plan,
            ModelAssignmentRole::Execute => &self.assignment.execute,
            ModelAssignmentRole::Review => &self.assignment.review,
        }
    }

    pub fn set(&mut self, role: ModelAssignmentRole, model_id: impl Into<String>) {
        match role {
            ModelAssignmentRole::Plan => self.assignment.plan = model_id.into(),
            ModelAssignmentRole::Execute => self.assignment.execute = model_id.into(),
            ModelAssignmentRole::Review => self.assignment.review = model_id.into(),
        }
    }

    pub fn missing_roles(&self) -> Vec<ModelAssignmentRole> {
        ModelAssignmentRole::all()
            .into_iter()
            .filter(|role| self.get(*role).trim().is_empty())
            .collect()
    }

    pub fn referenced_by(&self, model_id: &str) -> Vec<ModelAssignmentRole> {
        ModelAssignmentRole::all()
            .into_iter()
            .filter(|role| self.get(*role) == model_id)
            .collect()
    }

    pub fn from_provider_tiers(providers: &ProviderConfig) -> Self {
        let mut config = Self::default();
        config.assignment.plan = tier_model_id(providers, &providers.tiers.light);
        config.assignment.execute = tier_model_id(providers, &providers.tiers.medium);
        config.assignment.review = tier_model_id(providers, &providers.tiers.high);
        config
    }
}

impl ModelAssignmentRole {
    pub fn all() -> [Self; 3] {
        [Self::Plan, Self::Execute, Self::Review]
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Plan => "Plan Model",
            Self::Execute => "Execute Model",
            Self::Review => "Review Model",
        }
    }

    pub fn compatibility_tier(self) -> &'static str {
        match self {
            Self::Plan => "light",
            Self::Execute => "medium",
            Self::Review => "high",
        }
    }
}

fn tier_model_id(providers: &ProviderConfig, tier: &TierConfig) -> String {
    if tier.model.trim().is_empty() {
        return String::new();
    }
    providers
        .model_library
        .models
        .iter()
        .find(|entry| entry.provider == tier.provider && entry.model_name == tier.model)
        .map(|entry| entry.id.clone())
        .unwrap_or_else(|| tier.model.clone())
}

/// Session settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionMeta {
    /// Persist messages.
    pub persist_messages: bool,
    /// Persist events.
    pub persist_events: bool,
}

/// Logging settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingMeta {
    /// Enable logging.
    pub enabled: bool,
    /// Log level.
    pub level: String,
    /// Redact secrets.
    pub redact_secrets: bool,
    /// Flush errors immediately.
    pub flush_error_immediately: bool,
}

/// Compatibility settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompatMeta {
    /// Read legacy project config.
    pub read_legacy_project_config: bool,
    /// Read legacy MCP config.
    pub read_legacy_mcp_config: bool,
    /// Read legacy project memory.
    pub read_legacy_project_memory: bool,
    /// Read legacy design docs.
    pub read_legacy_design_docs: bool,
}

// =============================================================================
// Provider Configuration (from providers.toml)
// =============================================================================

/// Provider and routing configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    /// Router settings.
    pub router: RouterConfig,
    /// Model tier configurations.
    pub tiers: TierConfigs,
    /// Individual provider configurations.
    pub providers: std::collections::HashMap<String, ProviderSettings>,
    /// Project-local model inventory.
    #[serde(default)]
    pub model_library: ModelLibraryConfig,
}

/// Router configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RouterConfig {
    /// Routing strategy.
    pub strategy: String,
    /// Default tier.
    pub default_tier: String,
    /// Fallback tier.
    pub fallback_tier: String,
}

/// Model tier configurations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfigs {
    /// Light tier for fast tasks.
    pub light: TierConfig,
    /// Medium tier for default work.
    pub medium: TierConfig,
    /// High tier for complex tasks.
    pub high: TierConfig,
}

/// Single tier configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TierConfig {
    /// Tier description.
    pub description: String,
    /// Provider for this tier.
    pub provider: String,
    /// Model for this tier (empty means use default).
    pub model: String,
}

/// Individual provider settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderSettings {
    /// Whether this provider is enabled.
    pub enabled: bool,
    /// Provider kind (openai, anthropic, google, openai-compatible).
    pub kind: String,
    /// Base URL for API calls.
    pub base_url: String,
    /// Environment variable name for API key.
    pub api_key_env: String,
}

/// Local model inventory stored with provider routing configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelLibraryConfig {
    /// Available model entries.
    #[serde(default)]
    pub models: Vec<ModelLibraryEntry>,
}

/// A project-local model entry.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ModelLibraryEntry {
    /// Stable model entry identifier.
    pub id: String,
    /// Human-facing model label.
    pub display_name: String,
    /// Provider key from `providers`.
    pub provider: String,
    /// Base URL used for this model.
    pub base_url: String,
    /// Provider-native model name.
    pub model_name: String,
    /// User-facing reasoning note.
    pub reasoning_note: ReasoningNote,
    /// Whether this model can be selected.
    pub enabled: bool,
}

/// User-facing reasoning note for model selection.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ReasoningNote {
    /// Fast model use cases.
    #[serde(rename = "Quick Reasoning")]
    Quick,
    /// Default project reasoning.
    #[serde(rename = "Standard Reasoning")]
    Standard,
    /// Complex or deep reasoning.
    #[serde(rename = "Deep Reasoning")]
    Deep,
}

impl ReasoningNote {
    pub fn label(self) -> &'static str {
        match self {
            Self::Quick => "Quick Reasoning",
            Self::Standard => "Standard Reasoning",
            Self::Deep => "Deep Reasoning",
        }
    }

    pub fn tier(self) -> &'static str {
        match self {
            Self::Quick => "light",
            Self::Standard => "medium",
            Self::Deep => "high",
        }
    }

    pub fn from_tier(tier: &str) -> Option<Self> {
        match tier {
            "light" => Some(Self::Quick),
            "medium" => Some(Self::Standard),
            "high" => Some(Self::Deep),
            _ => None,
        }
    }

    pub fn all() -> [Self; 3] {
        [Self::Quick, Self::Standard, Self::Deep]
    }
}

/// Resolved provider configuration for runtime use.
#[derive(Debug, Clone)]
pub struct ResolvedProviderConfig {
    /// Default provider type.
    pub default_provider: String,
    /// Default model.
    pub default_model: String,
    /// Router strategy.
    pub router_strategy: String,
    /// Configured providers.
    pub providers: Vec<ResolvedProvider>,
}

/// Resolved provider with API key.
#[derive(Debug, Clone)]
pub struct ResolvedProvider {
    /// Provider name.
    pub name: String,
    /// Provider kind.
    pub kind: String,
    /// Base URL.
    pub base_url: String,
    /// API key (resolved from environment).
    pub api_key: Option<String>,
}

// =============================================================================
// Tool Configuration (from tools.toml)
// =============================================================================

/// Tool registration and execution configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfig {
    /// Tool registry settings.
    pub registry: ToolRegistryConfig,
    /// Tool execution settings.
    pub execution: ToolExecutionConfig,
    /// Tool confirmation policy.
    pub confirmation: ToolConfirmationConfig,
    /// MCP configuration.
    pub mcp: McpToolConfig,
    /// Plugin configuration.
    pub plugin: PluginToolConfig,
    /// Workflow configuration.
    pub workflow: WorkflowToolConfig,
}

/// Tool registry settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolRegistryConfig {
    /// Enable Rust WASM module tools.
    pub rust_wasm_modules: bool,
    /// Enable MCP tools.
    pub mcp_tools: bool,
    /// Enable workflow tools.
    pub workflow_tools: bool,
}

/// Tool execution settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolExecutionConfig {
    /// Default timeout in milliseconds.
    pub default_timeout_ms: u64,
    /// Maximum parallel tool calls.
    pub max_parallel_tools: u32,
    /// Persist tool traces.
    pub persist_tool_trace: bool,
}

/// Tool confirmation policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolConfirmationConfig {
    /// Read-only tool confirmation policy.
    pub read_only_tools: String,
    /// Write tool confirmation policy.
    pub write_tools: String,
    /// Shell tool confirmation policy.
    pub shell_tools: String,
    /// Network tool confirmation policy.
    pub network_tools: String,
    /// Remote A2A tool confirmation policy.
    pub remote_a2a_tools: String,
}

/// MCP tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpToolConfig {
    /// Path to MCP config file.
    pub config: String,
    /// Load MCP on workspace start.
    pub load_on_workspace_start: bool,
    /// Register MCP tools.
    pub register_as_tools: bool,
}

/// Plugin tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginToolConfig {
    /// Load plugins on workspace start.
    pub load_on_workspace_start: bool,
    /// Register plugin tools.
    pub register_as_tools: bool,
}

/// Workflow tool configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowToolConfig {
    /// Load project workflows.
    pub load_project_workflows: bool,
    /// Register workflow tools.
    pub register_as_tools: bool,
}

/// Resolved tool configuration for runtime use.
#[derive(Debug, Clone)]
pub struct ResolvedToolConfig {
    /// Enable Rust WASM module tools.
    pub rust_wasm_modules: bool,
    /// Enable MCP tools.
    pub mcp_tools: bool,
    /// Enable workflow tools.
    pub workflow_tools: bool,
    /// Maximum parallel tools.
    pub max_parallel_tools: u32,
    /// Default timeout.
    pub default_timeout_ms: u64,
}

// =============================================================================
// Permission Configuration (from permissions.toml)
// =============================================================================

/// Permission and capability boundary configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionConfig {
    /// Filesystem permissions.
    pub filesystem: FilesystemPermission,
    /// Shell permissions.
    pub shell: ShellPermission,
    /// Network permissions.
    pub network: NetworkPermission,
    /// Memory permissions.
    pub memory: MemoryPermission,
    /// Project document permissions.
    pub project_documents: ProjectDocumentPermission,
    /// Remote A2A permissions.
    pub remote_a2a: RemoteA2APermission,
    /// Embedded SDK permissions.
    pub embedded_sdk: EmbeddedSdkPermission,
}

/// Filesystem permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilesystemPermission {
    /// Workspace root path.
    pub workspace_root: String,
    /// Allow read operations.
    pub allow_read: bool,
    /// Allow write operations.
    pub allow_write: bool,
    /// Allow delete operations.
    pub allow_delete: bool,
    /// Require confirmation for writes.
    pub require_confirmation_for_write: bool,
    /// Require confirmation for deletes.
    pub require_confirmation_for_delete: bool,
}

/// Shell permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellPermission {
    /// Enable shell commands.
    pub enabled: bool,
    /// Require confirmation.
    pub require_confirmation: bool,
    /// Scope to workspace.
    pub workspace_scoped: bool,
    /// Deny unknown scope.
    pub deny_unknown_scope: bool,
    /// Require confirmation for outside workspace.
    pub require_confirmation_for_outside_workspace: bool,
    /// Allowed commands.
    pub allowlist: Vec<String>,
    /// Denied commands.
    pub denylist: Vec<String>,
    /// Shell scope settings.
    pub scope: ShellScopeConfig,
}

/// Shell scope configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ShellScopeConfig {
    /// Allow read outside workspace.
    pub allow_read_outside_workspace: bool,
    /// Allow write outside workspace.
    pub allow_write_outside_workspace: bool,
    /// Allow delete workspace root.
    pub allow_delete_workspace_root: bool,
    /// Allow delete outside workspace.
    pub allow_delete_outside_workspace: bool,
    /// Follow symlinks outside workspace.
    pub follow_symlink_outside_workspace: bool,
    /// Allow redirection outside workspace.
    pub allow_redirection_outside_workspace: bool,
    /// Allow shell eval without inspection.
    pub allow_shell_eval_without_inspection: bool,
}

/// Network permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPermission {
    /// Enable network access.
    pub enabled: bool,
    /// Require confirmation.
    pub require_confirmation: bool,
    /// Allowed endpoints.
    pub allowlist: Vec<String>,
    /// Denied endpoints.
    pub denylist: Vec<String>,
}

/// Memory permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoryPermission {
    /// Allow read.
    pub allow_read: bool,
    /// Allow write.
    pub allow_write: bool,
    /// Allow semantic index rebuild.
    pub allow_semantic_index_rebuild: bool,
}

/// Project document permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectDocumentPermission {
    /// Allow updates.
    pub allow_update: bool,
    /// Require history entry.
    pub require_history_entry: bool,
    /// Document root path.
    pub root: String,
}

/// Remote A2A permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RemoteA2APermission {
    /// Enable remote A2A.
    pub enabled: bool,
    /// Allow filesystem access.
    pub allow_filesystem: bool,
    /// Allow shell access.
    pub allow_shell: bool,
    /// Allow network access.
    pub allow_network: bool,
    /// Allowed tools.
    pub allowed_tools: Vec<String>,
}

/// Embedded SDK permissions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EmbeddedSdkPermission {
    /// Allow shell access.
    pub allow_shell: bool,
    /// Allow local tools.
    pub allow_local_tools: bool,
    /// Allow LanceDB.
    pub allow_lancedb: bool,
    /// Allow local embedding.
    pub allow_local_embedding: bool,
    /// Allow plugin runtime.
    pub allow_plugin_runtime: bool,
}

/// Resolved permission configuration for runtime use.
#[derive(Debug, Clone)]
pub struct ResolvedPermissionConfig {
    /// Allow read operations.
    pub allow_read: bool,
    /// Allow write operations.
    pub allow_write: bool,
    /// Allow delete operations.
    pub allow_delete: bool,
    /// Require confirmation for writes.
    pub require_confirmation_for_write: bool,
    /// Require confirmation for deletes.
    pub require_confirmation_for_delete: bool,
}

// =============================================================================
// Protocol Configuration (from protocol.toml)
// =============================================================================

/// Protocol Interface Layer configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolConfig {
    /// Protocol version.
    pub protocol: ProtocolVersionConfig,
    /// Local Rust interface settings.
    pub local_rust: LocalRustConfig,
    /// JSON-RPC settings.
    pub json_rpc: JsonRpcConfig,
    /// IDE RPC settings.
    pub ide_rpc: IdeRpcConfig,
    /// A2A settings.
    pub a2a: A2AConfig,
    /// FFI settings.
    pub ffi: FfiConfig,
    /// Event settings.
    pub events: EventConfig,
    /// Command settings.
    pub commands: CommandConfig,
}

/// Protocol version configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProtocolVersionConfig {
    /// Major version.
    pub major: u32,
    /// Minor version.
    pub minor: u32,
    /// Enable tracing.
    pub trace_enabled: bool,
    /// Enable event sequencing.
    pub event_sequence_enabled: bool,
}

/// Local Rust interface configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalRustConfig {
    /// Enable local Rust interface.
    pub enabled: bool,
    /// Transport type.
    pub transport: String,
    /// Default origin.
    pub default_origin: String,
}

/// JSON-RPC configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcConfig {
    /// Enable JSON-RPC.
    pub enabled: bool,
    /// Transport type.
    pub transport: String,
    /// Socket path.
    pub socket_path: String,
    /// Method prefix.
    pub method_prefix: String,
}

/// IDE RPC configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdeRpcConfig {
    /// Enable IDE RPC.
    pub enabled: bool,
    /// Transport type.
    pub transport: String,
    /// Workspace-scoped filesystem.
    pub workspace_scoped_filesystem: bool,
}

/// A2A configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AConfig {
    /// Enable A2A.
    pub enabled: bool,
    /// Enable A2A server.
    pub server_enabled: bool,
    /// Enable A2A client.
    pub client_enabled: bool,
    /// Agent card source path.
    pub agent_card_source: String,
    /// Default remote capability.
    pub default_remote_capability: String,
}

/// FFI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FfiConfig {
    /// Enable FFI.
    pub enabled: bool,
    /// Core type (full or lite).
    pub core: String,
    /// Event delivery mode.
    pub event_delivery: String,
}

/// Event configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventConfig {
    /// Event buffer size.
    pub buffer_size: u32,
    /// Persist events.
    pub persist: bool,
    /// Allow event resume.
    pub allow_resume: bool,
    /// Default visibility.
    pub visibility_default: String,
}

/// Command configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CommandConfig {
    /// Enable approve_tool command.
    pub approve_tool: bool,
    /// Enable reject_tool command.
    pub reject_tool: bool,
    /// Enable answer_question command.
    pub answer_question: bool,
    /// Enable select_option command.
    pub select_option: bool,
    /// Enable update_plan command.
    pub update_plan: bool,
    /// Enable cancel_run command.
    pub cancel_run: bool,
    /// Enable pause_run command.
    pub pause_run: bool,
    /// Enable resume_run command.
    pub resume_run: bool,
}

// =============================================================================
// Soul Configuration (from soul.toml)
// =============================================================================

/// Soul configuration for agent identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SoulConfig {
    /// Agent identity.
    pub agent: AgentIdentity,
    /// Agent card settings.
    pub agent_card: AgentCardSettings,
    /// Supported interfaces.
    pub supported_interfaces: Vec<SupportedInterface>,
    /// Provider information.
    pub provider: AgentProvider,
    /// Agent capabilities.
    pub capabilities: AgentCapabilities,
    /// Interaction modes.
    pub interaction: InteractionModes,
    /// Agent skills.
    pub skills: Vec<AgentSkill>,
}

/// Agent identity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentIdentity {
    /// Agent name.
    pub name: String,
    /// Agent description.
    pub description: String,
    /// Agent version.
    pub version: String,
}

/// Agent card settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardSettings {
    /// Documentation URL.
    pub documentation_url: String,
    /// Icon URL.
    pub icon_url: String,
    /// Export path for agent-card.json.
    pub export_path: String,
}

/// Supported interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SupportedInterface {
    /// Interface URL.
    pub url: String,
    /// Protocol binding.
    pub protocol_binding: String,
    /// Protocol version.
    pub protocol_version: String,
}

/// Agent provider information.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProvider {
    /// Organization name.
    pub organization: String,
    /// Provider URL.
    pub url: String,
}

/// Agent capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCapabilities {
    /// Support streaming.
    pub streaming: bool,
    /// Support push notifications.
    pub push_notifications: bool,
    /// Support extended agent card.
    pub extended_agent_card: bool,
}

/// Interaction modes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionModes {
    /// Default input modes.
    pub default_input_modes: Vec<String>,
    /// Default output modes.
    pub default_output_modes: Vec<String>,
}

/// Agent skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSkill {
    /// Skill ID.
    pub id: String,
    /// Skill name.
    pub name: String,
    /// Skill description.
    pub description: String,
    /// Skill tags.
    pub tags: Vec<String>,
    /// Skill examples.
    pub examples: Vec<String>,
    /// Input modes.
    pub input_modes: Vec<String>,
    /// Output modes.
    pub output_modes: Vec<String>,
}

/// Resolved soul configuration for runtime use.
#[derive(Debug, Clone)]
pub struct ResolvedSoulConfig {
    /// Agent name.
    pub name: String,
    /// Agent description.
    pub description: String,
    /// System prompt (derived from role or generated).
    pub system_prompt: String,
}

// =============================================================================
// Agent Card View (for A2A)
// =============================================================================

/// Normalized Agent Card view for A2A publishing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardView {
    /// Agent name.
    pub name: String,
    /// Agent description.
    pub description: String,
    /// Agent version.
    pub version: String,
    /// Documentation URL.
    #[serde(rename = "documentationUrl")]
    pub documentation_url: String,
    /// Icon URL.
    #[serde(rename = "iconUrl")]
    pub icon_url: String,
    /// Provider information.
    pub provider: AgentCardProvider,
    /// Agent capabilities.
    pub capabilities: AgentCardCapabilities,
    /// Supported interfaces.
    #[serde(rename = "supportedInterfaces")]
    pub supported_interfaces: Vec<AgentCardInterface>,
    /// Agent skills.
    pub skills: Vec<AgentCardSkill>,
    /// Interaction modes.
    pub interaction: AgentCardInteraction,
}

/// Agent card provider.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardProvider {
    /// Organization name.
    pub organization: String,
    /// Provider URL.
    pub url: String,
}

/// Agent card capabilities.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardCapabilities {
    /// Support streaming.
    pub streaming: bool,
    /// Support push notifications.
    #[serde(rename = "pushNotifications")]
    pub push_notifications: bool,
    /// Support extended agent card.
    #[serde(rename = "extendedAgentCard")]
    pub extended_agent_card: bool,
}

/// Agent card interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardInterface {
    /// Interface URL.
    pub url: String,
    /// Protocol binding.
    #[serde(rename = "protocolBinding")]
    pub protocol_binding: String,
    /// Protocol version.
    #[serde(rename = "protocolVersion")]
    pub protocol_version: String,
}

/// Agent card skill.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardSkill {
    /// Skill ID.
    pub id: String,
    /// Skill name.
    pub name: String,
    /// Skill description.
    pub description: String,
    /// Skill tags.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    /// Skill examples.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<String>,
    /// Input modes.
    #[serde(rename = "inputModes")]
    pub input_modes: Vec<String>,
    /// Output modes.
    #[serde(rename = "outputModes")]
    pub output_modes: Vec<String>,
}

/// Agent card interaction.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentCardInteraction {
    /// Default input modes.
    #[serde(rename = "defaultInputModes")]
    pub default_input_modes: Vec<String>,
    /// Default output modes.
    #[serde(rename = "defaultOutputModes")]
    pub default_output_modes: Vec<String>,
}

// =============================================================================
// Shell Gate Configuration
// =============================================================================

/// Shell Gate configuration for secure command execution.
#[derive(Debug, Clone)]
pub struct ShellGateConfig {
    /// Whether shell is enabled.
    pub enabled: bool,
    /// Require confirmation for commands.
    pub require_confirmation: bool,
    /// Workspace root path.
    pub workspace_root: PathBuf,
    /// Allowed commands (allowlist).
    pub allowlist: Vec<String>,
    /// Denied commands (denylist).
    pub denylist: Vec<String>,
    /// Allow read outside workspace.
    pub allow_read_outside_workspace: bool,
    /// Allow write outside workspace.
    pub allow_write_outside_workspace: bool,
    /// Allow delete workspace root.
    pub allow_delete_workspace_root: bool,
    /// Allow delete outside workspace.
    pub allow_delete_outside_workspace: bool,
    /// Follow symlinks outside workspace.
    pub follow_symlink_outside_workspace: bool,
    /// Allow redirection outside workspace.
    pub allow_redirection_outside_workspace: bool,
}

// =============================================================================
// Logging Configuration
// =============================================================================

/// Logging configuration for Logging Manager.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Whether logging is enabled.
    pub enabled: bool,
    /// Log level.
    pub level: LogLevel,
    /// Redact secrets in logs.
    pub redact_secrets: bool,
    /// Flush errors immediately.
    pub flush_error_immediately: bool,
    /// Log directory path.
    pub log_dir: PathBuf,
}

/// Log level.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    /// Trace level.
    Trace,
    /// Debug level.
    Debug,
    /// Info level.
    Info,
    /// Warning level.
    Warn,
    /// Error level.
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

impl std::str::FromStr for LogLevel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "trace" => Ok(LogLevel::Trace),
            "debug" => Ok(LogLevel::Debug),
            "info" => Ok(LogLevel::Info),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "error" => Ok(LogLevel::Error),
            _ => Err(format!("Invalid log level: {}", s)),
        }
    }
}

// =============================================================================
// Session Configuration
// =============================================================================

/// Session configuration for Session Manager.
#[derive(Debug, Clone)]
pub struct SessionConfig {
    /// Persist messages.
    pub persist_messages: bool,
    /// Persist events.
    pub persist_events: bool,
    /// Session storage path.
    pub session_path: PathBuf,
}
