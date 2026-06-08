use runtime_config::{
    load_project_config, ModelAssignmentConfig, ModelAssignmentRole, ModelLibraryEntry,
    ProviderConfig, ProviderMode, ProviderSettings, ProviderType, ReasoningNote, Settings,
    SoulRole, TierConfig,
};
use runtime_model::LlmClient;
use rust_i18n::t;

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
}

#[derive(Debug, Clone)]
pub enum ConfigTaskOutcome {
    Next {
        accepted: String,
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
enum ConfigSection {
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

        let soul_choices = crate::formula::list_installed_souls().unwrap_or_default();
        let mut task = Self {
            draft: settings,
            providers,
            assignment,
            section: ConfigSection::ModelAssignment,
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
        if kind != ConfigTaskKind::ModelPool {
            task.jump_to_first_missing();
        }
        task
    }

    pub fn save_target(&self) -> ConfigSaveTarget {
        ConfigSaveTarget::Project
    }

    pub fn request_label(&self) -> &'static str {
        match self.kind {
            ConfigTaskKind::Config => "/config",
            ConfigTaskKind::Init => "/init",
            ConfigTaskKind::ModelPool => "/model",
        }
    }

    pub fn cancel_request_label(&self) -> String {
        match self.kind {
            ConfigTaskKind::Config => "Exit configuration".to_string(),
            ConfigTaskKind::Init => t!("workspace.init_task.cancel_request").to_string(),
            ConfigTaskKind::ModelPool => "Exit model pool".to_string(),
        }
    }

    pub fn prompt(&self) -> ConfigPrompt {
        ConfigPrompt {
            message: self.message(),
            input: self.input_state(),
        }
    }

    pub fn switch_tab(&mut self, reverse: bool) -> ConfigPrompt {
        if self.kind == ConfigTaskKind::ModelPool {
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

        if self.kind == ConfigTaskKind::ModelPool {
            return self.submit_model_pool(raw);
        }

        match self.section {
            ConfigSection::ModelAssignment => self.submit_model_assignment(raw),
            ConfigSection::Language => self.submit_language(raw),
            ConfigSection::Soul => self.submit_soul(raw),
        }
    }

    pub fn display_answer(&self, input: &str) -> String {
        let value = input.trim();
        if self.kind == ConfigTaskKind::ModelPool {
            if let Some(add) = &self.add_model {
                return if add.step == AddModelStep::ApiKey {
                    "API Key: configured".to_string()
                } else {
                    format!("{}: {}", add.step.label(), value)
                };
            }
            return format!("Model pool: {}", self.choice_label(value));
        }
        match self.section {
            ConfigSection::ModelAssignment => {
                if let Some(role) = self.assignment_role {
                    format!("{}: {}", role.label(), self.model_label(value))
                } else {
                    format!("Model assignment: {}", self.choice_label(value))
                }
            }
            ConfigSection::Language => format!("Language: {}", value),
            ConfigSection::Soul => format!("SOUL: {}", value),
        }
    }

    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    fn message(&self) -> String {
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
            ConfigSection::Language => vec!["Choose the workspace language.".to_string()],
            ConfigSection::Soul => vec!["Choose the active SOUL.".to_string()],
        });
        lines.join("\n")
    }

