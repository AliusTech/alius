use runtime_config::{
    load_project_config, ActionPanel as InitActionPanel, CheckItemStatus, InitApiProtocol,
    InitCommand, InitConfigIssue, InitConfigSection, InitContext, InitEvent, InitMessage,
    InitModelInfo, InitSoulRef, InitStage, InitState, InitViewModel, InitWizard,
    ModelAssignmentConfig, ModelAssignmentRole, ModelLibraryEntry, ProviderConfig, ProviderMode,
    ProviderSettings, ProviderType, ReasoningNote, Settings, SoulRole, TierConfig,
};
use runtime_model::LlmClient;
use rust_i18n::t;
use std::collections::HashSet;
use std::path::{Path, PathBuf};

use super::conversation::{ConfigOverviewRow, ConfigOverviewState};
use super::interaction::{PromptChoice, PromptInputKind, PromptInputState};
use crate::formula::FormulaDef;

const BIGMODEL_OPENAI_BASE_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4";
const BIGMODEL_ANTHROPIC_BASE_URL: &str = "https://open.bigmodel.cn/api/anthropic";
const XIAOMI_MIMO_OPENAI_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
const XIAOMI_MIMO_ANTHROPIC_BASE_URL: &str = "https://api.xiaomimimo.com/anthropic";
const DEEPSEEK_OPENAI_BASE_URL: &str = "https://api.deepseek.com";
const DEEPSEEK_ANTHROPIC_BASE_URL: &str = "https://api.deepseek.com/anthropic";

const LOCALES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh-CN", "Chinese (Simplified)"),
    ("ja", "Japanese"),
];

#[derive(Debug, Clone)]
pub struct ConfigPrompt {
    pub message: String,
    pub input: PromptInputState,
    pub side_panel: Option<ConfigSidePanel>,
}

#[derive(Debug, Clone)]
pub struct ConfigSidePanel {
    pub title: String,
    pub content: String,
}

/// Snapshot for the `/init` top-bar nav: which stage is active + per-stage done flags.
#[derive(Debug, Clone)]
pub struct InitNavSnapshot {
    pub current: Option<InitStage>,
    pub done: [bool; 4],
}

