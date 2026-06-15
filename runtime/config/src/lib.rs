pub mod agent_card;
pub mod capability;
pub mod config_manager;
pub mod error;
pub mod init_wizard;
pub mod loaders;
pub mod merger;
pub mod migration;
pub mod project_init;
pub mod settings;
pub mod soul;
pub mod soul_source;
pub mod views;
pub mod workspace_archive;
pub mod workspace_template;

pub use agent_card::{
    export_agent_card_json, export_agent_card_to_file, is_ready_for_publishing,
    normalize_agent_card, validate_agent_card_for_publishing,
};
pub use config_manager::{build_runtime_config, find_project_root, load_project_config};
pub use error::{ConfigError, ConfigResult};
pub use init_wizard::{
    ActionOption, ActionPanel, ApiProtocol as InitApiProtocol, CheckItemStatus,
    ConfigIssue as InitConfigIssue, InitCheckItem, InitCommand, InitConfigSection, InitContext,
    InitEvent, InitMessage, InitStage, InitState, InitViewModel, InitWizard,
    ModelInfo as InitModelInfo, MultiChoiceOption, RecoverAction, RenderedCheckItem,
    RenderedMessage, SoulRef as InitSoulRef, WorkspaceCheckResult,
};
pub use loaders::{save_model_assignment, validate_model_assignment};
pub use merger::{
    load_env_overrides, merge_model, merge_provider, merge_runtime_mode, CliOverrides,
    ConfigSource, EmbeddedDefaults, EnvOverrides, MergeResult, UserConfig,
};
pub use migration::{cleanup_legacy_files, migrate_legacy_config, MigrationReport};
pub use protocol_interface::{ProviderMode, ProviderType, SoulRole};
pub use settings::{
    AgentSettings, LlmSettings, Settings, SoulSettings, UiSettings, UpdateSettings,
};
pub use soul::system_prompt_for_role;
pub use views::{
    A2AConfig, AgentCapabilities, AgentCardCapabilities, AgentCardInteraction, AgentCardInterface,
    AgentCardProvider, AgentCardSettings, AgentCardSkill, AgentCardView, AgentIdentity,
    AgentProvider, AgentSkill, CommandConfig, CompatMeta, EventConfig, FfiConfig,
    FilesystemPermission, IdeRpcConfig, InteractionModes, JsonRpcConfig, LocalRustConfig, LogLevel,
    LoggingConfig, LoggingMeta, McpToolConfig, MemoryPermission, ModelAssignment,
    ModelAssignmentConfig, ModelAssignmentRole, ModelLibraryConfig, ModelLibraryEntry, ModelMeta,
    NetworkPermission, PermissionConfig, PluginToolConfig, ProjectConfigSnapshot,
    ProjectDocumentPermission, ProjectMeta, ProtocolConfig, ProtocolVersionConfig, ProviderConfig,
    ProviderSettings, ReasoningNote, RemoteA2APermission, ResolvedPermissionConfig,
    ResolvedProvider, ResolvedProviderConfig, ResolvedSoulConfig, ResolvedToolConfig, RouterConfig,
    RuntimeConfigView, RuntimeMeta, SessionConfig, SessionMeta, ShellGateConfig, ShellPermission,
    ShellScopeConfig, SoulConfig, SupportedInterface, TierConfig, TierConfigs, ToolConfig,
    ToolConfirmationConfig, ToolExecutionConfig, ToolRegistryConfig, WorkflowToolConfig,
};

/// Parse a provider type string (e.g., "openai", "anthropic") into a ProviderType.
pub fn parse_provider_type(s: &str) -> ProviderType {
    serde_json::from_value(serde_json::Value::String(s.to_string())).unwrap_or_default()
}