    fn input_state(&self) -> PromptInputState {
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
                    message: "Choose an enabled model from the model pool.".to_string(),
                    prompt: self.prompt(),
                };
            };
            self.assignment.set(role, entry.id.clone());
            self.sync_assignment_compat();
            self.assignment_role = None;
            self.dirty = true;
            return ConfigTaskOutcome::Next {
                accepted: format!("{}: {}", role.label(), model_entry_label(&entry)),
                prompt: self.prompt(),
            };
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
                message: "Model pool is managed by /model. Run /model to add models first."
                    .to_string(),
                prompt: self.prompt(),
            };
        }
        let Some(role) = parse_assignment_role(&value) else {
            return ConfigTaskOutcome::Invalid {
                message: "Choose Plan Model, Execute Model, or Review Model.".to_string(),
                prompt: self.prompt(),
            };
        };
        if self.enabled_models().is_empty() {
            return ConfigTaskOutcome::Invalid {
                message: "Current model pool is empty. Run /model to add models first.".to_string(),
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
                message: "Choose a supported language.".to_string(),
                prompt: self.prompt(),
            };
        };
        self.draft.ui.locale = locale.to_string();
        crate::set_locale(locale);
        self.dirty = true;
        ConfigTaskOutcome::Next {
            accepted: format!("Language: {}", locale_display(locale)),
            prompt: self.prompt(),
        }
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
                message: "Choose a SOUL.".to_string(),
                prompt: self.prompt(),
            };
        }
        if self.soul_choices.iter().any(|formula| formula.id == value) {
            let _ = crate::formula::activate_soul(&value);
        }
        self.draft.soul.role = SoulRole::new(value.clone());
        self.dirty = true;
        ConfigTaskOutcome::Next {
            accepted: format!("SOUL: {value}"),
            prompt: self.prompt(),
        }
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
                message: "Choose Add Model, View Model, Delete Model, or Save.".to_string(),
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
            message: "Choose a model to view.".to_string(),
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
                message: "Choose a model to delete.".to_string(),
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
                message: "Choose Confirm Delete or Cancel.".to_string(),
                prompt: self.prompt(),
            };
        }

        let id = self.pending_delete_model.take().unwrap_or_default();
        if id.is_empty() {
            self.pool_mode = ModelPoolMode::DeleteSelect;
            return ConfigTaskOutcome::Invalid {
                message: "No model is selected for deletion.".to_string(),
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
                        message: "Choose OpenAI API or Anthropic API.".to_string(),
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
                        message:
                            "Base URL format error: missing protocol. Example: https://api.example.com/v1"
                                .to_string(),
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
                        message:
                            "Failed to fetch model list. Check API Key or Base URL and try again."
                                .to_string(),
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
                        message: "Select at least one model.".to_string(),
                        prompt: self.prompt(),
                    };
                }
                add.selected_models = selected;
                let added = self.save_added_models(&add);
                self.add_model = None;
                self.dirty = true;
                ConfigTaskOutcome::Next {
                    accepted: format!("Added {} model(s) to the model pool.", added),
                    prompt: self.prompt(),
                }
            }
        }
    }

    fn save_config(&mut self) -> ConfigTaskOutcome {
        if self.kind != ConfigTaskKind::ModelPool {
            if let Some(issue) = self.first_issue() {
                self.jump_to_issue(issue.clone());
                return ConfigTaskOutcome::Invalid {
                    message: issue.message,
                    prompt: self.prompt(),
                };
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
                message: "Configuration is incomplete. Model pool has no enabled model. Run /model to add one.".to_string(),
                section: ConfigSection::ModelAssignment,
            });
        }
        for role in ModelAssignmentRole::all() {
            let model_id = self.assignment.get(role).trim();
            if model_id.is_empty() {
                issues.push(ConfigIssue {
                    message: format!("{} is not configured.", role.label()),
                    section: ConfigSection::ModelAssignment,
                });
            } else if self.model_entry(model_id).is_none() {
                issues.push(ConfigIssue {
                    message: format!(
                        "{} references '{}' but it is not in the enabled model pool.",
                        role.label(),
                        model_id
                    ),
                    section: ConfigSection::ModelAssignment,
                });
            }
        }
        if self.draft.soul.role.as_str().trim().is_empty() {
            issues.push(ConfigIssue {
                message: "Configuration is incomplete. Please choose a SOUL.".to_string(),
                section: ConfigSection::Soul,
            });
        }
        if self.draft.ui.locale.trim().is_empty() {
            issues.push(ConfigIssue {
                message: "Configuration is incomplete. Please choose a language.".to_string(),
                section: ConfigSection::Language,
            });
        }
        issues
    }

    fn jump_to_first_missing(&mut self) {
        if let Some(issue) = self.first_issue() {
            self.jump_to_issue(issue);
        }
    }

    fn jump_to_issue(&mut self, issue: ConfigIssue) {
        self.section = issue.section;
        self.missing_notice = Some(issue.message);
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

    fn choice_label(&self, value: &str) -> String {
        self.input_state()
            .choices
            .iter()
            .find(|choice| choice.value == value)
            .map(|choice| choice.label.clone())
            .unwrap_or_else(|| value.to_string())
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
                if section == self.section {
                    format!("[{}]", section.label())
                } else {
                    section.label().to_string()
                }
            })
            .collect::<Vec<_>>()
            .join("  ")
    }

    fn assignment_lines(&self) -> Vec<String> {
        let mut lines = vec![
            "Plan-Execute-Review model assignment.".to_string(),
            "Assignments are selected from the /model model pool only.".to_string(),
            String::new(),
            "Current assignment:".to_string(),
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
            lines.push("Current model pool is empty. Run /model to add models.".to_string());
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
            lines.push(add.step.question().to_string());
            if add.fetch_failed {
                lines.push(
                    "Failed to fetch model list. Check API Key or Base URL and try again."
                        .to_string(),
                );
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

    fn model_label_or_unconfigured(&self, id: &str) -> String {
        if id.trim().is_empty() {
            "not configured".to_string()
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
                "Up/Down choose model. Enter assigns this role. Esc cancels.",
            )
            .with_highlighted_value(self.assignment.get(role));
        }
        prompt(
            self.section.label(),
            PromptInputKind::SingleSelect,
            self.assignment_role_choices(),
            self.help(),
        )
    }

    fn language_input(&self) -> PromptInputState {
        prompt(
            self.section.label(),
            PromptInputKind::SingleSelect,
            with_config_actions(language_choices()),
            self.help(),
        )
        .with_highlighted_value(&self.draft.ui.locale)
    }

    fn soul_input(&self) -> PromptInputState {
        prompt(
            self.section.label(),
            PromptInputKind::SingleSelect,
            with_config_actions(self.soul_choices_for_prompt()),
            self.help(),
        )
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
                "Up/Down move options. Enter confirms. Esc cancels.",
            ),
            ModelPoolMode::ViewSelect => prompt(
                "view-model",
                PromptInputKind::SingleSelect,
                with_back(self.enabled_model_choices()),
                "Enter views model details. Esc cancels.",
            ),
            ModelPoolMode::ViewDetail => prompt(
                "model-detail",
                PromptInputKind::SingleSelect,
                vec![choice("Back", "back")],
                "Enter returns.",
            ),
            ModelPoolMode::DeleteSelect => prompt(
                "delete-model",
                PromptInputKind::SingleSelect,
                with_back(self.enabled_model_choices()),
                "Enter selects a model for deletion. Esc cancels.",
            ),
            ModelPoolMode::DeleteConfirm => prompt(
                "confirm-delete",
                PromptInputKind::SingleSelect,
                delete_confirm_choices(),
                "Enter confirms. Esc cancels.",
            ),
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
            "Enter confirms. Esc returns.",
        )
        .with_placeholder(placeholder)
        .with_input_value(value)
    }

    fn help(&self) -> String {
        "Tab/Shift+Tab switch configuration sections. Up/Down move options. Enter confirms."
            .to_string()
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
                "Model pool is empty. Run /model first",
                "model_pool",
            ));
        }
        choices.push(choice("Save Configuration", "save_config"));
        choices.push(choice("Cancel", "cancel"));
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
            choice("Add Model", "add_model"),
            choice("View Model", "view_model"),
            choice("Delete Model", "delete_model"),
            choice("Save Model Pool", "save_config"),
            choice("Cancel", "cancel"),
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
            ConfigTaskKind::ModelPool => "Model pool saved.".to_string(),
        }
    }

    fn cancelled_message(&self) -> String {
        match self.kind {
            ConfigTaskKind::Config => t!("workspace.config_task.cancelled").to_string(),
            ConfigTaskKind::Init => t!("workspace.init_task.cancelled").to_string(),
            ConfigTaskKind::ModelPool => "Model pool cancelled.".to_string(),
        }
    }
}