#[derive(Debug, Clone)]
pub enum ConfigTaskOutcome {
    Next {
        accepted: String,
        prompt: ConfigPrompt,
    },
    /// A selection was made and should be persisted immediately; keep editing.
    Applied {
        settings: Settings,
        providers: Box<ProviderConfig>,
        assignment: Box<ModelAssignmentConfig>,
        prompt: ConfigPrompt,
    },
    Saved {
        settings: Settings,
        providers: Box<ProviderConfig>,
        assignment: Box<ModelAssignmentConfig>,
        message: String,
    },
    Cancelled {
        message: String,
    },
    Invalid {
        message: String,
        prompt: ConfigPrompt,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigTaskKind {
    Config,
    Init,
    ModelPool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSaveTarget {
    Project,
}

#[derive(Debug, Clone)]
pub struct ConfigTask {
    draft: Settings,
    providers: ProviderConfig,
    assignment: ModelAssignmentConfig,
    section: ConfigSection,
    init_wizard: Option<InitWizard>,
    init_ignore_validation: bool,
    assignment_role: Option<ModelAssignmentRole>,
    pool_mode: ModelPoolMode,
    selected_view_model: Option<String>,
    pending_delete_model: Option<String>,
    add_model: Option<AddModelState>,
    soul_choices: Vec<FormulaDef>,
    kind: ConfigTaskKind,
    dirty: bool,
    missing_notice: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigSection {
    ModelAssignment,
    Language,
    Soul,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelPoolMode {
    Main,
    ViewSelect,
    ViewDetail,
    DeleteSelect,
    DeleteConfirm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigChoice {
    label: String,
    value: String,
}

#[derive(Debug, Clone)]
struct AddModelState {
    step: AddModelStep,
    provider: String,
    api_protocol: ApiProtocol,
    base_url: String,
    api_key: String,
    models: Vec<String>,
    selected_models: Vec<String>,
    fetch_failed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AddModelStep {
    Provider,
    ApiProtocol,
    BaseUrl,
    ApiKey,
    SelectModels,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ApiProtocol {
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ConfigIssue {
    message: String,
    section: ConfigSection,
}

impl ConfigTask {
    pub fn new(settings: Settings) -> Self {
        Self::with_kind(settings, ConfigTaskKind::Config)
    }

    pub fn init(settings: Settings) -> Self {
        Self::with_kind(settings, ConfigTaskKind::Init)
    }

    pub fn model_switch(settings: Settings) -> Self {
        Self::with_kind(settings, ConfigTaskKind::ModelPool)
    }

    fn with_kind(settings: Settings, kind: ConfigTaskKind) -> Self {
        let mut providers = load_current_provider_config();
        seed_model_library_from_settings(&settings, &mut providers);
        ensure_active_provider(&settings, &mut providers);
        let assignment = load_current_model_assignment(&providers);

        let soul_choices = load_soul_choices_for_kind(kind);
        let init_wizard = if kind == ConfigTaskKind::Init {
            Some(build_init_wizard(
                &settings,
                &providers,
                &assignment,
                &soul_choices,
            ))
        } else {
            None
        };
        let mut task = Self {
            draft: settings,
            providers,
            assignment,
            section: ConfigSection::ModelAssignment,
            init_wizard,
            init_ignore_validation: false,
            assignment_role: None,
            pool_mode: ModelPoolMode::Main,
            selected_view_model: None,
            pending_delete_model: None,
            add_model: None,
            soul_choices,
            kind,
            dirty: false,
            missing_notice: None,
        };
        if kind == ConfigTaskKind::Config {
            task.jump_to_first_missing();
        }
        // Init: skip the Start confirmation when there is nothing to resume.
        // Advance CheckWorkspace (+ first-run auto-create) WITHOUT persisting
        // init-state, so Esc before any interaction leaves no trace.
        if kind == ConfigTaskKind::Init
            && task
                .init_wizard
                .as_ref()
                .is_some_and(|w| w.state == InitState::Start)
        {
            task.auto_start_init_traceless();
        }
        task
    }

    /// Drive CheckWorkspace (+ first-run `ensure_project_defaults`) forward
    /// without writing `.alius/runtime/init-state.toml`, and without refreshing
    /// the wizard context from existing settings (so a fresh init does not
    /// prefill language/soul/models). Only persists once the user submits a real
    /// answer via `submit_init` → `run_init_command_chain`.
    fn auto_start_init_traceless(&mut self) {
        let cwd = init_cwd();
        let result = runtime_config::project_init::check_workspace(&cwd);
        let cmd = match self.init_wizard.as_mut() {
            Some(w) => w.handle_event(InitEvent::WorkspaceChecked(result)),
            None => return,
        };
        if let InitCommand::CreateProjectDirs { reset: false } = cmd {
            if runtime_config::project_init::ensure_project_defaults(&cwd).is_ok() {
                if let Some(w) = self.init_wizard.as_mut() {
                    let _ = w.handle_event(InitEvent::ProjectCreated);
                }
            }
        }
    }

    pub fn save_target(&self) -> ConfigSaveTarget {
        ConfigSaveTarget::Project
    }

    pub fn kind(&self) -> ConfigTaskKind {
        self.kind
    }

    pub fn prompt(&self) -> ConfigPrompt {
        ConfigPrompt {
            message: self.message(),
            input: self.input_state(),
            side_panel: self.side_panel(),
        }
    }

    /// Current section, but only for `/config` (used to render the nav bar).
    /// Returns `None` for `/init` and `/model`.
    pub fn current_section(&self) -> Option<ConfigSection> {
        if self.kind == ConfigTaskKind::Config {
            Some(self.section)
        } else {
            None
        }
    }

    /// `/init` nav snapshot (only for Init kind).
    pub fn init_nav_snapshot(&self) -> Option<InitNavSnapshot> {
        let wizard = self.init_wizard.as_ref()?;
        let done = InitStage::all().map(|stage| wizard.stage_done(stage));
        Some(InitNavSnapshot {
            current: wizard.current_stage(),
            done,
        })
    }

    /// Step the `/init` wizard back one stage. Returns the refreshed prompt.
    pub fn init_back(&mut self) -> Option<ConfigPrompt> {
        let wizard = self.init_wizard.as_mut()?;
        let _ = wizard.back();
        Some(self.prompt())
    }

    /// Snapshot of the three config sections for the overview conversation block.
    pub fn overview_snapshot(&self) -> ConfigOverviewState {
        ConfigOverviewState {
            title: t!("workspace.config_task.overview_title").to_string(),
            rows: vec![
                ConfigOverviewRow {
                    label: ConfigSection::ModelAssignment.display_label(),
                    done: self.section_done(ConfigSection::ModelAssignment),
                    current: self.execute_model_label(),
                },
                ConfigOverviewRow {
                    label: ConfigSection::Language.display_label(),
                    done: self.section_done(ConfigSection::Language),
                    current: self.draft.ui.locale.clone(),
                },
                ConfigOverviewRow {
                    label: ConfigSection::Soul.display_label(),
                    done: self.section_done(ConfigSection::Soul),
                    current: current_soul(&self.draft),
                },
            ],
        }
    }

    fn section_done(&self, section: ConfigSection) -> bool {
        self.validation_issues()
            .iter()
            .all(|issue| issue.section != section)
    }

    fn execute_model_label(&self) -> String {
        let id = self.assignment.get(ModelAssignmentRole::Execute);
        if id.trim().is_empty() {
            t!("workspace.config_task.not_configured").to_string()
        } else {
            self.model_label(id)
        }
    }

    pub fn switch_tab(&mut self, reverse: bool) -> ConfigPrompt {
        if self.kind == ConfigTaskKind::ModelPool || self.kind == ConfigTaskKind::Init {
            return self.prompt();
        }
        self.assignment_role = None;
        self.section = self.section.shift(reverse);
        self.prompt()
    }

    pub fn submit(&mut self, input: &str) -> ConfigTaskOutcome {
        let raw = input.trim();
        if is_cancel(raw) {
            return ConfigTaskOutcome::Cancelled {
                message: self.cancelled_message(),
            };
        }

        if self.kind == ConfigTaskKind::Init {
            return self.submit_init(raw);
        }

        if self.kind == ConfigTaskKind::ModelPool {
            return self.submit_model_pool(raw);
        }

        match self.section {
            ConfigSection::ModelAssignment => self.submit_model_assignment(raw),
            ConfigSection::Language => self.submit_language(raw),
            ConfigSection::Soul => self.submit_soul(raw),
        }
    }

    fn display_init_answer(&self, value: &str) -> String {
        let panel = self.init_view_model().action_panel;
        match panel {
            InitActionPanel::TextInput { title, .. } if title == "Enter API Key" => {
                "API Key: configured".to_string()
            }
            InitActionPanel::TextInput { title, .. } => {
                format!("{}: {value}", localize_init_action_title(&title))
            }
            InitActionPanel::MultiChoice { title, .. } => {
                let count = split_values(value).len();
                format!(
                    "{}: {}",
                    localize_init_action_title(&title),
                    t!("workspace.init_task.feedback.selected_count", count = count)
                )
            }
            InitActionPanel::SingleChoice { title, options, .. }
            | InitActionPanel::Summary { title, options, .. } => {
                let choices = options
                    .into_iter()
                    .map(|option| ConfigChoice {
                        label: option.label,
                        value: option.value,
                    })
                    .collect::<Vec<_>>();
                let value = choice_value(value, &choices);
                let label = choices
                    .iter()
                    .find(|choice| choice.value == value)
                    .map(|choice| choice.label.as_str())
                    .unwrap_or(value.as_str());
                format!(
                    "{}: {}",
                    localize_init_action_title(&title),
                    localize_init_option_label(label, &value)
                )
            }
            InitActionPanel::None => t!("workspace.init_task.title").to_string(),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn message(&self) -> String {
        if self.kind == ConfigTaskKind::Init {
            return self.init_message();
        }

        if self.kind == ConfigTaskKind::ModelPool {
            return self.model_pool_message();
        }

        let mut lines = vec![self.title_line()];
        if let Some(notice) = &self.missing_notice {
            lines.push(notice.clone());
            lines.push(String::new());
        }

        let issues = self.validation_issues();
        if !issues.is_empty() {
            lines.push("Unfinished configuration items:".to_string());
            lines.extend(
                issues
                    .into_iter()
                    .map(|issue| format!("- {}", issue.message)),
            );
            lines.push(String::new());
        }

        lines.extend(match self.section {
            ConfigSection::ModelAssignment => self.assignment_lines(),
            ConfigSection::Language => {
                vec![t!("workspace.config_task.section.language_prompt").to_string()]
            }
            ConfigSection::Soul => {
                vec![t!("workspace.config_task.section.soul_prompt").to_string()]
            }
        });
        lines.join("\n")
    }

    fn input_state(&self) -> PromptInputState {
        if self.kind == ConfigTaskKind::Init {
            return self.init_input();
        }

        if self.kind == ConfigTaskKind::ModelPool {
            return self.model_pool_input();
        }
        match self.section {
            ConfigSection::ModelAssignment => self.assignment_input(),
            ConfigSection::Language => self.language_input(),
            ConfigSection::Soul => self.soul_input(),
        }
    }

    fn submit_model_assignment(&mut self, raw: &str) -> ConfigTaskOutcome {
        if let Some(role) = self.assignment_role {
            let value = choice_value(raw, &self.assignment_model_choices(role));
            if value == "back" {
                self.assignment_role = None;
                return ConfigTaskOutcome::Next {
                    accepted: "Back".to_string(),
                    prompt: self.prompt(),
                };
            }
            let Some(entry) = self.model_entry(&value).cloned() else {
                return ConfigTaskOutcome::Invalid {
                    message: t!("workspace.config_task.validation.choose_enabled_model")
                        .to_string(),
                    prompt: self.prompt(),
                };
            };
            self.assignment.set(role, entry.id.clone());
            self.sync_assignment_compat();
            self.assignment_role = None;
            self.dirty = true;
            return self.applied();
        }

        let value = choice_value(raw, &self.assignment_role_choices());
        if value == "save_config" {
            return self.save_config();
        }
        if value == "cancel" {
            return ConfigTaskOutcome::Cancelled {
                message: self.cancelled_message(),
            };
        }
        if value == "model_pool" {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.model_pool_use_model").to_string(),
                prompt: self.prompt(),
            };
        }
        let Some(role) = parse_assignment_role(&value) else {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.choose_role").to_string(),
                prompt: self.prompt(),
            };
        };
        if self.enabled_models().is_empty() {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.empty_pool_run_model").to_string(),
                prompt: self.prompt(),
            };
        }
        self.assignment_role = Some(role);
        ConfigTaskOutcome::Next {
            accepted: role.label().to_string(),
            prompt: self.prompt(),
        }
    }

    fn submit_language(&mut self, raw: &str) -> ConfigTaskOutcome {
        let value = choice_value(raw, &language_choices());
        if value == "save_config" {
            return self.save_config();
        }
        if value == "cancel" {
            return ConfigTaskOutcome::Cancelled {
                message: self.cancelled_message(),
            };
        }
        let Some(locale) = LOCALES
            .iter()
            .find(|(code, _)| *code == value)
            .map(|(code, _)| *code)
        else {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.choose_language").to_string(),
                prompt: self.prompt(),
            };
        };
        self.draft.ui.locale = locale.to_string();
        crate::set_locale(locale);
        self.dirty = true;
        self.applied()
    }

    fn submit_soul(&mut self, raw: &str) -> ConfigTaskOutcome {
        let value = choice_value(raw, &self.soul_choices_for_prompt());
        if value == "save_config" {
            return self.save_config();
        }
        if value == "cancel" {
            return ConfigTaskOutcome::Cancelled {
                message: self.cancelled_message(),
            };
        }
        if value.trim().is_empty() {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.choose_soul").to_string(),
                prompt: self.prompt(),
            };
        }
        if self.soul_choices.iter().any(|formula| formula.id == value) {
            let _ = crate::formula::activate_soul(&value);
        }
        self.draft.soul.role = SoulRole::new(value.clone());
        self.dirty = true;
        self.applied()
    }

    fn submit_init(&mut self, raw: &str) -> ConfigTaskOutcome {
        self.missing_notice = None;
        let previous_message_count = self.init_message_count();
        let fallback_accepted = self.display_init_answer(raw);
        let command = match self.init_command_from_input(raw) {
            Ok(command) => command,
            Err(message) => {
                return ConfigTaskOutcome::Invalid {
                    message,
                    prompt: self.prompt(),
                };
            }
        };
        match self.run_init_command_chain(command) {
            Ok(Some(outcome)) => outcome,
            Ok(None) => {
                if let Err(error) = self.persist_init_progress() {
                    return ConfigTaskOutcome::Invalid {
                        message: format!("Failed to persist initialization state: {error}"),
                        prompt: self.prompt(),
                    };
                }
                if let Some(message) = self.init_error_message() {
                    ConfigTaskOutcome::Invalid {
                        message,
                        prompt: self.prompt(),
                    }
                } else {
                    let accepted = self
                        .init_feedback_since(previous_message_count)
                        .unwrap_or(fallback_accepted);
                    ConfigTaskOutcome::Next {
                        accepted,
                        prompt: self.prompt(),
                    }
                }
            }
            Err(message) => ConfigTaskOutcome::Invalid {
                message,
                prompt: self.prompt(),
            },
        }
    }

    fn init_command_from_input(&mut self, raw: &str) -> Result<InitCommand, String> {
        let panel = self.init_view_model().action_panel;
        match panel {
            InitActionPanel::TextInput { .. } => Ok(self
                .init_wizard_mut()?
                .handle_event(InitEvent::TextInput(raw.to_string()))),
            InitActionPanel::MultiChoice { options, .. } => {
                let selected = split_values(raw).into_iter().collect::<HashSet<String>>();
                let indices = options
                    .iter()
                    .enumerate()
                    .filter_map(|(index, option)| selected.contains(&option.value).then_some(index))
                    .collect::<Vec<_>>();
                self.init_wizard_mut()?.context.selected_model_indices = indices;
                Ok(self.init_wizard_mut()?.handle_event(InitEvent::Confirm))
            }
            InitActionPanel::SingleChoice { options, .. }
            | InitActionPanel::Summary { options, .. } => {
                let choices = options
                    .iter()
                    .map(|option| ConfigChoice {
                        label: option.label.clone(),
                        value: option.value.clone(),
                    })
                    .collect::<Vec<_>>();
                let value = choice_value(raw, &choices);
                if value == "ignore_validation" {
                    self.init_ignore_validation = true;
                }
                let index = options
                    .iter()
                    .position(|option| option.value == value)
                    .unwrap_or(0);
                self.init_wizard_mut()?
                    .handle_event(InitEvent::Select(index));
                Ok(self.init_wizard_mut()?.handle_event(InitEvent::Confirm))
            }
            InitActionPanel::None => Ok(self.init_wizard_mut()?.handle_event(InitEvent::Confirm)),
        }
    }

    fn run_init_command_chain(
        &mut self,
        mut command: InitCommand,
    ) -> Result<Option<ConfigTaskOutcome>, String> {
        loop {
            match command {
                InitCommand::None => return Ok(None),
                InitCommand::Cancel => {
                    let _ = runtime_config::project_init::clear_init_state(&init_cwd());
                    return Ok(Some(ConfigTaskOutcome::Cancelled {
                        message: self.cancelled_message(),
                    }));
                }
                InitCommand::Complete => {
                    let _ = runtime_config::project_init::clear_init_state(&init_cwd());
                    return Ok(Some(self.save_config()));
                }
                other => {
                    let Some(event) = self.execute_init_command(other) else {
                        return Ok(None);
                    };
                    command = self.init_wizard_mut()?.handle_event(event);
                    self.persist_init_progress().map_err(|error| {
                        format!("Failed to persist initialization state: {error}")
                    })?;
                }
            }
        }
    }

    fn execute_init_command(&mut self, command: InitCommand) -> Option<InitEvent> {
        let cwd = init_cwd();
        match command {
            InitCommand::CheckWorkspace => Some(InitEvent::WorkspaceChecked(
                runtime_config::project_init::check_workspace(&cwd),
            )),
            InitCommand::CreateProjectDirs { reset } => {
                let result = if reset {
                    runtime_config::project_init::reset_project_defaults(&cwd)
                } else {
                    runtime_config::project_init::ensure_project_defaults(&cwd)
                };
                match result {
                    Ok(()) => {
                        if reset {
                            self.draft = Settings::default();
                            crate::set_locale(&self.draft.ui.locale);
                            self.assignment_role = None;
                            self.add_model = None;
                        }
                        self.providers = load_current_provider_config();
                        self.assignment = load_current_model_assignment(&self.providers);
                        if !reset {
                            seed_model_library_from_settings(&self.draft, &mut self.providers);
                            ensure_active_provider(&self.draft, &mut self.providers);
                        }
                        self.dirty = true;
                        self.refresh_init_wizard_context();
                        Some(InitEvent::ProjectCreated)
                    }
                    Err(error) => Some(InitEvent::AsyncFailed(format!(
                        "Failed to initialize .alius project structure: {error}"
                    ))),
                }
            }
            InitCommand::WriteLanguageConfig { locale } => {
                let locale = if locale == "system" {
                    system_locale_or_default()
                } else {
                    locale
                };
                if LOCALES.iter().any(|(code, _)| *code == locale) {
                    self.draft.ui.locale = locale.clone();
                    crate::set_locale(&locale);
                    self.dirty = true;
                    if let Ok(wizard) = self.init_wizard_mut() {
                        wizard.context.language = Some(locale);
                    }
                }
                None
            }
            InitCommand::FetchModels {
                provider,
                api_protocol,
                base_url,
                api_key,
            } => {
                let models =
                    self.fetch_models_for_init(&provider, api_protocol, &base_url, &api_key);
                if models.is_empty() {
                    Some(InitEvent::AsyncFailed(
                        t!("workspace.config_task.validation.fetch_failed").to_string(),
                    ))
                } else {
                    self.apply_init_api_credentials(&provider, api_protocol, &base_url, &api_key);
                    self.dirty = true;
                    Some(InitEvent::ModelsFetched(models))
                }
            }
            InitCommand::ImportModels(models) => {
                let ids = self.import_init_models(&models);
                if let Err(message) = self.persist_model_pool() {
                    return Some(InitEvent::AsyncFailed(message));
                }
                self.dirty = true;
                Some(InitEvent::ModelsImported(ids))
            }
            InitCommand::WriteModelAssignment {
                plan,
                execute,
                review,
            } => {
                self.assignment.set(ModelAssignmentRole::Plan, plan);
                self.assignment.set(ModelAssignmentRole::Execute, execute);
                self.assignment.set(ModelAssignmentRole::Review, review);
                self.sync_assignment_compat();
                self.dirty = true;
                self.refresh_init_wizard_context();
                None
            }
            InitCommand::WriteSoulConfig(soul) => {
                if let Err(error) = self.activate_or_install_soul_for_init(&soul) {
                    return Some(InitEvent::AsyncFailed(error));
                }
                self.draft.soul.role = SoulRole::new(soul.clone());
                self.dirty = true;
                self.refresh_init_wizard_context();
                Some(InitEvent::AsyncOk)
            }
            InitCommand::ResolveCapability => {
                match runtime_config::project_init::resolve_capability_lock(&cwd) {
                    Ok(()) => Some(InitEvent::CapabilityResolved),
                    Err(error) => Some(InitEvent::AsyncFailed(format!(
                        "Failed to resolve capability bundle: {error}"
                    ))),
                }
            }
            InitCommand::CreateWorkspaceFromTemplate => {
                match runtime_config::project_init::create_workspace_template(&cwd) {
                    Ok(()) => Some(InitEvent::WorkspaceCreated),
                    Err(error) => Some(InitEvent::AsyncFailed(format!(
                        "Failed to create workspace template: {error}"
                    ))),
                }
            }
            InitCommand::ValidateConfig => {
                let issues = self
                    .validation_issues()
                    .into_iter()
                    .map(init_issue_from_config_issue)
                    .collect::<Vec<_>>();
                if issues.is_empty() {
                    Some(InitEvent::ValidationPassed)
                } else {
                    Some(InitEvent::ValidationFailed(issues))
                }
            }
            InitCommand::None | InitCommand::Complete | InitCommand::Cancel => None,
        }
    }

    fn activate_or_install_soul_for_init(&self, soul: &str) -> Result<(), String> {
        if self.soul_choices.iter().any(|formula| formula.id == soul) {
            return crate::formula::activate_soul(soul)
                .map(|_| ())
                .map_err(|error| format!("Failed to activate SOUL '{soul}': {error}"));
        }

        crate::formula::install_and_activate_soul(soul)
            .map(|_| ())
            .map_err(|error| format!("Failed to install SOUL '{soul}': {error}"))
    }

    fn fetch_models_for_init(
        &self,
        provider: &str,
        api_protocol: InitApiProtocol,
        base_url: &str,
        api_key: &str,
    ) -> Vec<InitModelInfo> {
        let mut settings = self.draft.llm.clone();
        settings.provider = provider_type_for_key(provider);
        settings.provider_mode = match api_protocol {
            InitApiProtocol::OpenAi => Some(ProviderMode::OpenAICompatible),
            InitApiProtocol::Anthropic => Some(ProviderMode::Native),
        };
        settings.base_url = Some(base_url.to_string());
        settings.model = String::new();
        settings.api_key = Some(api_key.to_string());
        settings.api_key_env = None;
        LlmClient::new(settings)
            .map(|client| client.list_models_blocking(base_url, api_key))
            .unwrap_or_default()
            .into_iter()
            .map(|model| InitModelInfo {
                id: model_entry_id(provider, base_url, &model),
                display_name: model.clone(),
                provider: provider.to_string(),
                api_protocol,
                base_url: base_url.to_string(),
                model_name: model,
            })
            .collect()
    }

    fn apply_init_api_credentials(
        &mut self,
        provider: &str,
        api_protocol: InitApiProtocol,
        base_url: &str,
        api_key: &str,
    ) {
        self.draft.llm.provider = provider_type_for_key(provider);
        self.draft.llm.provider_mode = match api_protocol {
            InitApiProtocol::OpenAi => Some(ProviderMode::OpenAICompatible),
            InitApiProtocol::Anthropic => Some(ProviderMode::Native),
        };
        self.draft.llm.base_url = Some(base_url.to_string());
        if !api_key.trim().is_empty() {
            self.draft.llm.api_key = Some(api_key.to_string());
            self.draft.llm.api_key_env = None;
        }
    }

    fn import_init_models(&mut self, models: &[InitModelInfo]) -> Vec<String> {
        let mut ids = Vec::new();
        for model in models {
            ensure_provider_entry(&model.provider, &mut self.providers);
            if let Some(provider) = self.providers.providers.get_mut(&model.provider) {
                provider.enabled = true;
                provider.kind = match model.api_protocol {
                    InitApiProtocol::OpenAi => "openai-compatible",
                    InitApiProtocol::Anthropic => "anthropic",
                }
                .to_string();
                provider.base_url = model.base_url.clone();
            }
            let entry = ModelLibraryEntry {
                id: model.id.clone(),
                display_name: model.display_name.clone(),
                provider: model.provider.clone(),
                base_url: model.base_url.clone(),
                model_name: model.model_name.clone(),
                reasoning_note: ReasoningNote::Standard,
                enabled: true,
            };
            if let Some(existing) = self
                .providers
                .model_library
                .models
                .iter_mut()
                .find(|entry| entry.id == model.id)
            {
                *existing = entry;
            } else {
                self.providers.model_library.models.push(entry);
            }
            ids.push(model.id.clone());
        }
        self.refresh_init_wizard_context();
        ids
    }

    fn init_wizard_mut(&mut self) -> Result<&mut InitWizard, String> {
        self.init_wizard
            .as_mut()
            .ok_or_else(|| "Initialization state is not active.".to_string())
    }

    fn init_view_model(&self) -> InitViewModel {
        self.init_wizard
            .as_ref()
            .map(InitWizard::view_model)
            .unwrap_or_else(|| InitWizard::new(init_cwd()).view_model())
    }

    fn init_error_message(&self) -> Option<String> {
        let wizard = self.init_wizard.as_ref()?;
        if wizard.state != InitState::Error {
            return None;
        }
        wizard
            .context
            .error
            .as_ref()
            .map(|error| error.message.clone())
    }

    fn init_message_count(&self) -> usize {
        self.init_wizard
            .as_ref()
            .map(|wizard| wizard.context.message_log.len())
            .unwrap_or(0)
    }

    fn init_feedback_since(&self, offset: usize) -> Option<String> {
        let wizard = self.init_wizard.as_ref()?;
        let lines = wizard
            .context
            .message_log
            .iter()
            .skip(offset)
            .map(init_message_text)
            .collect::<Vec<_>>();
        (!lines.is_empty()).then(|| lines.join("\n"))
    }

    fn persist_init_progress(&self) -> anyhow::Result<()> {
        let Some(wizard) = &self.init_wizard else {
            return Ok(());
        };
        if matches!(wizard.state, InitState::Complete | InitState::Cancelled) {
            runtime_config::project_init::clear_init_state(&wizard.context.cwd)
        } else {
            runtime_config::project_init::save_init_state(&wizard.context.cwd, wizard)
        }
    }

    fn persist_model_pool(&self) -> Result<(), String> {
        let cwd = init_cwd();
        let root = runtime_config::project_init::project_root_for_init(&cwd);
        let path = root.join(".alius/config/providers.toml");
        runtime_config::loaders::save_providers(&path, &self.providers)
            .map_err(|error| format!("Failed to save model pool: {error}"))
    }

    fn restore_existing_model_pool_if_current_empty(&mut self) {
        if !self.providers.model_library.models.is_empty() {
            return;
        }
        let Some(existing) = load_provider_file_for_current_workspace() else {
            return;
        };
        if existing.model_library.models.is_empty() {
            return;
        }

        for model in &existing.model_library.models {
            if let Some(provider) = existing.providers.get(&model.provider) {
                self.providers
                    .providers
                    .entry(model.provider.clone())
                    .or_insert_with(|| provider.clone());
            }
        }
        self.providers.model_library = existing.model_library;
    }

    fn refresh_init_wizard_context(&mut self) {
        let Some(wizard) = &mut self.init_wizard else {
            return;
        };
        refresh_init_context(
            &mut wizard.context,
            &self.draft,
            &self.providers,
            &self.assignment,
            &self.soul_choices,
        );
    }

    fn submit_model_pool(&mut self, raw: &str) -> ConfigTaskOutcome {
        if self.add_model.is_some() {
            return self.submit_add_model(raw);
        }

        match self.pool_mode {
            ModelPoolMode::Main => self.submit_model_pool_main(raw),
            ModelPoolMode::ViewSelect => self.submit_model_view_select(raw),
            ModelPoolMode::ViewDetail => {
                self.pool_mode = ModelPoolMode::ViewSelect;
                ConfigTaskOutcome::Next {
                    accepted: "Back".to_string(),
                    prompt: self.prompt(),
                }
            }
            ModelPoolMode::DeleteSelect => self.submit_model_delete_select(raw),
            ModelPoolMode::DeleteConfirm => self.submit_model_delete_confirm(raw),
        }
    }

    fn submit_model_pool_main(&mut self, raw: &str) -> ConfigTaskOutcome {
        let value = choice_value(raw, &self.model_pool_choices());
        match value.as_str() {
            "add_model" => {
                self.start_add_model();
                ConfigTaskOutcome::Next {
                    accepted: "Add Model".to_string(),
                    prompt: self.prompt(),
                }
            }
            "view_model" => {
                self.pool_mode = ModelPoolMode::ViewSelect;
                ConfigTaskOutcome::Next {
                    accepted: "View Model".to_string(),
                    prompt: self.prompt(),
                }
            }
            "delete_model" => {
                self.pool_mode = ModelPoolMode::DeleteSelect;
                ConfigTaskOutcome::Next {
                    accepted: "Delete Model".to_string(),
                    prompt: self.prompt(),
                }
            }
            "save_config" => self.save_config(),
            "cancel" => ConfigTaskOutcome::Cancelled {
                message: self.cancelled_message(),
            },
            _ => ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.choose_pool_action").to_string(),
                prompt: self.prompt(),
            },
        }
    }

    fn submit_model_view_select(&mut self, raw: &str) -> ConfigTaskOutcome {
        let value = choice_value(raw, &with_back(self.enabled_model_choices()));
        if value == "back" {
            self.pool_mode = ModelPoolMode::Main;
            return ConfigTaskOutcome::Next {
                accepted: "Back".to_string(),
                prompt: self.prompt(),
            };
        }
        if self.model_entry(&value).is_some() {
            self.selected_view_model = Some(value.clone());
            self.pool_mode = ModelPoolMode::ViewDetail;
            return ConfigTaskOutcome::Next {
                accepted: format!("Model: {}", self.model_label(&value)),
                prompt: self.prompt(),
            };
        }
        ConfigTaskOutcome::Invalid {
            message: t!("workspace.config_task.validation.choose_model_to_view").to_string(),
            prompt: self.prompt(),
        }
    }

    fn submit_model_delete_select(&mut self, raw: &str) -> ConfigTaskOutcome {
        let value = choice_value(raw, &with_back(self.enabled_model_choices()));
        if value == "back" {
            self.pool_mode = ModelPoolMode::Main;
            return ConfigTaskOutcome::Next {
                accepted: "Back".to_string(),
                prompt: self.prompt(),
            };
        }
        let Some(entry) = self.model_entry(&value).cloned() else {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.choose_model_to_delete").to_string(),
                prompt: self.prompt(),
            };
        };
        let refs = self.assignment.referenced_by(&entry.id);
        if let Some(role) = refs.first() {
            return ConfigTaskOutcome::Invalid {
                message: format!(
                    "{} is currently used by {}. Change it in /config before deleting.",
                    entry.display_name,
                    role.label()
                ),
                prompt: self.prompt(),
            };
        }
        self.pending_delete_model = Some(entry.id.clone());
        self.pool_mode = ModelPoolMode::DeleteConfirm;
        ConfigTaskOutcome::Next {
            accepted: format!("Delete: {}", model_entry_label(&entry)),
            prompt: self.prompt(),
        }
    }

    fn submit_model_delete_confirm(&mut self, raw: &str) -> ConfigTaskOutcome {
        let value = choice_value(raw, &delete_confirm_choices());
        if value == "cancel_delete" {
            self.pool_mode = ModelPoolMode::DeleteSelect;
            return ConfigTaskOutcome::Next {
                accepted: "Cancel delete".to_string(),
                prompt: self.prompt(),
            };
        }
        if value != "confirm_delete" {
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.choose_delete_action").to_string(),
                prompt: self.prompt(),
            };
        }

        let id = self.pending_delete_model.take().unwrap_or_default();
        if id.is_empty() {
            self.pool_mode = ModelPoolMode::DeleteSelect;
            return ConfigTaskOutcome::Invalid {
                message: t!("workspace.config_task.validation.no_model_selected").to_string(),
                prompt: self.prompt(),
            };
        }
        let before = self.providers.model_library.models.len();
        self.providers
            .model_library
            .models
            .retain(|entry| entry.id != id);
        self.pool_mode = ModelPoolMode::Main;
        self.dirty = self.providers.model_library.models.len() != before;
        if self.dirty {
            if let Err(message) = self.persist_model_pool() {
                return ConfigTaskOutcome::Invalid {
                    message,
                    prompt: self.prompt(),
                };
            }
        }
        ConfigTaskOutcome::Next {
            accepted: "Model deleted".to_string(),
            prompt: self.prompt(),
        }
    }

    fn submit_add_model(&mut self, raw: &str) -> ConfigTaskOutcome {
        let Some(mut add) = self.add_model.take() else {
            return ConfigTaskOutcome::Invalid {
                message: "No add-model flow is active.".to_string(),
                prompt: self.prompt(),
            };
        };

        match add.step {
            AddModelStep::Provider => {
                let provider = choice_value(raw, &provider_choices());
                ensure_provider_entry(&provider, &mut self.providers);
                add.provider = provider.clone();
                add.api_protocol = default_api_protocol_for_provider(&provider);
                add.base_url = default_base_url_for_provider_protocol(&provider, add.api_protocol);
                add.step = AddModelStep::ApiProtocol;
                self.add_model = Some(add);
                ConfigTaskOutcome::Next {
                    accepted: format!("Provider: {provider}"),
                    prompt: self.prompt(),
                }
            }
            AddModelStep::ApiProtocol => {
                let protocol = choice_value(raw, &api_protocol_choices());
                let Some(api_protocol) = parse_api_protocol(&protocol) else {
                    self.add_model = Some(add);
                    return ConfigTaskOutcome::Invalid {
                        message: t!("workspace.config_task.validation.choose_api_protocol")
                            .to_string(),
                        prompt: self.prompt(),
                    };
                };
                add.api_protocol = api_protocol;
                add.base_url = default_base_url_for_provider_protocol(&add.provider, api_protocol);
                add.step = AddModelStep::BaseUrl;
                self.add_model = Some(add);
                ConfigTaskOutcome::Next {
                    accepted: format!("API: {}", api_protocol.label()),
                    prompt: self.prompt(),
                }
            }
            AddModelStep::BaseUrl => {
                let base_url = choice_value(raw, &add_base_url_choices(&add));
                if !valid_url(&base_url) {
                    self.add_model = Some(add);
                    return ConfigTaskOutcome::Invalid {
                        message: t!("workspace.config_task.validation.base_url_format").to_string(),
                        prompt: self.prompt(),
                    };
                }
                add.base_url = base_url.clone();
                add.api_protocol = api_protocol_for_base_url(&base_url);
                add.step = AddModelStep::ApiKey;
                self.add_model = Some(add);
                ConfigTaskOutcome::Next {
                    accepted: format!("Base URL: {base_url}"),
                    prompt: self.prompt(),
                }
            }
            AddModelStep::ApiKey => {
                let value = raw.trim();
                add.api_key = value.to_string();
                if !value.is_empty() {
                    self.draft.llm.api_key = Some(value.to_string());
                    self.draft.llm.api_key_env = None;
                }
                let models = self.fetch_models_for_add(&add);
                if models.is_empty() {
                    add.fetch_failed = true;
                    add.step = AddModelStep::ApiKey;
                    self.add_model = Some(add);
                    ConfigTaskOutcome::Invalid {
                        message: t!("workspace.config_task.validation.fetch_failed").to_string(),
                        prompt: self.prompt(),
                    }
                } else {
                    add.fetch_failed = false;
                    add.models = models;
                    add.step = AddModelStep::SelectModels;
                    self.add_model = Some(add);
                    ConfigTaskOutcome::Next {
                        accepted: "Model list fetched".to_string(),
                        prompt: self.prompt(),
                    }
                }
            }
            AddModelStep::SelectModels => {
                let selected = split_values(raw);
                if selected.is_empty() {
                    self.add_model = Some(add);
                    return ConfigTaskOutcome::Invalid {
                        message: t!("workspace.config_task.validation.select_one_model")
                            .to_string(),
                        prompt: self.prompt(),
                    };
                }
                add.selected_models = selected;
                let added = self.save_added_models(&add);
                self.add_model = None;
                self.dirty = true;
                if self.kind == ConfigTaskKind::ModelPool {
                    if let Err(message) = self.persist_model_pool() {
                        return ConfigTaskOutcome::Invalid {
                            message,
                            prompt: self.prompt(),
                        };
                    }
                }
                ConfigTaskOutcome::Next {
                    accepted: format!("Added {} model(s) to the model pool.", added),
                    prompt: self.prompt(),
                }
            }
        }
    }

    fn save_config(&mut self) -> ConfigTaskOutcome {
        if self.kind != ConfigTaskKind::ModelPool {
            self.restore_existing_model_pool_if_current_empty();
            let allow_incomplete_init =
                self.kind == ConfigTaskKind::Init && self.init_ignore_validation;
            if !allow_incomplete_init {
                if let Some(issue) = self.first_issue() {
                    self.jump_to_issue(issue.clone());
                    return ConfigTaskOutcome::Invalid {
                        message: issue.message,
                        prompt: self.prompt(),
                    };
                }
            }
        }
        self.sync_assignment_compat();
        ConfigTaskOutcome::Saved {
            settings: self.draft.clone(),
            providers: Box::new(self.providers.clone()),
            assignment: Box::new(self.assignment.clone()),
            message: self.saved_message(),
        }
    }

    /// Build an `Applied` outcome: the selection is final and should be persisted
    /// immediately, but the task stays open for further edits. No validation gate
    /// (partial configs are allowed mid-editing).
    fn applied(&mut self) -> ConfigTaskOutcome {
        self.sync_assignment_compat();
        self.dirty = false;
        ConfigTaskOutcome::Applied {
            settings: self.draft.clone(),
            providers: Box::new(self.providers.clone()),
            assignment: Box::new(self.assignment.clone()),
            prompt: self.prompt(),
        }
    }

    fn start_add_model(&mut self) {
        let provider = provider_name(&self.draft.llm.provider).to_string();
        let base_url =
            provider_base_url(&self.providers, &provider).unwrap_or_else(|| self.draft.base_url());
        self.add_model = Some(AddModelState {
            step: AddModelStep::Provider,
            provider,
            api_protocol: ApiProtocol::OpenAi,
            base_url,
            api_key: String::new(),
            models: Vec::new(),
            selected_models: Vec::new(),
            fetch_failed: false,
        });
    }

    fn save_added_models(&mut self, add: &AddModelState) -> usize {
        ensure_provider_entry(&add.provider, &mut self.providers);
        if let Some(provider) = self.providers.providers.get_mut(&add.provider) {
            provider.enabled = true;
            provider.kind = provider_kind_for_base_url(&add.base_url).to_string();
            provider.base_url = add.base_url.clone();
        }
        let mut added = 0;
        for model in &add.selected_models {
            let id = model_entry_id(&add.provider, &add.base_url, model);
            let entry = ModelLibraryEntry {
                id: id.clone(),
                display_name: model.clone(),
                provider: add.provider.clone(),
                base_url: add.base_url.clone(),
                model_name: model.clone(),
                reasoning_note: ReasoningNote::Standard,
                enabled: true,
            };
            if let Some(existing) = self
                .providers
                .model_library
                .models
                .iter_mut()
                .find(|entry| entry.id == id)
            {
                *existing = entry;
            } else {
                self.providers.model_library.models.push(entry);
            }
            added += 1;
        }
        added
    }

    fn fetch_models_for_add(&self, add: &AddModelState) -> Vec<String> {
        let mut settings = self.draft.llm.clone();
        settings.provider = provider_type_for_key(&add.provider);
        settings.provider_mode = provider_mode_for_base_url(&add.base_url);
        settings.base_url = Some(add.base_url.clone());
        settings.model = String::new();
        if !add.api_key.trim().is_empty() {
            settings.api_key = Some(add.api_key.clone());
            settings.api_key_env = None;
        }
        let Some(api_key) = settings.get_api_key() else {
            return Vec::new();
        };
        LlmClient::new(settings)
            .map(|client| client.list_models_blocking(&add.base_url, &api_key))
            .unwrap_or_default()
    }

    fn sync_assignment_compat(&mut self) {
        for role in ModelAssignmentRole::all() {
            let model_id = self.assignment.get(role).trim();
            if model_id.is_empty() {
                continue;
            }
            let Some(entry) = self.model_entry(model_id).cloned() else {
                continue;
            };
            let tier = TierConfig {
                description: role.label().to_string(),
                provider: entry.provider.clone(),
                model: entry.model_name.clone(),
            };
            match role {
                ModelAssignmentRole::Plan => self.providers.tiers.light = tier,
                ModelAssignmentRole::Execute => {
                    self.providers.tiers.medium = tier;
                    self.draft.llm.provider = provider_type_for_key(&entry.provider);
                    self.draft.llm.provider_mode = provider_mode_for_base_url(&entry.base_url);
                    self.draft.llm.base_url = Some(entry.base_url.clone());
                    self.draft.llm.model = entry.model_name.clone();
                }
                ModelAssignmentRole::Review => {
                    self.providers.tiers.high = tier;
                    self.draft.llm.review_model = Some(entry.model_name.clone());
                }
            }
        }
    }

    fn first_issue(&self) -> Option<ConfigIssue> {
        self.validation_issues().into_iter().next()
    }

    fn validation_issues(&self) -> Vec<ConfigIssue> {
        let mut issues = Vec::new();
        if self.enabled_models().is_empty() {
            issues.push(ConfigIssue {
                message: t!("workspace.config_task.validation.incomplete_no_model").to_string(),
                section: ConfigSection::ModelAssignment,
            });
        }
        for role in ModelAssignmentRole::all() {
            let model_id = self.assignment.get(role).trim();
            if model_id.is_empty() {
                issues.push(ConfigIssue {
                    message: t!(
                        "workspace.config_task.validation.role_not_configured",
                        role = role.label()
                    )
                    .to_string(),
                    section: ConfigSection::ModelAssignment,
                });
            } else if self.model_entry(model_id).is_none() {
                issues.push(ConfigIssue {
                    message: t!(
                        "workspace.config_task.validation.role_references_missing",
                        role = role.label(),
                        model = model_id
                    )
                    .to_string(),
                    section: ConfigSection::ModelAssignment,
                });
            }
        }
        if self.draft.soul.role.as_str().trim().is_empty() {
            issues.push(ConfigIssue {
                message: t!("workspace.config_task.validation.incomplete_no_soul").to_string(),
                section: ConfigSection::Soul,
            });
        }
        if self.draft.llm.get_api_key().is_none() {
            issues.push(ConfigIssue {
                message: t!("workspace.config_task.validation.incomplete_no_api_key").to_string(),
                section: ConfigSection::ModelAssignment,
            });
        }
        if self.draft.ui.locale.trim().is_empty() {
            issues.push(ConfigIssue {
                message: t!("workspace.config_task.validation.incomplete_no_language").to_string(),
                section: ConfigSection::Language,
            });
        }
        issues
    }

    fn jump_to_issue(&mut self, issue: ConfigIssue) {
        self.section = issue.section;
        self.missing_notice = Some(issue.message);
    }

    fn jump_to_first_missing(&mut self) {
        if let Some(issue) = self.first_issue() {
            self.jump_to_issue(issue);
        }
    }

    fn enabled_models(&self) -> Vec<ModelLibraryEntry> {
        self.providers
            .model_library
            .models
            .iter()
            .filter(|entry| entry.enabled)
            .cloned()
            .collect()
    }

    fn model_entry(&self, id: &str) -> Option<&ModelLibraryEntry> {
        self.providers
            .model_library
            .models
            .iter()
            .find(|entry| entry.id == id && entry.enabled)
    }

    fn model_label(&self, id: &str) -> String {
        self.model_entry(id)
            .map(model_entry_label)
            .unwrap_or_else(|| id.to_string())
    }

    fn title_line(&self) -> String {
        match self.kind {
            ConfigTaskKind::Config => self.tabs_title(),
            ConfigTaskKind::Init => format!("Project Initialization\n{}", self.tabs_title()),
            ConfigTaskKind::ModelPool => "Model Pool".to_string(),
        }
    }

    fn tabs_title(&self) -> String {
        ConfigSection::all()
            .into_iter()
            .map(|section| {
                let label = section.display_label();
                if section == self.section {
                    format!("[{label}]")
                } else {
                    label
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
    }

    fn assignment_lines(&self) -> Vec<String> {
        let mut lines = vec![
            t!("workspace.config_task.section.models_prompt").to_string(),
            String::new(),
            t!("workspace.config_task.current_assignment").to_string(),
        ];
        for role in ModelAssignmentRole::all() {
            lines.push(format!(
                "{}: {}",
                role.label(),
                self.model_label_or_unconfigured(self.assignment.get(role))
            ));
        }
        if self.enabled_models().is_empty() {
            lines.push(String::new());
            lines.push(t!("workspace.config_task.empty_pool_hint").to_string());
        }
        lines
    }

    fn model_pool_message(&self) -> String {
        let mut lines = vec![
            "Model pool stores the models available to this project.".to_string(),
            "Plan, Execute, and Review assignment is configured in /config.".to_string(),
            String::new(),
        ];
        if let Some(add) = &self.add_model {
            lines.push("Add Model".to_string());
            lines.push(add.step.question());
            if add.fetch_failed {
                lines.push(t!("workspace.config_task.validation.fetch_failed").to_string());
            }
            return lines.join("\n");
        }
        lines.push("Current model pool:".to_string());
        let models = self.enabled_models();
        if models.is_empty() {
            lines.push("- empty".to_string());
        } else {
            for entry in &models {
                lines.push(format!("- {}", model_entry_label(entry)));
            }
        }
        if self.pool_mode == ModelPoolMode::ViewDetail {
            if let Some(entry) = self
                .selected_view_model
                .as_deref()
                .and_then(|id| self.model_entry(id))
            {
                lines.push(String::new());
                lines.push("Model detail:".to_string());
                lines.push(format!("ID: {}", entry.id));
                lines.push(format!("Provider: {}", entry.provider));
                lines.push(format!("Base URL: {}", entry.base_url));
                lines.push(format!("Model Name: {}", entry.model_name));
                lines.push("API Key: configured".to_string());
                lines.push(format!(
                    "Status: {}",
                    if entry.enabled { "enabled" } else { "disabled" }
                ));
            }
        }
        lines.join("\n")
    }

    fn init_message(&self) -> String {
        let vm = self.init_view_model();
        let mut lines = vec![
            localize_init_header(&vm.header),
            format!(
                "{}: {}",
                t!("workspace.init_task.state"),
                localize_init_scope_title(&vm.scope_title)
            ),
            String::new(),
        ];
        lines.push(t!("workspace.init_task.configuration_check").to_string());
        lines.extend(vm.check_items.into_iter().map(|item| {
            let marker = match item.status {
                CheckItemStatus::Done => "✓",
                CheckItemStatus::Active => "●",
                CheckItemStatus::Pending => "○",
                CheckItemStatus::Warning => "!",
                CheckItemStatus::Failed => "!",
            };
            format!(
                "{marker} {}. {}",
                item.index,
                localize_init_check_title(&item.title)
            )
        }));
        lines.push(String::new());

        lines.extend(vm.messages.into_iter().map(|message| {
            format!(
                "{} {}",
                message.marker,
                localize_init_message(&message.text)
            )
        }));

        if let Some(error) = self.init_error_message() {
            lines.push(format!("! {error}"));
        }

        match vm.action_panel {
            InitActionPanel::Summary { lines: summary, .. } => {
                lines.push(String::new());
                lines.extend(
                    summary
                        .into_iter()
                        .map(|line| localize_init_summary_line(&line)),
                );
            }
            InitActionPanel::MultiChoice { title, options, .. } => {
                lines.push(String::new());
                lines.push(localize_init_action_title(&title));
                lines.extend(options.into_iter().map(|option| {
                    format!(
                        "{} {}",
                        if option.selected { "[x]" } else { "[ ]" },
                        localize_init_option_label(&option.label, &option.value)
                    )
                }));
            }
            _ => {}
        }
        lines.join("\n")
    }

    fn init_flow_message(&self) -> String {
        let vm = self.init_view_model();
        let mut lines = vec![
            localize_init_header(&vm.header),
            format!(
                "{}: {}",
                t!("workspace.init_task.state"),
                localize_init_scope_title(&vm.scope_title)
            ),
            String::new(),
        ];
        lines.push(t!("workspace.init_task.configuration_check").to_string());
        lines.extend(vm.check_items.into_iter().map(|item| {
            let marker = match item.status {
                CheckItemStatus::Done => "✓",
                CheckItemStatus::Active => "●",
                CheckItemStatus::Pending => "○",
                CheckItemStatus::Warning => "!",
                CheckItemStatus::Failed => "!",
            };
            format!(
                "{marker} {}. {}",
                item.index,
                localize_init_check_title(&item.title)
            )
        }));

        match vm.action_panel {
            InitActionPanel::Summary { lines: summary, .. } => {
                lines.push(String::new());
                lines.extend(
                    summary
                        .into_iter()
                        .map(|line| localize_init_summary_line(&line)),
                );
            }
            InitActionPanel::MultiChoice { title, options, .. } => {
                lines.push(String::new());
                lines.push(localize_init_action_title(&title));
                lines.extend(options.into_iter().map(|option| {
                    format!(
                        "{} {}",
                        if option.selected { "[x]" } else { "[ ]" },
                        localize_init_option_label(&option.label, &option.value)
                    )
                }));
            }
            _ => {}
        }
        lines.join("\n")
    }

    fn init_input(&self) -> PromptInputState {
        let vm = self.init_view_model();
        let scope = vm.scope_title.clone();
        match vm.action_panel {
            InitActionPanel::SingleChoice {
                title,
                options,
                selected,
                hint,
            }
            | InitActionPanel::Summary {
                title,
                options,
                selected,
                hint,
                ..
            } => {
                let mut state = prompt(
                    localize_init_action_title(&title),
                    PromptInputKind::SingleSelect,
                    options
                        .into_iter()
                        .map(|option| ConfigChoice {
                            label: localize_init_option_label(&option.label, &option.value),
                            value: option.value,
                        })
                        .collect(),
                    localize_init_help(&hint),
                )
                .with_scope_title(scope);
                state.highlighted = selected.min(state.choices.len().saturating_sub(1));
                state
            }
            InitActionPanel::MultiChoice {
                title,
                options,
                highlighted,
                hint,
            } => {
                let mut state = PromptInputState::new(
                    localize_init_action_title(&title),
                    PromptInputKind::MultiSelect,
                    options
                        .into_iter()
                        .map(|option| {
                            let mut choice = PromptChoice::new(
                                localize_init_option_label(&option.label, &option.value),
                                option.value,
                            );
                            choice.selected = option.selected;
                            choice
                        })
                        .collect(),
                    localize_init_help(&hint),
                )
                .with_scope_title(scope);
                state.highlighted = highlighted.min(state.choices.len().saturating_sub(1));
                state
            }
            InitActionPanel::TextInput {
                title,
                value,
                placeholder,
                hint,
                masked,
            } => prompt(
                localize_init_action_title(&title),
                PromptInputKind::Text { masked },
                Vec::new(),
                localize_init_help(&hint),
            )
            .with_scope_title(scope)
            .with_placeholder(localize_init_placeholder(&placeholder))
            .with_input_value(value),
            InitActionPanel::None => prompt(
                t!("workspace.init_task.title").to_string(),
                PromptInputKind::SingleSelect,
                Vec::new(),
                t!("workspace.init_task.help.working").to_string(),
            )
            .with_scope_title(scope),
        }
    }

    fn model_label_or_unconfigured(&self, id: &str) -> String {
        if id.trim().is_empty() {
            t!("workspace.config_task.not_configured").to_string()
        } else {
            self.model_label(id)
        }
    }

    fn assignment_input(&self) -> PromptInputState {
        if let Some(role) = self.assignment_role {
            return prompt(
                role.label(),
                PromptInputKind::SingleSelect,
                self.assignment_model_choices(role),
                t!("workspace.config_task.hint.assign_role").to_string(),
            )
            .with_scope_title(self.active_scope_title())
            .with_highlighted_value(self.assignment.get(role));
        }
        prompt(
            "Model Assignment",
            PromptInputKind::SingleSelect,
            self.assignment_role_choices(),
            self.help(),
        )
        .with_scope_title(self.active_scope_title())
    }

    fn language_input(&self) -> PromptInputState {
        prompt(
            "Language",
            PromptInputKind::SingleSelect,
            with_config_actions(language_choices()),
            self.help(),
        )
        .with_scope_title(self.active_scope_title())
        .with_highlighted_value(&self.draft.ui.locale)
    }

    fn soul_input(&self) -> PromptInputState {
        prompt(
            "Role",
            PromptInputKind::SingleSelect,
            with_config_actions(self.soul_choices_for_prompt()),
            self.help(),
        )
        .with_scope_title(self.active_scope_title())
        .with_highlighted_value(current_soul(&self.draft).as_str())
    }

    fn model_pool_input(&self) -> PromptInputState {
        if let Some(add) = &self.add_model {
            return self.add_model_input(add);
        }
        match self.pool_mode {
            ModelPoolMode::Main => prompt(
                "model-pool",
                PromptInputKind::SingleSelect,
                self.model_pool_choices(),
                t!("workspace.config_task.hint.move_options").to_string(),
            )
            .with_scope_title(self.active_scope_title()),
            ModelPoolMode::ViewSelect => prompt(
                "View Model",
                PromptInputKind::SingleSelect,
                with_back(self.enabled_model_choices()),
                t!("workspace.config_task.hint.view_detail").to_string(),
            )
            .with_scope_title(self.active_scope_title()),
            ModelPoolMode::ViewDetail => prompt(
                "Model Detail",
                PromptInputKind::SingleSelect,
                vec![choice("Back", "back")],
                t!("workspace.config_task.hint.back_only").to_string(),
            )
            .with_scope_title(self.active_scope_title()),
            ModelPoolMode::DeleteSelect => prompt(
                "Delete Model",
                PromptInputKind::SingleSelect,
                with_back(self.enabled_model_choices()),
                t!("workspace.config_task.hint.delete_select").to_string(),
            )
            .with_scope_title(self.active_scope_title()),
            ModelPoolMode::DeleteConfirm => prompt(
                "Confirm Delete",
                PromptInputKind::SingleSelect,
                delete_confirm_choices(),
                t!("workspace.config_task.hint.delete_confirm").to_string(),
            )
            .with_scope_title(self.active_scope_title()),
        }
    }

    fn add_model_input(&self, add: &AddModelState) -> PromptInputState {
        let (kind, choices, placeholder, value) = match add.step {
            AddModelStep::Provider => (
                PromptInputKind::SingleSelect,
                provider_choices(),
                String::new(),
                add.provider.clone(),
            ),
            AddModelStep::ApiProtocol => (
                PromptInputKind::SingleSelect,
                api_protocol_choices(),
                String::new(),
                add.api_protocol.value().to_string(),
            ),
            AddModelStep::BaseUrl => (
                PromptInputKind::SingleSelectWithInput { masked: false },
                add_base_url_choices(add),
                "Enter Base URL".to_string(),
                add.base_url.clone(),
            ),
            AddModelStep::ApiKey => (
                PromptInputKind::Text { masked: false },
                Vec::new(),
                "Enter API Key".to_string(),
                add.api_key.clone(),
            ),
            AddModelStep::SelectModels => (
                PromptInputKind::MultiSelect,
                add.models
                    .iter()
                    .map(|model| ConfigChoice {
                        label: model.clone(),
                        value: model.clone(),
                    })
                    .collect(),
                String::new(),
                String::new(),
            ),
        };

        prompt(
            add.step.label(),
            kind,
            choices,
            t!("workspace.config_task.hint.add_step").to_string(),
        )
        .with_scope_title(self.active_scope_title())
        .with_placeholder(placeholder)
        .with_input_value(value)
    }

    fn active_scope_title(&self) -> String {
        match self.kind {
            ConfigTaskKind::Config => self.section.label().to_string(),
            ConfigTaskKind::Init => self.init_view_model().scope_title,
            ConfigTaskKind::ModelPool => self.model_pool_scope_title().to_string(),
        }
    }

    fn side_panel(&self) -> Option<ConfigSidePanel> {
        (self.kind == ConfigTaskKind::Init).then(|| ConfigSidePanel {
            title: t!("workspace.init_task.title").to_string(),
            content: self.init_flow_message(),
        })
    }

    fn model_pool_scope_title(&self) -> &'static str {
        if self.add_model.is_some() {
            return "model-pool-add";
        }
        match self.pool_mode {
            ModelPoolMode::Main => "model-pool",
            ModelPoolMode::ViewSelect | ModelPoolMode::ViewDetail => "model-pool-view",
            ModelPoolMode::DeleteSelect | ModelPoolMode::DeleteConfirm => "model-pool-delete",
        }
    }

    fn help(&self) -> String {
        t!("workspace.config_task.help").to_string()
    }

    fn assignment_role_choices(&self) -> Vec<ConfigChoice> {
        let mut choices = ModelAssignmentRole::all()
            .into_iter()
            .map(|role| {
                let current = self.model_label_or_unconfigured(self.assignment.get(role));
                ConfigChoice {
                    label: format!("{}    {}", role.label(), current),
                    value: role_value(role).to_string(),
                }
            })
            .collect::<Vec<_>>();
        if self.enabled_models().is_empty() {
            choices.push(choice(
                t!("workspace.config_task.action.empty_pool_first"),
                "model_pool",
            ));
        }
        choices.push(choice(
            t!("workspace.config_task.action.save"),
            "save_config",
        ));
        choices.push(choice(t!("workspace.config_task.action.cancel"), "cancel"));
        choices
    }

    fn assignment_model_choices(&self, _role: ModelAssignmentRole) -> Vec<ConfigChoice> {
        with_back(self.enabled_model_choices())
    }

    fn enabled_model_choices(&self) -> Vec<ConfigChoice> {
        self.enabled_models()
            .into_iter()
            .map(|entry| ConfigChoice {
                label: model_entry_label(&entry),
                value: entry.id,
            })
            .collect()
    }

    fn model_pool_choices(&self) -> Vec<ConfigChoice> {
        vec![
            choice(t!("workspace.config_task.action.add_model"), "add_model"),
            choice(t!("workspace.config_task.action.view_model"), "view_model"),
            choice(
                t!("workspace.config_task.action.delete_model"),
                "delete_model",
            ),
        ]
    }

    fn soul_choices_for_prompt(&self) -> Vec<ConfigChoice> {
        let current = current_soul(&self.draft);
        let mut choices = Vec::new();
        if !current.trim().is_empty() {
            choices.push(ConfigChoice {
                label: format!("Current: {current}"),
                value: current,
            });
        }
        choices.extend(self.soul_choices.iter().map(|formula| ConfigChoice {
            label: format!("{} - {}", formula.id, formula.description),
            value: formula.id.clone(),
        }));
        choices
    }

    fn saved_message(&self) -> String {
        match self.kind {
            ConfigTaskKind::Config => t!("workspace.config_task.saved").to_string(),
            ConfigTaskKind::Init => t!("workspace.init_task.saved").to_string(),
            ConfigTaskKind::ModelPool => {
                t!("workspace.config_task.validation.model_pool_saved").to_string()
            }
        }
    }

    fn cancelled_message(&self) -> String {
        match self.kind {
            ConfigTaskKind::Config => t!("workspace.config_task.cancelled").to_string(),
            ConfigTaskKind::Init => t!("workspace.init_task.cancelled").to_string(),
            ConfigTaskKind::ModelPool => {
                t!("workspace.config_task.validation.model_pool_cancelled").to_string()
            }
        }
    }
}

impl ConfigSection {
    pub fn all() -> [Self; 3] {
        [Self::ModelAssignment, Self::Language, Self::Soul]
    }

    fn label(self) -> &'static str {
        match self {
            Self::ModelAssignment => "configuration-models",
            Self::Language => "configuration-language",
            Self::Soul => "configuration-soul",
        }
    }

    pub fn display_label(self) -> String {
        match self {
            Self::ModelAssignment => t!("workspace.config_task.tab.models").to_string(),
            Self::Language => t!("workspace.config_task.tab.language").to_string(),
            Self::Soul => t!("workspace.config_task.tab.soul").to_string(),
        }
    }

    fn shift(self, reverse: bool) -> Self {
        let sections = Self::all();
        let current = sections
            .iter()
            .position(|section| *section == self)
            .unwrap_or(0);
        let next = if reverse {
            current.checked_sub(1).unwrap_or(sections.len() - 1)
        } else {
            (current + 1) % sections.len()
        };
        sections[next]
    }
}

impl AddModelStep {
    fn label(self) -> String {
        match self {
            Self::Provider => t!("workspace.config_task.add_model_step.provider_label").to_string(),
            Self::ApiProtocol => t!("workspace.config_task.add_model_step.api_label").to_string(),
            Self::BaseUrl => t!("workspace.config_task.add_model_step.base_url_label").to_string(),
            Self::ApiKey => t!("workspace.config_task.add_model_step.api_key_label").to_string(),
            Self::SelectModels => {
                t!("workspace.config_task.add_model_step.models_label").to_string()
            }
        }
    }

    fn question(self) -> String {
        match self {
            Self::Provider => t!("workspace.config_task.add_model_step.provider_q").to_string(),
            Self::ApiProtocol => t!("workspace.config_task.add_model_step.api_q").to_string(),
            Self::BaseUrl => t!("workspace.config_task.add_model_step.base_url_q").to_string(),
            Self::ApiKey => t!("workspace.config_task.add_model_step.api_key_q").to_string(),
            Self::SelectModels => t!("workspace.config_task.add_model_step.models_q").to_string(),
        }
    }
}

impl ApiProtocol {
    fn label(self) -> &'static str {
        match self {
            Self::OpenAi => "OpenAI API",
            Self::Anthropic => "Anthropic API",
        }
    }

    fn value(self) -> &'static str {
        match self {
            Self::OpenAi => "openai",
            Self::Anthropic => "anthropic",
        }
    }
}

fn prompt(
    title: impl Into<String>,
    kind: PromptInputKind,
    choices: Vec<ConfigChoice>,
    help: impl Into<String>,
) -> PromptInputState {
    PromptInputState::new(
        title,
        kind,
        choices
            .into_iter()
            .map(|choice| PromptChoice::new(choice.label, choice.value))
            .collect(),
        help,
    )
}

fn init_message_text(message: &InitMessage) -> String {
    match message {
        InitMessage::Info(text) => localize_init_message(text),
        InitMessage::Success(text) => localize_init_message(text),
        InitMessage::Warning(text) => {
            format!("{}: {}", t!("workspace.init_task.message.warning"), text)
        }
        InitMessage::Error(text) => {
            format!("{}: {}", t!("workspace.init_task.message.error"), text)
        }
        InitMessage::Running(text) => localize_init_message(text),
    }
}

fn load_soul_choices_for_kind(kind: ConfigTaskKind) -> Vec<FormulaDef> {
    let choices = crate::formula::list_installed_souls().unwrap_or_default();

    #[cfg(not(test))]
    if matches!(kind, ConfigTaskKind::Init) && choices.is_empty() {
        let _ = crate::formula::sync_all_souls();
        return crate::formula::list_installed_souls().unwrap_or_default();
    }

    let _ = kind;
    choices
}

fn localize_init_header(header: &str) -> String {
    if header.contains('✓') {
        t!("workspace.init_task.header.complete").to_string()
    } else {
        t!("workspace.init_task.header.incomplete").to_string()
    }
}

fn localize_init_scope_title(scope: &str) -> String {
    match scope {
        "init-start" => t!("workspace.init_task.scope.init_start").to_string(),
        "resume" => t!("workspace.init_task.scope.resume").to_string(),
        "check-workspace" => t!("workspace.init_task.scope.check_workspace").to_string(),
        "create-project" => t!("workspace.init_task.scope.create_project").to_string(),
        "select-language" => t!("workspace.init_task.scope.select_language").to_string(),
        "configure-model-pool" => t!("workspace.init_task.scope.configure_model_pool").to_string(),
        "configure-assignment" => t!("workspace.init_task.scope.configure_assignment").to_string(),
        "configure-soul" => t!("workspace.init_task.scope.configure_soul").to_string(),
        "resolve-capability" => t!("workspace.init_task.scope.resolve_capability").to_string(),
        "create-workspace" => t!("workspace.init_task.scope.create_workspace").to_string(),
        "validate" => t!("workspace.init_task.scope.validate").to_string(),
        "complete" => t!("workspace.init_task.scope.complete").to_string(),
        "error" => t!("workspace.init_task.scope.error").to_string(),
        "cancelled" => t!("workspace.init_task.scope.cancelled").to_string(),
        _ => scope.to_string(),
    }
}

fn localize_init_check_title(title: &str) -> String {
    match title {
        "Check workspace" => t!("workspace.init_task.check.workspace").to_string(),
        "Initialize .alius/" => t!("workspace.init_task.check.project").to_string(),
        "Choose language" => t!("workspace.init_task.check.language").to_string(),
        "Configure model pool" => t!("workspace.init_task.check.model_pool").to_string(),
        "Configure Plan/Execute/Review" => t!("workspace.init_task.check.assignment").to_string(),
        "Configure Role" | "Configure SOUL" => t!("workspace.init_task.check.soul").to_string(),
        "Resolve Capability" => t!("workspace.init_task.check.capability").to_string(),
        "Create workspace" => t!("workspace.init_task.check.workspace_template").to_string(),
        "Validate configuration" => t!("workspace.init_task.check.validation").to_string(),
        _ => title.to_string(),
    }
}

fn localize_init_action_title(title: &str) -> String {
    if let Some(count) = found_model_count(title) {
        return t!("workspace.init_task.action.found_models", count = count).to_string();
    }

    match title {
        "Start Initialization" => t!("workspace.init_task.action.start").to_string(),
        "Detected unfinished initialization" => t!("workspace.init_task.action.resume").to_string(),
        "Project structure" => t!("workspace.init_task.action.project").to_string(),
        "Choose interface language" => t!("workspace.init_task.action.language").to_string(),
        "Model pool" => t!("workspace.init_task.action.model_pool").to_string(),
        "Choose model provider and API" => {
            t!("workspace.init_task.action.provider_api").to_string()
        }
        "Enter Base URL" => t!("workspace.init_task.action.base_url").to_string(),
        "Enter API Key" => t!("workspace.init_task.action.api_key").to_string(),
        "Plan / Execute / Review" => t!("workspace.init_task.action.assignment").to_string(),
        "Plan Model" => t!("workspace.init_task.option.plan_model").to_string(),
        "Execute Model" => t!("workspace.init_task.option.execute_model").to_string(),
        "Review Model" => t!("workspace.init_task.option.review_model").to_string(),
        "Choose Role" | "Choose SOUL" => t!("workspace.init_task.action.soul").to_string(),
        "Resolve Capability" => t!("workspace.init_task.action.capability").to_string(),
        "Create workspace" => t!("workspace.init_task.action.workspace").to_string(),
        "Final validation" => t!("workspace.init_task.action.validation").to_string(),
        "Initialization complete" => t!("workspace.init_task.action.complete").to_string(),
        "Recover" => t!("workspace.init_task.action.recover").to_string(),
        "Cancelled" => t!("workspace.init_task.scope.cancelled").to_string(),
        "Initialization" => t!("workspace.init_task.title").to_string(),
        _ => title.to_string(),
    }
}

fn found_model_count(title: &str) -> Option<usize> {
    title
        .strip_prefix("Found ")
        .and_then(|value| value.strip_suffix(" model(s)"))
        .and_then(|value| value.parse::<usize>().ok())
}

fn localize_init_option_label(label: &str, value: &str) -> String {
    match value {
        "start" => t!("workspace.init_task.option.start").to_string(),
        "exit" | "cancel" => t!("workspace.init_task.option.exit").to_string(),
        "continue" => t!("workspace.init_task.option.continue_previous").to_string(),
        "restart" => t!("workspace.init_task.option.restart").to_string(),
        "continue_existing" => t!("workspace.init_task.option.continue_existing").to_string(),
        "reinitialize_project" => t!("workspace.init_task.option.reinitialize_project").to_string(),
        "zh-CN" => t!("workspace.init_task.option.zh_cn").to_string(),
        "en" => t!("workspace.init_task.option.en").to_string(),
        "ja" => t!("workspace.init_task.option.ja").to_string(),
        "system" => t!("workspace.init_task.option.system").to_string(),
        "back" => t!("common.back").to_string(),
        "add_model" if label == "Add Another Model" => {
            t!("workspace.init_task.option.add_another_model").to_string()
        }
        "add_model" => t!("workspace.init_task.option.add_model").to_string(),
        "configure_later" => t!("workspace.init_task.option.configure_later").to_string(),
        "continue_assignment" => t!("workspace.init_task.option.continue_assignment").to_string(),
        "plan" => assignment_option_label(
            t!("workspace.init_task.option.plan_model").as_ref(),
            assignment_status(label, "Plan Model"),
        ),
        "execute" => assignment_option_label(
            t!("workspace.init_task.option.execute_model").as_ref(),
            assignment_status(label, "Execute Model"),
        ),
        "review" => assignment_option_label(
            t!("workspace.init_task.option.review_model").as_ref(),
            assignment_status(label, "Review Model"),
        ),
        "continue_soul" => t!("workspace.init_task.option.continue_soul").to_string(),
        "resolve_capability" => t!("workspace.init_task.option.resolve_capability").to_string(),
        "skip_capability" | "skip" => t!("workspace.init_task.option.skip").to_string(),
        "create_workspace" => t!("workspace.init_task.option.create_workspace").to_string(),
        "run_validate" => t!("workspace.init_task.option.run_validation").to_string(),
        "fix_validation" => t!("workspace.init_task.option.fix_validation").to_string(),
        "ignore_validation" => t!("workspace.init_task.option.ignore_complete").to_string(),
        "enter_copilot" => t!("workspace.init_task.option.enter_copilot").to_string(),
        "enter_team" => t!("workspace.init_task.option.enter_team").to_string(),
        "view_config" => t!("workspace.init_task.option.view_config").to_string(),
        "retry" => t!("workspace.init_task.option.retry").to_string(),
        _ => localize_init_free_label(label),
    }
}

fn assignment_status<'a>(label: &'a str, prefix: &str) -> &'a str {
    label.strip_prefix(prefix).unwrap_or(label).trim()
}

