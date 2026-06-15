//! Pure `/init` state machine.
//!
//! This module owns initialization state, transitions, and renderable view
//! data. It intentionally performs no filesystem, network, model, or SOUL IO.

use crate::{ModelAssignmentRole, ModelLibraryEntry};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitState {
    Start,
    Resume,
    CheckWorkspace,
    CreateProject,
    SelectLanguage,
    ConfigureModelPool,
    ModelSelectProvider,
    ModelInputBaseUrl,
    ModelInputApiKey,
    ModelFetchList,
    ModelImportSelect,
    ConfigureModelAssignment,
    SelectPlanModel,
    SelectExecuteModel,
    SelectReviewModel,
    ConfigureSoul,
    ResolveCapability,
    CreateWorkspace,
    Validate,
    Complete,
    Error,
    Cancelled,
}

/// The four user-configurable init stages, shown in the top-bar nav.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InitStage {
    Language,
    ModelPool,
    Assignment,
    Soul,
}

impl InitStage {
    pub fn all() -> [Self; 4] {
        [
            Self::Language,
            Self::ModelPool,
            Self::Assignment,
            Self::Soul,
        ]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitEvent {
    Start,
    Select(usize),
    Toggle(usize),
    Confirm,
    Back,
    Cancel,
    TextInput(String),
    AsyncOk,
    AsyncFailed(String),
    WorkspaceChecked(WorkspaceCheckResult),
    ProjectCreated,
    ModelsFetched(Vec<ModelInfo>),
    ModelsImported(Vec<String>),
    CapabilityResolved,
    WorkspaceCreated,
    ValidationPassed,
    ValidationFailed(Vec<ConfigIssue>),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitCommand {
    None,
    CheckWorkspace,
    CreateProjectDirs {
        reset: bool,
    },
    WriteLanguageConfig {
        locale: String,
    },
    FetchModels {
        provider: String,
        api_protocol: ApiProtocol,
        base_url: String,
        api_key: String,
    },
    ImportModels(Vec<ModelInfo>),
    WriteModelAssignment {
        plan: String,
        execute: String,
        review: String,
    },
    WriteSoulConfig(String),
    ResolveCapability,
    CreateWorkspaceFromTemplate,
    ValidateConfig,
    Complete,
    Cancel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ApiProtocol {
    OpenAi,
    Anthropic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkspaceCheckResult {
    pub ok: bool,
    pub git: Option<GitStatus>,
    pub message: Option<String>,
    /// True when `.alius` already existed before `check_workspace` ran.
    /// When false, the wizard auto-creates project defaults without prompting.
    pub alius_existed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GitStatus {
    pub branch: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelInfo {
    pub id: String,
    pub display_name: String,
    pub provider: String,
    pub api_protocol: ApiProtocol,
    pub base_url: String,
    pub model_name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SoulRef {
    pub id: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigIssue {
    pub message: String,
    pub section: InitConfigSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitConfigSection {
    Language,
    ModelPool,
    ModelAssignment,
    Soul,
    Capability,
    Workspace,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum InitMessage {
    Info(String),
    Success(String),
    Warning(String),
    Error(String),
    Running(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CheckItemStatus {
    Done,
    Active,
    Pending,
    Warning,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitCheckItem {
    pub index: usize,
    pub title: String,
    pub status: CheckItemStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitErrorState {
    pub source_state: InitState,
    pub message: String,
    pub recover_options: Vec<RecoverAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RecoverAction {
    Retry,
    Back,
    Skip,
    Cancel,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitContext {
    pub cwd: PathBuf,
    pub git: Option<GitStatus>,
    pub language: Option<String>,
    pub selected_provider: Option<String>,
    pub api_protocol: Option<ApiProtocol>,
    pub base_url: Option<String>,
    pub api_key: Option<String>,
    pub fetched_models: Vec<ModelInfo>,
    pub selected_model_indices: Vec<usize>,
    pub model_pool: Vec<ModelInfo>,
    pub plan_model: Option<String>,
    pub execute_model: Option<String>,
    pub review_model: Option<String>,
    pub soul_choices: Vec<SoulRef>,
    pub selected_soul: Option<String>,
    pub capability_lock_exists: bool,
    pub workspace_created: bool,
    pub issues: Vec<ConfigIssue>,
    pub message_log: Vec<InitMessage>,
    pub selected_index: usize,
    pub error: Option<InitErrorState>,
}

impl InitContext {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            cwd,
            git: None,
            language: None,
            selected_provider: None,
            api_protocol: None,
            base_url: None,
            api_key: None,
            fetched_models: Vec::new(),
            selected_model_indices: Vec::new(),
            model_pool: Vec::new(),
            plan_model: None,
            execute_model: None,
            review_model: None,
            soul_choices: Vec::new(),
            selected_soul: None,
            capability_lock_exists: false,
            workspace_created: false,
            issues: Vec::new(),
            message_log: Vec::new(),
            selected_index: 0,
            error: None,
        }
    }

    pub fn with_existing_config(
        cwd: PathBuf,
        language: Option<String>,
        model_pool: Vec<ModelInfo>,
        plan_model: Option<String>,
        execute_model: Option<String>,
        review_model: Option<String>,
        selected_soul: Option<String>,
        soul_choices: Vec<SoulRef>,
    ) -> Self {
        let mut context = Self::new(cwd);
        context.language = language;
        context.model_pool = model_pool;
        context.plan_model = plan_model;
        context.execute_model = execute_model;
        context.review_model = review_model;
        context.selected_soul = selected_soul;
        context.soul_choices = soul_choices;
        context
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitWizard {
    pub state: InitState,
    pub context: InitContext,
}

impl InitWizard {
    pub fn new(cwd: PathBuf) -> Self {
        Self {
            state: InitState::Start,
            context: InitContext::new(cwd),
        }
    }

    pub fn with_context(context: InitContext) -> Self {
        Self {
            state: InitState::Start,
            context,
        }
    }

    pub fn resume(mut self) -> Self {
        self.state = InitState::Resume;
        self.context.selected_index = 0;
        self
    }

    pub fn handle_event(&mut self, event: InitEvent) -> InitCommand {
        match event {
            InitEvent::Select(index) => {
                self.context.selected_index = index.min(self.max_selected_index());
                InitCommand::None
            }
            InitEvent::Toggle(index) => {
                self.toggle_index(index);
                InitCommand::None
            }
            InitEvent::Back => self.back(),
            InitEvent::Cancel => self.cancel(),
            InitEvent::Confirm => self.confirm(),
            InitEvent::TextInput(value) => self.text_input(value),
            InitEvent::WorkspaceChecked(result) => self.workspace_checked(result),
            InitEvent::ProjectCreated => self.project_created(),
            InitEvent::ModelsFetched(models) => self.models_fetched(models),
            InitEvent::ModelsImported(ids) => self.models_imported(ids),
            InitEvent::CapabilityResolved => self.capability_resolved(),
            InitEvent::WorkspaceCreated => self.workspace_created(),
            InitEvent::ValidationPassed => self.validation_passed(),
            InitEvent::ValidationFailed(issues) => self.validation_failed(issues),
            InitEvent::AsyncOk => self.async_ok(),
            InitEvent::AsyncFailed(message) => self.async_failed(message),
            InitEvent::Start => {
                self.state = InitState::Start;
                self.context.selected_index = 0;
                InitCommand::None
            }
        }
    }

    pub fn view_model(&self) -> InitViewModel {
        InitViewModel {
            header: self.header_text(),
            messages: self.render_messages(),
            check_items: self.render_check_items(),
            action_panel: self.action_panel(),
            footer: self.footer_text(),
            scope_title: self.scope_title().to_string(),
        }
    }

    fn confirm(&mut self) -> InitCommand {
        match self.state {
            InitState::Start => match self.selected_value().as_deref() {
                Some("exit") => self.cancel(),
                _ => {
                    self.state = InitState::CheckWorkspace;
                    self.context.selected_index = 0;
                    self.context
                        .message_log
                        .push(InitMessage::Running("Checking workspace.".to_string()));
                    InitCommand::CheckWorkspace
                }
            },
            InitState::Resume => match self.selected_value().as_deref() {
                Some("restart") => {
                    self.state = InitState::Start;
                    self.context.message_log.clear();
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                Some("exit") => self.cancel(),
                _ => {
                    self.state = self.next_unfinished_state();
                    self.context.selected_index = 0;
                    InitCommand::None
                }
            },
            InitState::CreateProject => match self.selected_value().as_deref() {
                Some("reinitialize_project") => InitCommand::CreateProjectDirs { reset: true },
                Some("exit") => self.cancel(),
                _ => self.cancel(),
            },
            InitState::SelectLanguage => match self.selected_value().as_deref() {
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                Some(value) => {
                    let locale = if value == "system" {
                        "system".to_string()
                    } else {
                        value.to_string()
                    };
                    self.context.language = Some(locale.clone());
                    self.context
                        .message_log
                        .push(InitMessage::Success(format!("Language selected: {locale}")));
                    self.state = InitState::ConfigureModelPool;
                    self.context.selected_index = 0;
                    InitCommand::WriteLanguageConfig { locale }
                }
                None => InitCommand::None,
            },
            InitState::ConfigureModelPool => match self.selected_value().as_deref() {
                Some("add_model") => {
                    self.state = InitState::ModelSelectProvider;
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                Some("configure_later") | Some("continue_assignment") => {
                    self.state = InitState::ConfigureModelAssignment;
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                _ => InitCommand::None,
            },
            InitState::ModelSelectProvider => match self.selected_model_provider_choice() {
                Some(choice) => {
                    self.context.selected_provider = Some(choice.provider.to_string());
                    self.context.api_protocol = Some(choice.api_protocol);
                    self.context.base_url = Some(choice.base_url.to_string());
                    self.state = InitState::ModelInputBaseUrl;
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                None => InitCommand::None,
            },
            InitState::ModelImportSelect => {
                let selected = self.selected_models_for_import();
                if selected.is_empty() {
                    self.enter_error(
                        InitState::ModelImportSelect,
                        "Select at least one model to import.",
                        vec![RecoverAction::Back, RecoverAction::Cancel],
                    );
                    InitCommand::None
                } else {
                    InitCommand::ImportModels(selected)
                }
            }
            InitState::ConfigureModelAssignment => match self.selected_value().as_deref() {
                Some("plan") => self.enter_assignment_state(InitState::SelectPlanModel),
                Some("execute") => self.enter_assignment_state(InitState::SelectExecuteModel),
                Some("review") => self.enter_assignment_state(InitState::SelectReviewModel),
                Some("continue_soul") => {
                    if let Some(command) = self.assignment_command() {
                        self.state = InitState::ConfigureSoul;
                        self.context.selected_index = 0;
                        command
                    } else {
                        self.enter_error(
                            InitState::ConfigureModelAssignment,
                            "Plan, Execute, and Review models must be assigned.",
                            vec![RecoverAction::Back, RecoverAction::Cancel],
                        );
                        InitCommand::None
                    }
                }
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                _ => InitCommand::None,
            },
            InitState::SelectPlanModel
            | InitState::SelectExecuteModel
            | InitState::SelectReviewModel => match self.selected_value().as_deref() {
                Some("back") => self.back(),
                Some(model_id) => {
                    match self.state {
                        InitState::SelectPlanModel => {
                            self.context.plan_model = Some(model_id.to_string())
                        }
                        InitState::SelectExecuteModel => {
                            self.context.execute_model = Some(model_id.to_string())
                        }
                        InitState::SelectReviewModel => {
                            self.context.review_model = Some(model_id.to_string())
                        }
                        _ => {}
                    }
                    self.state = InitState::ConfigureModelAssignment;
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                None => InitCommand::None,
            },
            InitState::ConfigureSoul => match self.selected_value().as_deref() {
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                Some(soul) => {
                    self.context.selected_soul = Some(soul.to_string());
                    self.context.selected_index = 0;
                    InitCommand::WriteSoulConfig(soul.to_string())
                }
                None => InitCommand::None,
            },
            InitState::ResolveCapability => match self.selected_value().as_deref() {
                Some("skip_capability") => {
                    self.state = InitState::Complete;
                    self.context.selected_index = 0;
                    InitCommand::Complete
                }
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                _ => {
                    self.state = InitState::Complete;
                    self.context.selected_index = 0;
                    InitCommand::Complete
                }
            },
            InitState::CreateWorkspace => match self.selected_value().as_deref() {
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                _ => InitCommand::CreateWorkspaceFromTemplate,
            },
            InitState::Validate => match self.selected_value().as_deref() {
                Some("fix_validation") => {
                    self.state = self.state_for_first_issue();
                    self.context.issues.clear();
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                Some("ignore_validation") => {
                    self.context.issues.clear();
                    self.state = InitState::Complete;
                    self.context.selected_index = 0;
                    InitCommand::None
                }
                Some("back") => self.back(),
                Some("exit") => self.cancel(),
                _ => InitCommand::ValidateConfig,
            },
            InitState::Complete => match self.selected_value().as_deref() {
                Some("view_config") => {
                    self.context
                        .message_log
                        .push(InitMessage::Info(self.summary()));
                    InitCommand::None
                }
                _ => InitCommand::Complete,
            },
            InitState::Error => self.recover(),
            InitState::CheckWorkspace
            | InitState::ModelInputBaseUrl
            | InitState::ModelInputApiKey
            | InitState::ModelFetchList
            | InitState::Cancelled => InitCommand::None,
        }
    }

    fn text_input(&mut self, value: String) -> InitCommand {
        match self.state {
            InitState::ModelInputBaseUrl => {
                let url = value.trim();
                if !valid_url(url) {
                    self.enter_error(
                        InitState::ModelInputBaseUrl,
                        "Base URL format error: missing protocol. Example: https://api.example.com/v1",
                        vec![RecoverAction::Retry, RecoverAction::Back, RecoverAction::Cancel],
                    );
                    InitCommand::None
                } else {
                    self.context.base_url = Some(url.to_string());
                    self.state = InitState::ModelInputApiKey;
                    self.context.selected_index = 0;
                    InitCommand::None
                }
            }
            InitState::ModelInputApiKey => {
                let api_key = value.trim().to_string();
                self.context.api_key = Some(api_key.clone());
                self.state = InitState::ModelFetchList;
                self.context
                    .message_log
                    .push(InitMessage::Running("Fetching model list.".to_string()));
                InitCommand::FetchModels {
                    provider: self.context.selected_provider.clone().unwrap_or_default(),
                    api_protocol: self.context.api_protocol.unwrap_or(ApiProtocol::OpenAi),
                    base_url: self.context.base_url.clone().unwrap_or_default(),
                    api_key,
                }
            }
            _ => InitCommand::None,
        }
    }

    fn workspace_checked(&mut self, result: WorkspaceCheckResult) -> InitCommand {
        if result.ok {
            self.context.git = result.git;
            self.context.message_log.push(InitMessage::Success(
                result
                    .message
                    .unwrap_or_else(|| "Workspace check complete.".to_string()),
            ));
            // Fresh workspace: auto-create .alius without prompting.
            // Existing .alius: land on CreateProject so the user picks Reinitialize/Exit.
            if !result.alius_existed {
                self.state = InitState::CreateProject;
                self.context.selected_index = 0;
                return InitCommand::CreateProjectDirs { reset: false };
            }
            self.state = InitState::CreateProject;
            self.context.selected_index = 0;
        } else {
            self.enter_error(
                InitState::CheckWorkspace,
                result
                    .message
                    .unwrap_or_else(|| "Workspace check failed.".to_string()),
                vec![
                    RecoverAction::Retry,
                    RecoverAction::Back,
                    RecoverAction::Cancel,
                ],
            );
        }
        InitCommand::None
    }

    fn project_created(&mut self) -> InitCommand {
        self.context.message_log.push(InitMessage::Success(
            ".alius project structure ready.".to_string(),
        ));
        self.state = InitState::SelectLanguage;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn models_fetched(&mut self, models: Vec<ModelInfo>) -> InitCommand {
        if models.is_empty() {
            self.enter_error(
                InitState::ModelInputApiKey,
                "No models returned. Check API Key or Base URL and try again.",
                vec![
                    RecoverAction::Retry,
                    RecoverAction::Back,
                    RecoverAction::Cancel,
                ],
            );
            return InitCommand::None;
        }
        let model_count = models.len();
        self.context.fetched_models = models;
        self.context.selected_model_indices = (0..self.context.fetched_models.len()).collect();
        self.context.message_log.push(InitMessage::Success(format!(
            "Fetched {model_count} model(s)."
        )));
        self.state = InitState::ModelImportSelect;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn models_imported(&mut self, ids: Vec<String>) -> InitCommand {
        for model in self
            .context
            .fetched_models
            .iter()
            .filter(|model| ids.iter().any(|id| id == &model.id))
        {
            if let Some(existing) = self
                .context
                .model_pool
                .iter_mut()
                .find(|entry| entry.id == model.id)
            {
                *existing = model.clone();
            } else {
                self.context.model_pool.push(model.clone());
            }
        }
        self.context.message_log.push(InitMessage::Success(format!(
            "Imported {} model(s).",
            ids.len()
        )));
        self.state = InitState::ConfigureModelAssignment;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn capability_resolved(&mut self) -> InitCommand {
        self.context.capability_lock_exists = true;
        self.context.message_log.push(InitMessage::Success(
            "Capability lock generated.".to_string(),
        ));
        self.state = InitState::Complete;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn workspace_created(&mut self) -> InitCommand {
        self.context.workspace_created = true;
        self.context.message_log.push(InitMessage::Success(
            "Workspace template created.".to_string(),
        ));
        self.state = InitState::Validate;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn validation_passed(&mut self) -> InitCommand {
        self.context.issues.clear();
        self.context
            .message_log
            .push(InitMessage::Success("Final validation passed.".to_string()));
        self.state = InitState::Complete;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn validation_failed(&mut self, issues: Vec<ConfigIssue>) -> InitCommand {
        self.context.issues = issues;
        self.enter_error(
            InitState::Validate,
            "Final validation found unfinished items.",
            vec![
                RecoverAction::Retry,
                RecoverAction::Back,
                RecoverAction::Skip,
                RecoverAction::Cancel,
            ],
        );
        InitCommand::None
    }

    fn async_ok(&mut self) -> InitCommand {
        match self.state {
            InitState::CreateProject => self.project_created(),
            InitState::ConfigureSoul => {
                let soul = self
                    .context
                    .selected_soul
                    .clone()
                    .unwrap_or_else(|| "Role".to_string());
                self.context
                    .message_log
                    .push(InitMessage::Success(format!("Role selected: {soul}")));
                self.state = InitState::Complete;
                self.context.selected_index = 0;
                InitCommand::Complete
            }
            InitState::ResolveCapability => self.capability_resolved(),
            InitState::CreateWorkspace => self.workspace_created(),
            _ => InitCommand::None,
        }
    }

    fn async_failed(&mut self, message: String) -> InitCommand {
        let source_state = match self.state {
            InitState::ModelFetchList => InitState::ModelInputApiKey,
            other => other,
        };
        self.enter_error(
            source_state,
            message,
            vec![
                RecoverAction::Retry,
                RecoverAction::Back,
                RecoverAction::Skip,
                RecoverAction::Cancel,
            ],
        );
        InitCommand::None
    }

    pub fn back(&mut self) -> InitCommand {
        self.state = match self.state {
            InitState::Resume => InitState::Start,
            InitState::CreateProject => InitState::Start,
            InitState::SelectLanguage => InitState::CreateProject,
            InitState::ConfigureModelPool => InitState::SelectLanguage,
            InitState::ModelSelectProvider => InitState::ConfigureModelPool,
            InitState::ModelInputBaseUrl => InitState::ModelSelectProvider,
            InitState::ModelInputApiKey => InitState::ModelInputBaseUrl,
            InitState::ModelImportSelect => InitState::ModelInputApiKey,
            InitState::ConfigureModelAssignment => InitState::ConfigureModelPool,
            InitState::SelectPlanModel
            | InitState::SelectExecuteModel
            | InitState::SelectReviewModel => InitState::ConfigureModelAssignment,
            InitState::ConfigureSoul => InitState::ConfigureModelAssignment,
            InitState::ResolveCapability => InitState::ConfigureSoul,
            InitState::CreateWorkspace | InitState::Validate => InitState::ConfigureSoul,
            InitState::Error => self
                .context
                .error
                .as_ref()
                .map(|error| previous_state(error.source_state))
                .unwrap_or(InitState::Start),
            InitState::Complete => InitState::ConfigureSoul,
            state => state,
        };
        self.context.error = None;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn cancel(&mut self) -> InitCommand {
        self.state = InitState::Cancelled;
        self.context.selected_index = 0;
        InitCommand::Cancel
    }

    fn recover(&mut self) -> InitCommand {
        let Some(error) = self.context.error.clone() else {
            self.state = InitState::Start;
            return InitCommand::None;
        };
        match self.selected_recover_action(&error) {
            Some(RecoverAction::Retry) => {
                self.state = error.source_state;
                self.context.error = None;
                self.context.selected_index = 0;
                match self.state {
                    InitState::CheckWorkspace => InitCommand::CheckWorkspace,
                    InitState::CreateProject => InitCommand::CreateProjectDirs { reset: false },
                    InitState::ModelInputApiKey => InitCommand::None,
                    InitState::ResolveCapability => InitCommand::Complete,
                    InitState::CreateWorkspace => InitCommand::CreateWorkspaceFromTemplate,
                    InitState::Validate => InitCommand::ValidateConfig,
                    _ => InitCommand::None,
                }
            }
            Some(RecoverAction::Back) => self.back(),
            Some(RecoverAction::Skip) => {
                self.context.error = None;
                self.context.selected_index = 0;
                self.state = match error.source_state {
                    InitState::ResolveCapability => InitState::Complete,
                    InitState::Validate => InitState::Complete,
                    _ => previous_state(error.source_state),
                };
                if self.state == InitState::Complete {
                    InitCommand::Complete
                } else {
                    InitCommand::None
                }
            }
            Some(RecoverAction::Cancel) | None => self.cancel(),
        }
    }

    fn enter_assignment_state(&mut self, state: InitState) -> InitCommand {
        self.state = state;
        self.context.selected_index = 0;
        InitCommand::None
    }

    fn enter_error(
        &mut self,
        source_state: InitState,
        message: impl Into<String>,
        recover_options: Vec<RecoverAction>,
    ) {
        let message = message.into();
        self.context
            .message_log
            .push(InitMessage::Error(message.clone()));
        self.context.error = Some(InitErrorState {
            source_state,
            message,
            recover_options,
        });
        self.state = InitState::Error;
        self.context.selected_index = 0;
    }

    fn toggle_index(&mut self, index: usize) {
        if self.state != InitState::ModelImportSelect {
            return;
        }
        if index >= self.context.fetched_models.len() {
            return;
        }
        if let Some(pos) = self
            .context
            .selected_model_indices
            .iter()
            .position(|selected| *selected == index)
        {
            self.context.selected_model_indices.remove(pos);
        } else {
            self.context.selected_model_indices.push(index);
            self.context.selected_model_indices.sort_unstable();
        }
    }

    fn assignment_command(&self) -> Option<InitCommand> {
        let plan = self.context.plan_model.clone()?;
        let execute = self.context.execute_model.clone()?;
        let review = self.context.review_model.clone()?;
        Some(InitCommand::WriteModelAssignment {
            plan,
            execute,
            review,
        })
    }

    fn state_for_first_issue(&self) -> InitState {
        self.context
            .issues
            .first()
            .map(|issue| match issue.section {
                InitConfigSection::Language => InitState::SelectLanguage,
                InitConfigSection::ModelPool => InitState::ConfigureModelPool,
                InitConfigSection::ModelAssignment => InitState::ConfigureModelAssignment,
                InitConfigSection::Soul => InitState::ConfigureSoul,
                InitConfigSection::Capability | InitConfigSection::Workspace => InitState::Complete,
            })
            .unwrap_or(InitState::Validate)
    }

    fn next_unfinished_state(&self) -> InitState {
        if self.context.language.is_none() {
            InitState::SelectLanguage
        } else if self.context.model_pool.is_empty() {
            InitState::ConfigureModelPool
        } else if self.assignment_command().is_none() {
            InitState::ConfigureModelAssignment
        } else if self.context.selected_soul.is_none() {
            InitState::ConfigureSoul
        } else {
            InitState::Complete
        }
    }

    fn selected_value(&self) -> Option<String> {
        match self.action_panel() {
            ActionPanel::SingleChoice {
                options, selected, ..
            }
            | ActionPanel::Summary {
                options, selected, ..
            } => options.get(selected).map(|option| option.value.clone()),
            _ => None,
        }
    }

    fn selected_recover_action(&self, error: &InitErrorState) -> Option<RecoverAction> {
        error
            .recover_options
            .get(self.context.selected_index)
            .copied()
    }

    fn selected_model_provider_choice(&self) -> Option<ModelProviderChoice> {
        model_provider_choices()
            .get(self.context.selected_index)
            .copied()
    }

    fn selected_models_for_import(&self) -> Vec<ModelInfo> {
        self.context
            .selected_model_indices
            .iter()
            .filter_map(|index| self.context.fetched_models.get(*index).cloned())
            .collect()
    }

    fn max_selected_index(&self) -> usize {
        match self.action_panel() {
            ActionPanel::SingleChoice { options, .. } | ActionPanel::Summary { options, .. } => {
                options.len().saturating_sub(1)
            }
            ActionPanel::MultiChoice { options, .. } => options.len().saturating_sub(1),
            _ => 0,
        }
    }

    fn header_text(&self) -> String {
        match self.state {
            InitState::Complete => "Alius  ✓ configuration complete".to_string(),
            _ => "Alius  ✕ configuration incomplete".to_string(),
        }
    }

    fn footer_text(&self) -> String {
        match &self.context.git {
            Some(git) => match &git.branch {
                Some(branch) => format!("cwd: {}  git: {}", self.context.cwd.display(), branch),
                None => format!("cwd: {}  git", self.context.cwd.display()),
            },
            None => format!("cwd: {}", self.context.cwd.display()),
        }
    }

    fn render_messages(&self) -> Vec<RenderedMessage> {
        let mut messages = self
            .context
            .message_log
            .iter()
            .map(|message| match message {
                InitMessage::Info(text) => RenderedMessage::new("●", text),
                InitMessage::Success(text) => RenderedMessage::new("✓", text),
                InitMessage::Warning(text) => RenderedMessage::new("!", text),
                InitMessage::Error(text) => RenderedMessage::new("!", text),
                InitMessage::Running(text) => RenderedMessage::new("○", text),
            })
            .collect::<Vec<_>>();
        if messages.is_empty() {
            messages.push(RenderedMessage::new(
                "●",
                "Welcome to Alius. This workspace is not fully initialized yet.",
            ));
        }
        messages
    }

    fn render_check_items(&self) -> Vec<RenderedCheckItem> {
        let active = self.checklist_state();
        INIT_CHECK_ITEMS
            .iter()
            .enumerate()
            .map(|(index, (state, title))| {
                let status = if self.state == InitState::Complete {
                    CheckItemStatus::Done
                } else if *state == active {
                    CheckItemStatus::Active
                } else if checklist_index(*state) < checklist_index(active) {
                    CheckItemStatus::Done
                } else {
                    CheckItemStatus::Pending
                };
                RenderedCheckItem {
                    index: index + 1,
                    title: (*title).to_string(),
                    status,
                }
            })
            .collect()
    }

    fn action_panel(&self) -> ActionPanel {
        match self.state {
            InitState::Start => single(
                "Start Initialization",
                vec![
                    option("Start Initialization", "start"),
                    option("Exit", "exit"),
                ],
                self.context.selected_index,
                "Up/Down choose. Enter confirms.",
            ),
            InitState::Resume => single(
                "Detected unfinished initialization",
                vec![
                    option("Continue Previous", "continue"),
                    option("Restart", "restart"),
                    option("Exit", "exit"),
                ],
                self.context.selected_index,
                "Up/Down choose. Enter confirms.",
            ),
            InitState::CheckWorkspace | InitState::ModelFetchList => ActionPanel::None,
            InitState::CreateProject => single(
                "Project structure",
                vec![
                    option("Reinitialize .alius Defaults", "reinitialize_project"),
                    option("Exit", "exit"),
                ],
                self.context.selected_index,
                "Up/Down choose. Enter confirms.",
            ),
            InitState::SelectLanguage => single(
                "Choose interface language",
                vec![
                    option("Chinese (Simplified) - zh-CN", "zh-CN"),
                    option("English - en", "en"),
                    option("Japanese - ja", "ja"),
                    option("Follow System", "system"),
                    option("Back", "back"),
                    option("Exit", "exit"),
                ],
                self.context.selected_index,
                "Up/Down choose. Enter confirms. Esc returns.",
            ),
            InitState::ConfigureModelPool => {
                let mut options = if self.context.model_pool.is_empty() {
                    vec![
                        option("Add Model", "add_model"),
                        option("Configure Later", "configure_later"),
                    ]
                } else {
                    vec![
                        option("Continue to Model Assignment", "continue_assignment"),
                        option("Add Another Model", "add_model"),
                    ]
                };
                options.push(option("Back", "back"));
                options.push(option("Exit", "exit"));
                single(
                    "Model pool",
                    options,
                    self.context.selected_index,
                    "Up/Down choose. Enter confirms.",
                )
            }
            InitState::ModelSelectProvider => single(
                "Choose model provider and API",
                model_provider_choices()
                    .iter()
                    .map(|choice| option(choice.label, choice.value))
                    .collect(),
                self.context.selected_index,
                "Up/Down choose. Enter confirms. Esc returns.",
            ),
            InitState::ModelInputBaseUrl => ActionPanel::TextInput {
                title: "Enter Base URL".to_string(),
                value: self.context.base_url.clone().unwrap_or_default(),
                placeholder: "https://api.example.com/v1".to_string(),
                hint: "Enter confirms. Esc returns.".to_string(),
                masked: false,
            },
            InitState::ModelInputApiKey => ActionPanel::TextInput {
                title: "Enter API Key".to_string(),
                value: String::new(),
                placeholder: "API Key".to_string(),
                hint: "Enter confirms. Esc returns.".to_string(),
                masked: false,
            },
            InitState::ModelImportSelect => ActionPanel::MultiChoice {
                title: format!("Found {} model(s)", self.context.fetched_models.len()),
                options: self
                    .context
                    .fetched_models
                    .iter()
                    .enumerate()
                    .map(|(index, model)| MultiChoiceOption {
                        label: model.display_name.clone(),
                        value: model.id.clone(),
                        selected: self.context.selected_model_indices.contains(&index),
                    })
                    .collect(),
                highlighted: self.context.selected_index,
                hint: "Space toggles. Enter imports selected. Esc returns.".to_string(),
            },
            InitState::ConfigureModelAssignment => {
                let mut options = vec![
                    option(
                        &format!(
                            "Plan Model    {}",
                            self.model_label(self.context.plan_model.as_deref())
                        ),
                        "plan",
                    ),
                    option(
                        &format!(
                            "Execute Model    {}",
                            self.model_label(self.context.execute_model.as_deref())
                        ),
                        "execute",
                    ),
                    option(
                        &format!(
                            "Review Model    {}",
                            self.model_label(self.context.review_model.as_deref())
                        ),
                        "review",
                    ),
                ];
                if self.assignment_command().is_some() {
                    options.push(option("Continue to Role", "continue_soul"));
                }
                options.push(option("Back", "back"));
                options.push(option("Exit", "exit"));
                single(
                    "Plan / Execute / Review",
                    options,
                    self.context.selected_index,
                    "Up/Down choose. Enter confirms.",
                )
            }
            InitState::SelectPlanModel
            | InitState::SelectExecuteModel
            | InitState::SelectReviewModel => {
                let mut options = self
                    .context
                    .model_pool
                    .iter()
                    .map(|model| option(&model.display_name, &model.id))
                    .collect::<Vec<_>>();
                options.push(option("Back", "back"));
                single(
                    self.assignment_title(),
                    options,
                    self.context.selected_index,
                    "Up/Down choose. Enter assigns.",
                )
            }
            InitState::ConfigureSoul => {
                let mut options = self
                    .context
                    .soul_choices
                    .iter()
                    .map(|soul| option(&format!("{} - {}", soul.id, soul.description), &soul.id))
                    .collect::<Vec<_>>();
                options.push(option("Back", "back"));
                options.push(option("Exit", "exit"));
                single(
                    "Choose Role",
                    options,
                    self.context.selected_index,
                    "Up/Down choose. Enter confirms.",
                )
            }
            InitState::ResolveCapability => ActionPanel::None,
            InitState::CreateWorkspace => single(
                "Create workspace",
                vec![
                    option("Create Workspace", "create_workspace"),
                    option("Back", "back"),
                    option("Exit", "exit"),
                ],
                self.context.selected_index,
                "Up/Down choose. Enter confirms.",
            ),
            InitState::Validate => {
                let options = if self.context.issues.is_empty() {
                    vec![
                        option("Run Final Validation", "run_validate"),
                        option("Back", "back"),
                        option("Exit", "exit"),
                    ]
                } else {
                    vec![
                        option("Return to Fix", "fix_validation"),
                        option("Ignore and Complete", "ignore_validation"),
                        option("Exit", "exit"),
                    ]
                };
                single(
                    "Final validation",
                    options,
                    self.context.selected_index,
                    "Up/Down choose. Enter confirms.",
                )
            }
            InitState::Complete => ActionPanel::Summary {
                title: "Initialization complete".to_string(),
                lines: self.summary().lines().map(str::to_string).collect(),
                options: Vec::new(),
                selected: self.context.selected_index,
                hint: "Working.".to_string(),
            },
            InitState::Error => {
                let options = self
                    .context
                    .error
                    .as_ref()
                    .map(|error| {
                        error
                            .recover_options
                            .iter()
                            .map(|action| recover_option(*action))
                            .collect()
                    })
                    .unwrap_or_else(|| vec![option("Exit", "cancel")]);
                single(
                    "Recover",
                    options,
                    self.context.selected_index,
                    "Up/Down choose. Enter confirms.",
                )
            }
            InitState::Cancelled => single(
                "Cancelled",
                vec![option("Exit", "exit")],
                self.context.selected_index,
                "Enter confirms.",
            ),
        }
    }

    /// The configurable stage the user is currently in (for the nav bar highlight).
    /// Returns `None` for automatic/pre/post stages.
    pub fn current_stage(&self) -> Option<InitStage> {
        match self.state {
            InitState::SelectLanguage => Some(InitStage::Language),
            InitState::ConfigureModelPool
            | InitState::ModelSelectProvider
            | InitState::ModelInputBaseUrl
            | InitState::ModelInputApiKey
            | InitState::ModelFetchList
            | InitState::ModelImportSelect => Some(InitStage::ModelPool),
            InitState::ConfigureModelAssignment
            | InitState::SelectPlanModel
            | InitState::SelectExecuteModel
            | InitState::SelectReviewModel => Some(InitStage::Assignment),
            InitState::ConfigureSoul => Some(InitStage::Soul),
            _ => None,
        }
    }

    /// Whether a stage's data is already set (for the nav bar ✓ marker).
    pub fn stage_done(&self, stage: InitStage) -> bool {
        match stage {
            InitStage::Language => self.context.language.is_some(),
            InitStage::ModelPool => !self.context.model_pool.is_empty(),
            InitStage::Assignment => {
                self.context.plan_model.is_some()
                    && self.context.execute_model.is_some()
                    && self.context.review_model.is_some()
            }
            InitStage::Soul => self.context.selected_soul.is_some(),
        }
    }

    fn checklist_state(&self) -> InitState {
        match self.state {
            InitState::Start | InitState::Resume | InitState::CheckWorkspace => {
                InitState::CheckWorkspace
            }
            InitState::CreateProject => InitState::CreateProject,
            InitState::SelectLanguage => InitState::SelectLanguage,
            InitState::ConfigureModelPool
            | InitState::ModelSelectProvider
            | InitState::ModelInputBaseUrl
            | InitState::ModelInputApiKey
            | InitState::ModelFetchList
            | InitState::ModelImportSelect => InitState::ConfigureModelPool,
            InitState::ConfigureModelAssignment
            | InitState::SelectPlanModel
            | InitState::SelectExecuteModel
            | InitState::SelectReviewModel => InitState::ConfigureModelAssignment,
            InitState::ConfigureSoul
            | InitState::ResolveCapability
            | InitState::CreateWorkspace
            | InitState::Validate
            | InitState::Complete => InitState::ConfigureSoul,
            InitState::Error => self
                .context
                .error
                .as_ref()
                .map(|error| error.source_state)
                .unwrap_or(InitState::CheckWorkspace),
            InitState::Cancelled => InitState::CheckWorkspace,
        }
    }

    fn scope_title(&self) -> &'static str {
        match self.state {
            InitState::Start => "init-start",
            InitState::Resume => "resume",
            InitState::CheckWorkspace => "check-workspace",
            InitState::CreateProject => "create-project",
            InitState::SelectLanguage => "select-language",
            InitState::ConfigureModelPool
            | InitState::ModelSelectProvider
            | InitState::ModelInputBaseUrl
            | InitState::ModelInputApiKey
            | InitState::ModelFetchList
            | InitState::ModelImportSelect => "configure-model-pool",
            InitState::ConfigureModelAssignment
            | InitState::SelectPlanModel
            | InitState::SelectExecuteModel
            | InitState::SelectReviewModel => "configure-assignment",
            InitState::ConfigureSoul => "configure-soul",
            InitState::ResolveCapability => "complete",
            InitState::CreateWorkspace => "create-workspace",
            InitState::Validate => "validate",
            InitState::Complete => "complete",
            InitState::Error => "error",
            InitState::Cancelled => "cancelled",
        }
    }

    fn assignment_title(&self) -> &'static str {
        match self.state {
            InitState::SelectPlanModel => "Plan Model",
            InitState::SelectExecuteModel => "Execute Model",
            InitState::SelectReviewModel => "Review Model",
            _ => "Model",
        }
    }

    fn model_label(&self, model_id: Option<&str>) -> String {
        let Some(id) = model_id.filter(|id| !id.trim().is_empty()) else {
            return "not configured".to_string();
        };
        self.context
            .model_pool
            .iter()
            .find(|model| model.id == id)
            .map(|model| model.display_name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    fn summary(&self) -> String {
        format!(
            "Role: {}\nPlan: {}\nExecute: {}\nReview: {}",
            self.context
                .selected_soul
                .clone()
                .unwrap_or_else(|| "not configured".to_string()),
            self.model_label(self.context.plan_model.as_deref()),
            self.model_label(self.context.execute_model.as_deref()),
            self.model_label(self.context.review_model.as_deref()),
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InitViewModel {
    pub header: String,
    pub messages: Vec<RenderedMessage>,
    pub check_items: Vec<RenderedCheckItem>,
    pub action_panel: ActionPanel,
    pub footer: String,
    pub scope_title: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedMessage {
    pub marker: String,
    pub text: String,
}

impl RenderedMessage {
    fn new(marker: &str, text: impl Into<String>) -> Self {
        Self {
            marker: marker.to_string(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RenderedCheckItem {
    pub index: usize,
    pub title: String,
    pub status: CheckItemStatus,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionPanel {
    SingleChoice {
        title: String,
        options: Vec<ActionOption>,
        selected: usize,
        hint: String,
    },
    MultiChoice {
        title: String,
        options: Vec<MultiChoiceOption>,
        highlighted: usize,
        hint: String,
    },
    TextInput {
        title: String,
        value: String,
        placeholder: String,
        hint: String,
        masked: bool,
    },
    Summary {
        title: String,
        lines: Vec<String>,
        options: Vec<ActionOption>,
        selected: usize,
        hint: String,
    },
    None,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionOption {
    pub label: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiChoiceOption {
    pub label: String,
    pub value: String,
    pub selected: bool,
}

const INIT_CHECK_ITEMS: &[(InitState, &str)] = &[
    (InitState::CheckWorkspace, "Check workspace"),
    (InitState::CreateProject, "Initialize .alius/"),
    (InitState::SelectLanguage, "Choose language"),
    (InitState::ConfigureModelPool, "Configure model pool"),
    (
        InitState::ConfigureModelAssignment,
        "Configure Plan/Execute/Review",
    ),
    (InitState::ConfigureSoul, "Configure Role"),
];

#[derive(Debug, Clone, Copy)]
struct ModelProviderChoice {
    label: &'static str,
    value: &'static str,
    provider: &'static str,
    api_protocol: ApiProtocol,
    base_url: &'static str,
}

fn model_provider_choices() -> &'static [ModelProviderChoice] {
    &[
        ModelProviderChoice {
            label: "BigModel GLM (Coding Plan) - OpenAI API",
            value: "bigmodel-openai",
            provider: "bigmodel",
            api_protocol: ApiProtocol::OpenAi,
            base_url: "https://open.bigmodel.cn/api/coding/paas/v4",
        },
        ModelProviderChoice {
            label: "BigModel GLM (Coding Plan) - Anthropic API",
            value: "bigmodel-anthropic",
            provider: "bigmodel",
            api_protocol: ApiProtocol::Anthropic,
            base_url: "https://open.bigmodel.cn/api/anthropic",
        },
        ModelProviderChoice {
            label: "Xiaomi MiMo (Token Plan) - OpenAI API",
            value: "xiaomi-mimo-openai",
            provider: "xiaomi_mimo",
            api_protocol: ApiProtocol::OpenAi,
            base_url: "https://api.xiaomimimo.com/v1",
        },
        ModelProviderChoice {
            label: "Xiaomi MiMo (Token Plan) - Anthropic API",
            value: "xiaomi-mimo-anthropic",
            provider: "xiaomi_mimo",
            api_protocol: ApiProtocol::Anthropic,
            base_url: "https://api.xiaomimimo.com/anthropic",
        },
        ModelProviderChoice {
            label: "DeepSeek - OpenAI API",
            value: "deepseek-openai",
            provider: "deepseek",
            api_protocol: ApiProtocol::OpenAi,
            base_url: "https://api.deepseek.com",
        },
        ModelProviderChoice {
            label: "DeepSeek - Anthropic API",
            value: "deepseek-anthropic",
            provider: "deepseek",
            api_protocol: ApiProtocol::Anthropic,
            base_url: "https://api.deepseek.com/anthropic",
        },
    ]
}

fn previous_state(state: InitState) -> InitState {
    match state {
        InitState::CreateProject => InitState::Start,
        InitState::SelectLanguage => InitState::CreateProject,
        InitState::ConfigureModelPool => InitState::SelectLanguage,
        InitState::ModelSelectProvider => InitState::ConfigureModelPool,
        InitState::ModelInputBaseUrl => InitState::ModelSelectProvider,
        InitState::ModelInputApiKey => InitState::ModelInputBaseUrl,
        InitState::ModelImportSelect => InitState::ModelInputApiKey,
        InitState::ConfigureModelAssignment => InitState::ConfigureModelPool,
        InitState::SelectPlanModel
        | InitState::SelectExecuteModel
        | InitState::SelectReviewModel => InitState::ConfigureModelAssignment,
        InitState::ConfigureSoul => InitState::ConfigureModelAssignment,
        InitState::ResolveCapability | InitState::CreateWorkspace | InitState::Validate => {
            InitState::ConfigureSoul
        }
        _ => InitState::Start,
    }
}

fn checklist_index(state: InitState) -> usize {
    INIT_CHECK_ITEMS
        .iter()
        .position(|(item_state, _)| *item_state == state)
        .unwrap_or(0)
}

fn valid_url(value: &str) -> bool {
    value.starts_with("https://") || value.starts_with("http://")
}

fn single(
    title: impl Into<String>,
    options: Vec<ActionOption>,
    selected: usize,
    hint: impl Into<String>,
) -> ActionPanel {
    ActionPanel::SingleChoice {
        title: title.into(),
        selected: selected.min(options.len().saturating_sub(1)),
        options,
        hint: hint.into(),
    }
}

fn option(label: impl Into<String>, value: impl Into<String>) -> ActionOption {
    ActionOption {
        label: label.into(),
        value: value.into(),
    }
}

fn recover_option(action: RecoverAction) -> ActionOption {
    match action {
        RecoverAction::Retry => option("Retry", "retry"),
        RecoverAction::Back => option("Back", "back"),
        RecoverAction::Skip => option("Skip", "skip"),
        RecoverAction::Cancel => option("Exit", "cancel"),
    }
}

pub fn model_info_from_library(entry: &ModelLibraryEntry) -> ModelInfo {
    ModelInfo {
        id: entry.id.clone(),
        display_name: entry.display_name.clone(),
        provider: entry.provider.clone(),
        api_protocol: if entry.base_url.contains("/anthropic") {
            ApiProtocol::Anthropic
        } else {
            ApiProtocol::OpenAi
        },
        base_url: entry.base_url.clone(),
        model_name: entry.model_name.clone(),
    }
}

pub fn role_from_assignment_state(state: InitState) -> Option<ModelAssignmentRole> {
    match state {
        InitState::SelectPlanModel => Some(ModelAssignmentRole::Plan),
        InitState::SelectExecuteModel => Some(ModelAssignmentRole::Execute),
        InitState::SelectReviewModel => Some(ModelAssignmentRole::Review),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wizard() -> InitWizard {
        InitWizard::new(PathBuf::from("/tmp/project"))
    }

    #[test]
    fn start_confirm_goes_to_check_workspace() {
        let mut wizard = wizard();

        let command = wizard.handle_event(InitEvent::Confirm);

        assert_eq!(wizard.state, InitState::CheckWorkspace);
        assert_eq!(command, InitCommand::CheckWorkspace);
    }

    #[test]
    fn workspace_ok_goes_to_create_project() {
        let mut wizard = wizard();
        wizard.state = InitState::CheckWorkspace;

        let command = wizard.handle_event(InitEvent::WorkspaceChecked(WorkspaceCheckResult {
            ok: true,
            git: Some(GitStatus {
                branch: Some("main".to_string()),
            }),
            message: None,
            alius_existed: true,
        }));

        assert_eq!(wizard.state, InitState::CreateProject);
        assert_eq!(command, InitCommand::None);
        assert!(wizard.context.git.is_some());
    }

    #[test]
    fn workspace_failure_enters_error() {
        let mut wizard = wizard();
        wizard.state = InitState::CheckWorkspace;

        wizard.handle_event(InitEvent::WorkspaceChecked(WorkspaceCheckResult {
            ok: false,
            git: None,
            message: Some("no permission".to_string()),
            alius_existed: true,
        }));

        assert_eq!(wizard.state, InitState::Error);
        assert_eq!(
            wizard
                .context
                .error
                .as_ref()
                .map(|error| error.source_state),
            Some(InitState::CheckWorkspace)
        );
    }

    #[test]
    fn select_language_returns_write_command() {
        let mut wizard = wizard();
        wizard.state = InitState::SelectLanguage;
        wizard.handle_event(InitEvent::Select(1));

        let command = wizard.handle_event(InitEvent::Confirm);

        assert_eq!(wizard.state, InitState::ConfigureModelPool);
        assert_eq!(
            command,
            InitCommand::WriteLanguageConfig {
                locale: "en".to_string()
            }
        );
    }

    #[test]
    fn model_fetch_failure_enters_api_key_error() {
        let mut wizard = wizard();
        wizard.state = InitState::ModelFetchList;

        wizard.handle_event(InitEvent::AsyncFailed("bad key".to_string()));

        assert_eq!(wizard.state, InitState::Error);
        assert_eq!(
            wizard
                .context
                .error
                .as_ref()
                .map(|error| error.source_state),
            Some(InitState::ModelInputApiKey)
        );
    }

    #[test]
    fn models_fetched_go_to_import_select() {
        let mut wizard = wizard();
        wizard.state = InitState::ModelFetchList;

        wizard.handle_event(InitEvent::ModelsFetched(vec![ModelInfo {
            id: "m1".to_string(),
            display_name: "m1".to_string(),
            provider: "bigmodel".to_string(),
            api_protocol: ApiProtocol::OpenAi,
            base_url: "https://example.com/v1".to_string(),
            model_name: "m1".to_string(),
        }]));

        assert_eq!(wizard.state, InitState::ModelImportSelect);
        assert_eq!(wizard.context.selected_model_indices, vec![0]);
    }

    #[test]
    fn validation_passed_goes_complete() {
        let mut wizard = wizard();
        wizard.state = InitState::Validate;

        wizard.handle_event(InitEvent::ValidationPassed);

        assert_eq!(wizard.state, InitState::Complete);
    }

    #[test]
    fn validation_failed_enters_error() {
        let mut wizard = wizard();
        wizard.state = InitState::Validate;

        wizard.handle_event(InitEvent::ValidationFailed(vec![ConfigIssue {
            message: "Review Model is not configured.".to_string(),
            section: InitConfigSection::ModelAssignment,
        }]));

        assert_eq!(wizard.state, InitState::Error);
        assert_eq!(wizard.context.issues.len(), 1);
    }

    #[test]
    fn legacy_capability_state_completes_without_visible_step() {
        let mut wizard = wizard();
        wizard.state = InitState::ResolveCapability;

        let command = wizard.handle_event(InitEvent::Confirm);

        assert_eq!(wizard.state, InitState::Complete);
        assert_eq!(command, InitCommand::Complete);
        let titles = wizard
            .view_model()
            .check_items
            .into_iter()
            .map(|item| item.title)
            .collect::<Vec<_>>();
        assert!(!titles.iter().any(|title| title == "Resolve Capability"));
        assert!(!titles.iter().any(|title| title == "Create workspace"));
        assert!(!titles.iter().any(|title| title == "Validate configuration"));
    }

    #[test]
    fn complete_view_has_no_final_mode_choice() {
        let mut wizard = wizard();
        wizard.state = InitState::Complete;

        let vm = wizard.view_model();

        let ActionPanel::Summary { options, .. } = vm.action_panel else {
            panic!("expected complete summary panel");
        };
        assert!(options.is_empty());
    }

    #[test]
    fn start_view_model_uses_single_choice() {
        let wizard = wizard();
        let vm = wizard.view_model();

        assert_eq!(vm.scope_title, "init-start");
        assert!(matches!(vm.action_panel, ActionPanel::SingleChoice { .. }));
    }

    #[test]
    fn api_key_view_model_is_plaintext() {
        let mut wizard = wizard();
        wizard.state = InitState::ModelInputApiKey;
        let vm = wizard.view_model();

        assert!(matches!(
            vm.action_panel,
            ActionPanel::TextInput { masked: false, .. }
        ));
    }
}