impl ConfigSection {
    fn all() -> [Self; 3] {
        [Self::ModelAssignment, Self::Language, Self::Soul]
    }

    fn label(self) -> &'static str {
        match self {
            Self::ModelAssignment => "configuration-models",
            Self::Language => "configuration-language",
            Self::Soul => "configuration-soul",
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
    fn label(self) -> &'static str {
        match self {
            Self::Provider => "Provider",
            Self::ApiProtocol => "API",
            Self::BaseUrl => "Base URL",
            Self::ApiKey => "API Key",
            Self::SelectModels => "Models",
        }
    }

    fn question(self) -> &'static str {
        match self {
            Self::Provider => "Choose the model provider.",
            Self::ApiProtocol => "Choose OpenAI API or Anthropic API.",
            Self::BaseUrl => "Enter or confirm the Base URL.",
            Self::ApiKey => "Enter the API Key. It is shown as plaintext and supports paste.",
            Self::SelectModels => "Select one or more models to import.",
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
    .with_scope_title("configuration-models  configuration-language  configuration-soul")
}

fn load_current_provider_config() -> ProviderConfig {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| load_project_config(&cwd).ok())
        .map(|snapshot| snapshot.providers)
        .unwrap_or_default()
}

fn load_current_model_assignment(providers: &ProviderConfig) -> ModelAssignmentConfig {
    std::env::current_dir()
        .ok()
        .and_then(|cwd| load_project_config(&cwd).ok())
        .map(|snapshot| snapshot.model_assignment)
        .unwrap_or_else(|| ModelAssignmentConfig::from_provider_tiers(providers))
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
        label: "Custom Base URL".to_string(),
        value: current.to_string(),
    });
    choices
}

fn delete_confirm_choices() -> Vec<ConfigChoice> {
    vec![
        choice("Confirm Delete", "confirm_delete"),
        choice("Cancel", "cancel_delete"),
    ]
}

fn choice(label: &str, value: &str) -> ConfigChoice {
    ConfigChoice {
        label: label.to_string(),
        value: value.to_string(),
    }
}

fn with_config_actions(mut choices: Vec<ConfigChoice>) -> Vec<ConfigChoice> {
    choices.push(choice("Save Configuration", "save_config"));
    choices.push(choice("Cancel", "cancel"));
    choices
}

fn with_back(mut choices: Vec<ConfigChoice>) -> Vec<ConfigChoice> {
    choices.push(choice("Back", "back"));
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

fn locale_display(locale: &str) -> &str {
    LOCALES
        .iter()
        .find(|(code, _)| *code == locale)
        .map(|(_, label)| *label)
        .unwrap_or(locale)
}

fn is_cancel(value: &str) -> bool {
    value.eq_ignore_ascii_case("/cancel") || value.eq_ignore_ascii_case("cancel")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

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
}