fn assignment_option_label(role: &str, status: &str) -> String {
    format!("{role}    {}", localize_model_status(status))
}

fn localize_model_status(status: &str) -> String {
    if status == "not configured" {
        t!("common.not_configured").to_string()
    } else {
        status.to_string()
    }
}

fn localize_init_free_label(label: &str) -> String {
    if let Some(id) = label.strip_suffix(" - current") {
        return format!("{} - {}", id, t!("workspace.init_task.option.current"));
    }
    localize_model_status(label)
}

fn localize_init_summary_line(line: &str) -> String {
    if let Some(value) = line
        .strip_prefix("Role: ")
        .or_else(|| line.strip_prefix("SOUL: "))
    {
        format!(
            "{}: {}",
            t!("workspace.init_task.summary.soul"),
            localize_model_status(value)
        )
    } else if let Some(value) = line.strip_prefix("Plan: ") {
        format!(
            "{}: {}",
            t!("workspace.init_task.summary.plan"),
            localize_model_status(value)
        )
    } else if let Some(value) = line.strip_prefix("Execute: ") {
        format!(
            "{}: {}",
            t!("workspace.init_task.summary.execute"),
            localize_model_status(value)
        )
    } else if let Some(value) = line.strip_prefix("Review: ") {
        format!(
            "{}: {}",
            t!("workspace.init_task.summary.review"),
            localize_model_status(value)
        )
    } else {
        line.to_string()
    }
}

fn localize_init_help(hint: &str) -> String {
    match hint {
        "Up/Down choose. Enter confirms." => t!("workspace.init_task.help.choose").to_string(),
        "Up/Down choose. Enter confirms. Esc returns." => {
            t!("workspace.init_task.help.choose_esc").to_string()
        }
        "Up/Down choose. Enter assigns." => t!("workspace.init_task.help.assign").to_string(),
        "Space toggles. Enter imports selected. Esc returns." => {
            t!("workspace.init_task.help.multi").to_string()
        }
        "Enter confirms. Esc returns." => t!("workspace.init_task.help.text").to_string(),
        "Enter confirms." => t!("workspace.init_task.help.enter").to_string(),
        "Working." => t!("workspace.init_task.help.working").to_string(),
        _ => hint.to_string(),
    }
}

fn localize_init_placeholder(placeholder: &str) -> String {
    match placeholder {
        "API Key" => t!("common.api_key").to_string(),
        "https://api.example.com/v1" => placeholder.to_string(),
        _ => placeholder.to_string(),
    }
}

fn localize_init_message(text: &str) -> String {
    if let Some(locale) = text.strip_prefix("Language selected: ") {
        return t!(
            "workspace.init_task.feedback.language_selected",
            value = locale
        )
        .to_string();
    }
    if let Some(soul) = text
        .strip_prefix("Role selected: ")
        .or_else(|| text.strip_prefix("SOUL selected: "))
    {
        return t!("workspace.init_task.feedback.soul_selected", soul = soul).to_string();
    }
    if let Some(count) = model_count_message(text, "Fetched ", " model(s).") {
        return t!("workspace.init_task.feedback.models_fetched", count = count).to_string();
    }
    if let Some(count) = model_count_message(text, "Imported ", " model(s).") {
        return t!(
            "workspace.init_task.feedback.models_imported",
            count = count
        )
        .to_string();
    }

    match text {
        "Welcome to Alius. This workspace is not fully initialized yet." => {
            t!("workspace.init_task.feedback.welcome").to_string()
        }
        "Checking workspace." => t!("workspace.init_task.feedback.checking_workspace").to_string(),
        "Fetching model list." => t!("workspace.init_task.feedback.fetching_models").to_string(),
        "Capability lock generated." => {
            t!("workspace.init_task.feedback.capability_generated").to_string()
        }
        "Workspace template created." => {
            t!("workspace.init_task.feedback.workspace_created").to_string()
        }
        "Final validation passed." => {
            t!("workspace.init_task.feedback.validation_passed").to_string()
        }
        "Final validation found unfinished items." => {
            t!("workspace.init_task.feedback.validation_failed").to_string()
        }
        _ => text.to_string(),
    }
}

fn model_count_message(text: &str, prefix: &str, suffix: &str) -> Option<usize> {
    text.strip_prefix(prefix)
        .and_then(|value| value.strip_suffix(suffix))
        .and_then(|value| value.parse::<usize>().ok())
}

fn load_current_provider_config() -> ProviderConfig {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| {
            load_project_config(&cwd)
                .ok()
                .map(|snapshot| snapshot.providers)
                .or_else(|| load_provider_file_for_workspace(&cwd))
        })
        .unwrap_or_default()
}

fn load_provider_file_for_current_workspace() -> Option<ProviderConfig> {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| load_provider_file_for_workspace(&cwd))
}

fn load_provider_file_for_workspace(cwd: &Path) -> Option<ProviderConfig> {
    let root = runtime_config::project_init::project_root_for_init(cwd);
    let path = root.join(".alius/config/providers.toml");
    runtime_config::loaders::load_providers(&path).ok()
}

fn load_current_model_assignment(providers: &ProviderConfig) -> ModelAssignmentConfig {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| load_project_config(&cwd).ok())
        .map(|snapshot| snapshot.model_assignment)
        .unwrap_or_else(|| ModelAssignmentConfig::from_provider_tiers(providers))
}

fn init_cwd() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

fn build_init_wizard(
    settings: &Settings,
    providers: &ProviderConfig,
    assignment: &ModelAssignmentConfig,
    soul_choices: &[FormulaDef],
) -> InitWizard {
    let cwd = init_cwd();
    let loaded = runtime_config::project_init::load_init_state(&cwd)
        .ok()
        .flatten()
        .map(InitWizard::resume);
    let mut wizard =
        loaded.unwrap_or_else(|| InitWizard::with_context(InitContext::new(cwd.clone())));
    wizard.context.cwd = cwd;
    if wizard.state == InitState::Resume {
        refresh_init_context(
            &mut wizard.context,
            settings,
            providers,
            assignment,
            soul_choices,
        );
    } else {
        wizard.context.soul_choices = soul_choices
            .iter()
            .map(|formula| InitSoulRef {
                id: formula.id.clone(),
                description: formula.description.clone(),
            })
            .collect();
    }
    wizard
}

fn refresh_init_context(
    context: &mut InitContext,
    settings: &Settings,
    providers: &ProviderConfig,
    assignment: &ModelAssignmentConfig,
    soul_choices: &[FormulaDef],
) {
    context.cwd = init_cwd();
    context.language = (!settings.ui.locale.trim().is_empty()).then(|| settings.ui.locale.clone());
    context.model_pool = providers
        .model_library
        .models
        .iter()
        .filter(|entry| entry.enabled)
        .map(runtime_config::init_wizard::model_info_from_library)
        .collect();
    context.plan_model = non_empty_string(assignment.get(ModelAssignmentRole::Plan));
    context.execute_model = non_empty_string(assignment.get(ModelAssignmentRole::Execute));
    context.review_model = non_empty_string(assignment.get(ModelAssignmentRole::Review));
    context.selected_soul = non_empty_string(current_soul(settings).as_str());
    context.soul_choices = soul_choices
        .iter()
        .map(|formula| InitSoulRef {
            id: formula.id.clone(),
            description: formula.description.clone(),
        })
        .collect();
    if let Some(soul) = context.selected_soul.clone() {
        ensure_init_soul_choice(context, &soul, "current");
    }
    let alius_dir = runtime_config::project_init::project_alius_dir(&context.cwd);
    context.capability_lock_exists = alius_dir.join("capability/lock.toml").exists();
    context.workspace_created =
        alius_dir.join("workspace/specs").exists() || alius_dir.join("workspace/plans").exists();
}

fn non_empty_string(value: &str) -> Option<String> {
    (!value.trim().is_empty()).then(|| value.to_string())
}

fn ensure_init_soul_choice(context: &mut InitContext, id: &str, description: &str) {
    if context.soul_choices.iter().any(|soul| soul.id == id) {
        return;
    }
    context.soul_choices.push(InitSoulRef {
        id: id.to_string(),
        description: description.to_string(),
    });
}

fn init_issue_from_config_issue(issue: ConfigIssue) -> InitConfigIssue {
    InitConfigIssue {
        message: issue.message,
        section: match issue.section {
            ConfigSection::Language => InitConfigSection::Language,
            ConfigSection::ModelAssignment => InitConfigSection::ModelAssignment,
            ConfigSection::Soul => InitConfigSection::Soul,
        },
    }
}

fn seed_model_library_from_settings(settings: &Settings, providers: &mut ProviderConfig) {
    if settings.llm.model.trim().is_empty() || !providers.model_library.models.is_empty() {
        return;
    }
    let provider = provider_name(&settings.llm.provider).to_string();
    let model = settings.llm.model.clone();
    let base_url = settings.effective_base_url();
    let entry = ModelLibraryEntry {
        id: model_entry_id(&provider, &base_url, &model),
        display_name: model.clone(),
        provider: provider.clone(),
        base_url,
        model_name: model.clone(),
        reasoning_note: ReasoningNote::Standard,
        enabled: true,
    };
    providers.model_library.models.push(entry);
    providers.tiers.medium.provider = provider;
    providers.tiers.medium.model = model;
}

fn ensure_active_provider(settings: &Settings, providers: &mut ProviderConfig) {
    let provider = provider_name(&settings.llm.provider).to_string();
    ensure_provider_entry(&provider, providers);
    if let Some(entry) = providers.providers.get_mut(&provider) {
        entry.enabled = true;
        if entry.base_url.trim().is_empty() {
            entry.base_url = settings.effective_base_url();
        }
    }
}

fn ensure_provider_entry(provider: &str, providers: &mut ProviderConfig) {
    providers
        .providers
        .entry(provider.to_string())
        .or_insert_with(|| ProviderSettings {
            enabled: true,
            kind: provider_kind(provider).to_string(),
            base_url: default_base_url_for_provider(provider),
            api_key_env: default_api_key_env(provider).to_string(),
        });
}

fn provider_choices() -> Vec<ConfigChoice> {
    ["bigmodel", "xiaomi_mimo", "deepseek"]
        .into_iter()
        .map(|value| choice(provider_display_name(value), value))
        .collect()
}

fn api_protocol_choices() -> Vec<ConfigChoice> {
    vec![
        choice(ApiProtocol::OpenAi.label(), ApiProtocol::OpenAi.value()),
        choice(
            ApiProtocol::Anthropic.label(),
            ApiProtocol::Anthropic.value(),
        ),
    ]
}

fn language_choices() -> Vec<ConfigChoice> {
    LOCALES
        .iter()
        .map(|(code, label)| ConfigChoice {
            label: format!("{label} - {code}"),
            value: (*code).to_string(),
        })
        .collect()
}

fn add_base_url_choices(add: &AddModelState) -> Vec<ConfigChoice> {
    base_url_choices_for_provider(&add.provider, add.api_protocol, &add.base_url)
}

fn base_url_choices_for_provider(
    provider: &str,
    api_protocol: ApiProtocol,
    current: &str,
) -> Vec<ConfigChoice> {
    let mut choices = vec![(
        api_protocol.label(),
        default_base_url_for_provider_protocol(provider, api_protocol),
    )]
    .into_iter()
    .map(|(label, value)| ConfigChoice {
        label: format!("{label} - {value}"),
        value,
    })
    .collect::<Vec<_>>();
    choices.push(ConfigChoice {
        label: t!("workspace.config_task.action.custom_base_url").to_string(),
        value: current.to_string(),
    });
    choices
}

fn delete_confirm_choices() -> Vec<ConfigChoice> {
    vec![
        choice(
            t!("workspace.config_task.action.confirm_delete"),
            "confirm_delete",
        ),
        choice(
            t!("workspace.config_task.action.cancel_delete"),
            "cancel_delete",
        ),
    ]
}

fn choice(label: impl Into<String>, value: impl Into<String>) -> ConfigChoice {
    ConfigChoice {
        label: label.into(),
        value: value.into(),
    }
}

fn with_config_actions(choices: Vec<ConfigChoice>) -> Vec<ConfigChoice> {
    // Selections apply immediately; no Save/Cancel actions in the picker.
    choices
}

fn with_back(mut choices: Vec<ConfigChoice>) -> Vec<ConfigChoice> {
    choices.push(choice(t!("workspace.config_task.action.back"), "back"));
    choices
}

fn choice_value(raw: &str, choices: &[ConfigChoice]) -> String {
    let trimmed = raw.trim();
    if let Ok(number) = trimmed.parse::<usize>() {
        if let Some(choice) = choices.get(number.saturating_sub(1)) {
            return choice.value.clone();
        }
    }
    if let Some(choice) = choices.iter().find(|choice| {
        choice.value.eq_ignore_ascii_case(trimmed) || choice.label.eq_ignore_ascii_case(trimmed)
    }) {
        return choice.value.clone();
    }
    if trimmed.is_empty() {
        choices
            .first()
            .map(|choice| choice.value.clone())
            .unwrap_or_default()
    } else {
        trimmed.to_string()
    }
}

fn split_values(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect()
}

fn parse_assignment_role(value: &str) -> Option<ModelAssignmentRole> {
    match value {
        "plan" => Some(ModelAssignmentRole::Plan),
        "execute" => Some(ModelAssignmentRole::Execute),
        "review" => Some(ModelAssignmentRole::Review),
        _ => None,
    }
}

fn role_value(role: ModelAssignmentRole) -> &'static str {
    match role {
        ModelAssignmentRole::Plan => "plan",
        ModelAssignmentRole::Execute => "execute",
        ModelAssignmentRole::Review => "review",
    }
}

fn provider_name(provider: &ProviderType) -> &'static str {
    match provider {
        ProviderType::BigModel => "bigmodel",
        ProviderType::XiaomiMimo => "xiaomi_mimo",
        ProviderType::DeepSeek => "deepseek",
        _ => "bigmodel",
    }
}

fn provider_type_for_key(provider: &str) -> ProviderType {
    match provider {
        "bigmodel" => ProviderType::BigModel,
        "xiaomi_mimo" => ProviderType::XiaomiMimo,
        "deepseek" => ProviderType::DeepSeek,
        _ => ProviderType::BigModel,
    }
}

fn provider_kind(provider: &str) -> &'static str {
    match provider {
        "bigmodel" | "xiaomi_mimo" | "deepseek" => "openai-compatible",
        _ => "openai-compatible",
    }
}

fn provider_kind_for_base_url(base_url: &str) -> &'static str {
    if is_anthropic_base_url(base_url) {
        "anthropic"
    } else {
        "openai-compatible"
    }
}

fn default_api_key_env(provider: &str) -> &'static str {
    match provider {
        "bigmodel" => "BIGMODEL_API_KEY",
        "xiaomi_mimo" => "XIAOMI_MIMO_API_KEY",
        "deepseek" => "DEEPSEEK_API_KEY",
        _ => "BIGMODEL_API_KEY",
    }
}

fn default_base_url_for_provider(provider: &str) -> String {
    match provider {
        "bigmodel" => BIGMODEL_OPENAI_BASE_URL.to_string(),
        "xiaomi_mimo" => XIAOMI_MIMO_OPENAI_BASE_URL.to_string(),
        "deepseek" => DEEPSEEK_OPENAI_BASE_URL.to_string(),
        _ => BIGMODEL_OPENAI_BASE_URL.to_string(),
    }
}

fn default_base_url_for_provider_protocol(provider: &str, api_protocol: ApiProtocol) -> String {
    match (provider, api_protocol) {
        ("bigmodel", ApiProtocol::OpenAi) => BIGMODEL_OPENAI_BASE_URL.to_string(),
        ("bigmodel", ApiProtocol::Anthropic) => BIGMODEL_ANTHROPIC_BASE_URL.to_string(),
        ("xiaomi_mimo", ApiProtocol::OpenAi) => XIAOMI_MIMO_OPENAI_BASE_URL.to_string(),
        ("xiaomi_mimo", ApiProtocol::Anthropic) => XIAOMI_MIMO_ANTHROPIC_BASE_URL.to_string(),
        ("deepseek", ApiProtocol::OpenAi) => DEEPSEEK_OPENAI_BASE_URL.to_string(),
        ("deepseek", ApiProtocol::Anthropic) => DEEPSEEK_ANTHROPIC_BASE_URL.to_string(),
        (_, ApiProtocol::OpenAi) => BIGMODEL_OPENAI_BASE_URL.to_string(),
        (_, ApiProtocol::Anthropic) => BIGMODEL_ANTHROPIC_BASE_URL.to_string(),
    }
}

fn provider_mode_for_base_url(base_url: &str) -> Option<ProviderMode> {
    if is_anthropic_base_url(base_url) {
        Some(ProviderMode::Native)
    } else {
        Some(ProviderMode::OpenAICompatible)
    }
}

fn api_protocol_for_base_url(base_url: &str) -> ApiProtocol {
    if is_anthropic_base_url(base_url) {
        ApiProtocol::Anthropic
    } else {
        ApiProtocol::OpenAi
    }
}

fn parse_api_protocol(value: &str) -> Option<ApiProtocol> {
    match value {
        "openai" => Some(ApiProtocol::OpenAi),
        "anthropic" => Some(ApiProtocol::Anthropic),
        _ => None,
    }
}

fn default_api_protocol_for_provider(_provider: &str) -> ApiProtocol {
    ApiProtocol::OpenAi
}

fn is_anthropic_base_url(base_url: &str) -> bool {
    base_url.contains("/anthropic")
}

fn provider_display_name(provider: &str) -> &'static str {
    match provider {
        "bigmodel" => "BigModel GLM (Coding Plan)",
        "xiaomi_mimo" => "Xiaomi MiMo (Token Plan)",
        "deepseek" => "DeepSeek",
        _ => "Unknown Provider",
    }
}

fn provider_base_url(providers: &ProviderConfig, provider: &str) -> Option<String> {
    providers
        .providers
        .get(provider)
        .map(|settings| settings.base_url.clone())
        .filter(|url| !url.trim().is_empty())
}

fn valid_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn model_entry_id(provider: &str, base_url: &str, model: &str) -> String {
    let protocol = if is_anthropic_base_url(base_url) {
        "anthropic"
    } else {
        "openai"
    };
    format!("{}-{}-{}", provider, protocol, model)
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() {
                ch.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect::<String>()
        .split('-')
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("-")
}

fn model_entry_label(entry: &ModelLibraryEntry) -> String {
    let protocol = if is_anthropic_base_url(&entry.base_url) {
        "Anthropic API"
    } else {
        "OpenAI API"
    };
    format!(
        "{}    {}    {}",
        entry.display_name,
        provider_display_name(&entry.provider),
        protocol
    )
}

fn current_soul(settings: &Settings) -> String {
    let soul = settings.soul.role.as_str().trim();
    if soul.is_empty() {
        String::new()
    } else {
        soul.to_string()
    }
}

fn system_locale_or_default() -> String {
    let locale = std::env::var("LANG")
        .unwrap_or_default()
        .to_ascii_lowercase();
    if locale.starts_with("zh") {
        "zh-CN".to_string()
    } else if locale.starts_with("ja") {
        "ja".to_string()
    } else {
        "en".to_string()
    }
}

fn is_cancel(value: &str) -> bool {
    value.eq_ignore_ascii_case("/cancel") || value.eq_ignore_ascii_case("cancel")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tempfile::TempDir;

    struct CwdGuard(std::path::PathBuf);
    struct HomeGuard(Option<String>);

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    impl Drop for HomeGuard {
        fn drop(&mut self) {
            match &self.0 {
                Some(home) => std::env::set_var("HOME", home),
                None => std::env::remove_var("HOME"),
            }
        }
    }

    fn enter_temp_cwd() -> (TempDir, CwdGuard) {
        let original = std::env::current_dir().unwrap();
        let dir = TempDir::new().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        (dir, CwdGuard(original))
    }

    fn enter_temp_home() -> (TempDir, HomeGuard) {
        let original = std::env::var("HOME").ok();
        let dir = TempDir::new().unwrap();
        std::env::set_var("HOME", dir.path());
        (dir, HomeGuard(original))
    }

    fn install_test_soul(home: &std::path::Path, id: &str) {
        let dir = home
            .join(".alius")
            .join("soul")
            .join(id)
            .join("versions")
            .join("0.1.0");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("formula.toml"),
            format!(
                "id = \"{id}\"\nname = \"{id}\"\nversion = \"0.1.0\"\ntype = \"soul\"\ndescription = \"Test SOUL\"\n"
            ),
        )
        .unwrap();
    }

    fn settings_with_model() -> Settings {
        let mut settings = Settings::default();
        settings.llm.model = "gpt-4o".to_string();
        settings.llm.api_key = Some("sk-test".to_string());
        settings.soul.role = SoulRole::new("default".to_string());
        settings
    }

    fn add_second_model(task: &mut ConfigTask) {
        task.providers.model_library.models.push(ModelLibraryEntry {
            id: "reviewer".to_string(),
            display_name: "reviewer".to_string(),
            provider: "bigmodel".to_string(),
            base_url: BIGMODEL_OPENAI_BASE_URL.to_string(),
            model_name: "gpt-review".to_string(),
            reasoning_note: ReasoningNote::Standard,
            enabled: true,
        });
    }

    #[test]
    fn config_uses_plan_execute_review_labels() {
        let task = ConfigTask::new(settings_with_model());
        let prompt = task.prompt();

        assert!(prompt.message.contains("Plan-Execute-Review"));
        assert!(prompt
            .input
            .choices
            .iter()
            .any(|choice| choice.label.contains("Plan Model")));
        assert!(!prompt.message.contains("Quick Reasoning"));
    }

    #[test]
    fn init_auto_advances_to_select_language_without_start_prompt() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let task = ConfigTask::init(settings_with_model());
        let prompt = task.prompt();

        // Auto-start skips Start and lands on SelectLanguage (fresh temp dir).
        assert_eq!(prompt.input.title, "Choose interface language");
        assert_eq!(prompt.input.scope_title.as_deref(), Some("select-language"));
    }

    #[test]
    fn fresh_init_does_not_prefill_language_or_role_from_settings() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let mut settings = settings_with_model();
        settings.ui.locale = "zh-CN".to_string();
        settings.soul.role = SoulRole::new("default".to_string());

        let task = ConfigTask::init(settings);
        let wizard = task.init_wizard.as_ref().unwrap();

        assert!(wizard.context.language.is_none());
        assert!(wizard.context.selected_soul.is_none());
        assert!(wizard.context.plan_model.is_none());
        assert!(wizard.context.model_pool.is_empty());
    }

    #[test]
    fn init_reset_restores_default_project_configuration() {
        crate::set_locale("zh-CN");
        let (_dir, _guard) = enter_temp_cwd();
        let mut settings = settings_with_model();
        settings.ui.locale = "zh-CN".to_string();
        let mut task = ConfigTask::init(settings);
        assert!(!task.providers.model_library.models.is_empty());

        let event = task.execute_init_command(InitCommand::CreateProjectDirs { reset: true });

        assert!(matches!(event, Some(InitEvent::ProjectCreated)));
        assert_eq!(task.draft.ui.locale, "en");
        assert!(task.draft.soul.role.as_str().is_empty());
        assert!(task.providers.model_library.models.is_empty());
        assert!(task.assignment.get(ModelAssignmentRole::Plan).is_empty());
        assert_eq!(rust_i18n::locale().to_string(), "en");
    }

    #[test]
    fn init_language_state_uses_language_scope() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let mut task = ConfigTask::init(settings_with_model());
        task.init_wizard.as_mut().unwrap().state = InitState::SelectLanguage;

        let prompt = task.prompt().input;

        assert_eq!(prompt.title, "Choose interface language");
        assert_eq!(prompt.scope_title.as_deref(), Some("select-language"));
        assert!(prompt.choices.iter().any(|choice| choice.value == "zh-CN"));
        assert!(prompt.choices.iter().any(|choice| choice.value == "system"));
    }

    #[test]
    fn init_prompt_uses_side_panel_for_flow() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let task = ConfigTask::init(settings_with_model());
        let prompt = task.prompt();

        assert!(prompt.side_panel.is_some());
        assert!(prompt
            .side_panel
            .as_ref()
            .unwrap()
            .content
            .contains("Configuration check"));
        assert!(!prompt.side_panel.as_ref().unwrap().content.contains("cwd:"));
    }

    #[test]
    fn init_language_selection_returns_visible_feedback() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let mut task = ConfigTask::init(settings_with_model());
        task.init_wizard.as_mut().unwrap().state = InitState::SelectLanguage;

        let outcome = task.submit("zh-CN");
        crate::set_locale("en");

        match outcome {
            ConfigTaskOutcome::Next { accepted, .. } => {
                assert!(accepted.contains("zh-CN"), "accepted={accepted:?}");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn init_language_selection_localizes_next_prompt() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let mut task = ConfigTask::init(settings_with_model());
        task.init_wizard.as_mut().unwrap().state = InitState::SelectLanguage;

        let outcome = task.submit("zh-CN");
        crate::set_locale("en");

        let ConfigTaskOutcome::Next { prompt, .. } = outcome else {
            panic!("expected next prompt after language selection");
        };
        assert_eq!(prompt.input.title, "模型池");
        assert!(prompt
            .side_panel
            .as_ref()
            .unwrap()
            .content
            .contains("配置模型池"));
    }

    #[test]
    fn init_soul_selection_activates_soul_and_returns_feedback() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let (home, _home_guard) = enter_temp_home();
        install_test_soul(home.path(), "default");
        let mut task = ConfigTask::init(settings_with_model());
        let model_id = task.providers.model_library.models[0].id.clone();
        for role in ModelAssignmentRole::all() {
            task.assignment.set(role, model_id.clone());
        }
        task.sync_assignment_compat();
        task.init_wizard.as_mut().unwrap().state = InitState::ConfigureSoul;

        let outcome = task.submit("default");

        assert!(matches!(
            outcome,
            ConfigTaskOutcome::Saved { message, .. } if !message.trim().is_empty()
        ));
        assert_eq!(
            crate::formula::current_project_soul(),
            Some("default".to_string())
        );
        assert_eq!(
            task.init_wizard.as_ref().unwrap().state,
            InitState::Complete
        );
    }

    #[test]
    fn init_api_key_credentials_are_saved_for_chat_runtime() {
        crate::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let mut settings = settings_with_model();
        settings.llm.api_key = None;
        settings.llm.api_key_env = None;
        let mut task = ConfigTask::init(settings);

        task.apply_init_api_credentials(
            "deepseek",
            InitApiProtocol::OpenAi,
            DEEPSEEK_OPENAI_BASE_URL,
            "sk-init",
        );

        assert_eq!(task.draft.llm.get_api_key(), Some("sk-init".to_string()));
        assert_eq!(task.draft.llm.api_key_env, None);
        assert_eq!(
            task.draft.llm.base_url.as_deref(),
            Some(DEEPSEEK_OPENAI_BASE_URL)
        );
    }

    #[test]
    fn init_model_pool_add_flow_uses_init_operation_scope() {
        crate::set_locale("en");
        let mut task = ConfigTask::init(settings_with_model());
        let wizard = task.init_wizard.as_mut().unwrap();
        wizard.state = InitState::ModelInputApiKey;

        let prompt = task.prompt().input;

        assert_eq!(prompt.title, "Enter API Key");
        assert_eq!(prompt.scope_title.as_deref(), Some("configure-model-pool"));
        assert!(matches!(
            prompt.kind,
            PromptInputKind::Text { masked: false }
        ));
    }

    #[test]
    fn assignment_updates_compatibility_fields() {
        let mut task = ConfigTask::new(settings_with_model());
        add_second_model(&mut task);
        let primary = task.providers.model_library.models[0].id.clone();

        task.assignment
            .set(ModelAssignmentRole::Plan, primary.clone());
        task.assignment
            .set(ModelAssignmentRole::Execute, primary.clone());
        task.assignment.set(ModelAssignmentRole::Review, "reviewer");
        task.sync_assignment_compat();

        assert_eq!(task.providers.tiers.light.model, "gpt-4o");
        assert_eq!(task.providers.tiers.medium.model, "gpt-4o");
        assert_eq!(task.providers.tiers.high.model, "gpt-review");
        assert_eq!(task.draft.llm.model, "gpt-4o");
        assert_eq!(task.draft.llm.review_model.as_deref(), Some("gpt-review"));
    }

    #[test]
    fn deleting_assigned_model_is_blocked() {
        let mut task = ConfigTask::model_switch(settings_with_model());
        let id = task.providers.model_library.models[0].id.clone();
        task.assignment.set(ModelAssignmentRole::Plan, id.clone());
        task.pool_mode = ModelPoolMode::DeleteSelect;

        let outcome = task.submit_model_delete_select(&id);

        assert!(matches!(outcome, ConfigTaskOutcome::Invalid { .. }));
        assert!(task
            .providers
            .model_library
            .models
            .iter()
            .any(|entry| entry.id == id));
    }

    #[test]
    fn api_key_prompt_is_plaintext_and_accepts_paste() {
        let mut task = ConfigTask::model_switch(settings_with_model());
        task.start_add_model();
        task.add_model.as_mut().unwrap().step = AddModelStep::ApiKey;
        let prompt = task.prompt().input;

        assert!(matches!(
            prompt.kind,
            PromptInputKind::Text { masked: false }
        ));

        let mut input = prompt;
        input.paste("sk-pasted");
        assert_eq!(input.custom_input.value(), "sk-pasted");
        assert!(matches!(
            input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            super::super::interaction::PromptInputAction::Submit(value) if value == "sk-pasted"
        ));
    }

    #[test]
    fn model_pool_add_flow_does_not_offer_manual_model() {
        let mut task = ConfigTask::model_switch(settings_with_model());
        task.start_add_model();
        task.add_model.as_mut().unwrap().step = AddModelStep::SelectModels;
        task.add_model.as_mut().unwrap().models = vec!["gpt-4o".to_string()];

        let prompt = task.prompt().input;

        assert!(matches!(prompt.kind, PromptInputKind::MultiSelect));
        assert!(!prompt
            .choices
            .iter()
            .any(|choice| choice.label.to_ascii_lowercase().contains("manual")));
    }

    #[test]
    fn model_pool_add_flow_selects_provider_then_api_protocol() {
        let mut task = ConfigTask::model_switch(settings_with_model());
        task.start_add_model();

        let provider_prompt = task.prompt().input;
        assert!(provider_prompt
            .choices
            .iter()
            .any(|choice| choice.value == "deepseek"));

        let outcome = task.submit_add_model("deepseek");
        assert!(matches!(outcome, ConfigTaskOutcome::Next { .. }));
        assert_eq!(
            task.add_model.as_ref().map(|add| add.step),
            Some(AddModelStep::ApiProtocol)
        );

        let api_prompt = task.prompt().input;
        assert!(api_prompt
            .choices
            .iter()
            .any(|choice| choice.value == "openai"));
        assert!(api_prompt
            .choices
            .iter()
            .any(|choice| choice.value == "anthropic"));

        let outcome = task.submit_add_model("anthropic");
        assert!(matches!(outcome, ConfigTaskOutcome::Next { .. }));
        let add = task.add_model.as_ref().unwrap();
        assert_eq!(add.step, AddModelStep::BaseUrl);
        assert_eq!(add.base_url, DEEPSEEK_ANTHROPIC_BASE_URL);
    }

    #[test]
    fn model_pool_import_persists_for_next_config_task() {
        let (_dir, _guard) = enter_temp_cwd();
        let mut task = ConfigTask::model_switch(Settings::default());
        task.add_model = Some(AddModelState {
            step: AddModelStep::SelectModels,
            provider: "deepseek".to_string(),
            api_protocol: ApiProtocol::OpenAi,
            base_url: DEEPSEEK_OPENAI_BASE_URL.to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["deepseek-chat".to_string()],
            selected_models: Vec::new(),
            fetch_failed: false,
        });

        let outcome = task.submit_add_model("deepseek-chat");

        assert!(matches!(outcome, ConfigTaskOutcome::Next { .. }));
        let reloaded = ConfigTask::new(Settings::default());
        assert!(reloaded
            .enabled_models()
            .iter()
            .any(|entry| entry.model_name == "deepseek-chat"));
    }

    #[test]
    fn config_save_preserves_existing_model_pool_when_task_pool_is_empty() {
        let (_dir, _guard) = enter_temp_cwd();
        let mut model_task = ConfigTask::model_switch(Settings::default());
        model_task.add_model = Some(AddModelState {
            step: AddModelStep::SelectModels,
            provider: "deepseek".to_string(),
            api_protocol: ApiProtocol::OpenAi,
            base_url: DEEPSEEK_OPENAI_BASE_URL.to_string(),
            api_key: "sk-test".to_string(),
            models: vec!["deepseek-chat".to_string()],
            selected_models: Vec::new(),
            fetch_failed: false,
        });
        assert!(matches!(
            model_task.submit_add_model("deepseek-chat"),
            ConfigTaskOutcome::Next { .. }
        ));

        let model_id = "deepseek-openai-deepseek-chat";
        let mut config_task = ConfigTask::new(Settings::default());
        config_task.providers.model_library.models.clear();
        config_task
            .assignment
            .set(ModelAssignmentRole::Plan, model_id);
        config_task
            .assignment
            .set(ModelAssignmentRole::Execute, model_id);
        config_task
            .assignment
            .set(ModelAssignmentRole::Review, model_id);
        config_task.draft.soul.role = SoulRole::new("default".to_string());
        config_task.draft.llm.api_key = Some("sk-test".to_string());
        config_task.draft.llm.api_key_env = None;

        let outcome = config_task.save_config();

        let ConfigTaskOutcome::Saved { providers, .. } = outcome else {
            panic!("expected config save to preserve the model pool");
        };
        let root = runtime_config::project_init::project_root_for_init(&init_cwd());
        runtime_config::loaders::save_providers(
            &root.join(".alius/config/providers.toml"),
            &providers,
        )
        .unwrap();

        let reloaded = ConfigTask::model_switch(Settings::default());
        assert!(reloaded
            .enabled_models()
            .iter()
            .any(|entry| entry.model_name == "deepseek-chat"));
    }

    #[test]
    fn init_model_import_persists_for_next_config_task() {
        let (_dir, _guard) = enter_temp_cwd();
        let mut task = ConfigTask::init(Settings::default());
        let model = InitModelInfo {
            id: "deepseek-openai-deepseek-chat".to_string(),
            display_name: "deepseek-chat".to_string(),
            provider: "deepseek".to_string(),
            api_protocol: InitApiProtocol::OpenAi,
            base_url: DEEPSEEK_OPENAI_BASE_URL.to_string(),
            model_name: "deepseek-chat".to_string(),
        };

        let event = task.execute_init_command(InitCommand::ImportModels(vec![model]));

        assert!(
            matches!(event, Some(InitEvent::ModelsImported(ids)) if ids == vec!["deepseek-openai-deepseek-chat".to_string()])
        );
        let reloaded = ConfigTask::new(Settings::default());
        assert!(reloaded
            .enabled_models()
            .iter()
            .any(|entry| entry.model_name == "deepseek-chat"));
    }
}
