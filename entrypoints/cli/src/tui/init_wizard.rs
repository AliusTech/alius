use crate::formula::FormulaDef;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyModifiers};
use protocol_interface::{ProviderMode, ProviderType, SoulRole};
use runtime_config::Settings;
use rust_i18n::t;

#[cfg(test)]
use crossterm::event::KeyEvent;
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};

use crate::tui::app::TuiApp;
use crate::tui::components::{
    should_process_key_event, InputAction, ListAction, SelectableList, StepIndicator, TextInput,
};
use crate::tui::theme;

const PROVIDERS: &[&str] = &["bigmodel", "xiaomi_mimo", "deepseek"];
const BIGMODEL_OPENAI_BASE_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4";
const BIGMODEL_ANTHROPIC_BASE_URL: &str = "https://open.bigmodel.cn/api/anthropic";
const XIAOMI_MIMO_OPENAI_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
const XIAOMI_MIMO_ANTHROPIC_BASE_URL: &str = "https://api.xiaomimimo.com/anthropic";
const DEEPSEEK_OPENAI_BASE_URL: &str = "https://api.deepseek.com";
const DEEPSEEK_ANTHROPIC_BASE_URL: &str = "https://api.deepseek.com/anthropic";
const CUSTOM_BASE_URL: &str = "http://localhost:8080/v1";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubMode {
    List,
    Input,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WizardStep {
    Provider,
    BaseUrl,
    ApiKey,
    Model,
    Soul,
}

fn build_steps(_provider: &ProviderType) -> Vec<WizardStep> {
    let mut steps = vec![WizardStep::Provider];
    steps.extend_from_slice(&[
        WizardStep::BaseUrl,
        WizardStep::ApiKey,
        WizardStep::Model,
        WizardStep::Soul,
    ]);
    steps
}

fn step_display_name(step: WizardStep) -> String {
    match step {
        WizardStep::Provider => t!("init.step.provider").to_string(),
        WizardStep::BaseUrl => t!("init.step.base_url").to_string(),
        WizardStep::ApiKey => t!("init.step.api_key").to_string(),
        WizardStep::Model => t!("init.step.model").to_string(),
        WizardStep::Soul => t!("init.step.soul").to_string(),
    }
}

#[derive(Clone)]
struct SoulChoice {
    formula: FormulaDef,
}

impl std::fmt::Display for SoulChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} - {}", self.formula.id, self.formula.description)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BaseUrlChoice {
    label: &'static str,
    url: Option<&'static str>,
}

impl std::fmt::Display for BaseUrlChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.url {
            Some(url) => write!(f, "{}  ({})", self.label, url),
            None => write!(f, "{}", t!("init.custom_base_url")),
        }
    }
}

/// Run the init wizard as a standalone TUI (for `alius init` from shell).
pub async fn run_init_wizard() -> Result<Option<Settings>> {
    tokio::task::spawn_blocking(|| {
        let mut app = TuiApp::enter()?;
        let result = run_wizard_loop(&mut app);
        app.restore()?;
        result
    })
    .await
    .map_err(|e| anyhow::anyhow!("{}", e))?
}

fn run_wizard_loop(app: &mut TuiApp) -> Result<Option<Settings>> {
    let locale = Settings::load()
        .ok()
        .map(|s| s.ui.locale)
        .unwrap_or_else(|| "en".to_string());
    crate::set_locale(&locale);

    let mut provider: ProviderType = ProviderType::BigModel;
    let mut provider_mode: Option<ProviderMode> = None;
    let mut base_url = String::new();
    let mut api_key = String::new();
    let mut model = String::new();
    let mut base_url_mode = SubMode::List;
    let mut model_mode = SubMode::List;

    let soul_choices = load_available_souls();
    let soul_update_available = crate::formula::check_soul_updates().unwrap_or(false);

    let mut step_idx: usize = 0;

    let mut provider_list = SelectableList::new(
        t!("init.select_provider").to_string(),
        PROVIDERS.iter().map(|s| s.to_string()).collect(),
    );
    let base_url_choices = base_url_choices_strings(&provider, &provider_mode);
    let mut base_url_list =
        SelectableList::new(t!("init.select_base_url").to_string(), base_url_choices);
    let mut base_url_custom_input = TextInput::new(t!("init.custom_base_url_label").to_string());
    let mut api_key_input = TextInput::new(t!("init.step.api_key").to_string() + ":");
    let mut model_list = SelectableList::new(
        t!("init.select_model").to_string(),
        vec![t!("init.manual_model").to_string()],
    );
    let mut model_input = TextInput::new(t!("init.step.model").to_string() + ":");
    let mut soul_list = SelectableList::new(t!("init.select_soul").to_string(), soul_choices);

    loop {
        let steps = build_steps(&provider);
        let current_step = steps.get(step_idx).copied().unwrap_or(WizardStep::Provider);

        // 2 main steps for indicator: "Configure Model" and "Choose Soul"
        let main_step_names = vec![
            t!("init.step.model_main").to_string(),
            t!("init.step.soul").to_string(),
        ];
        let main_step_idx = if current_step == WizardStep::Soul {
            1
        } else {
            0
        };
        let steps_indicator = StepIndicator::new(main_step_names);

        app.draw(|frame| {
            let area = frame.area();
            let block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    format!(" {} ", t!("init.title")),
                    theme::title(),
                ))
                .style(theme::base())
                .border_style(theme::border(true))
                .title_alignment(Alignment::Center);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            let indicator_area = Rect::new(inner.x, inner.y, inner.width, 1);
            let mut indicator = steps_indicator;
            indicator.set_current(main_step_idx);
            indicator.render(frame, indicator_area);

            // Step label at row 2
            let label_area = Rect::new(inner.x + 2, inner.y + 2, inner.width.saturating_sub(4), 1);
            let sub_name = step_display_name(current_step);
            let label_text = if current_step == WizardStep::Soul {
                format!("2/2: {}", sub_name)
            } else {
                format!("1/2: {}", sub_name)
            };
            let step_label = Paragraph::new(Span::styled(label_text, theme::emphasis()));
            frame.render_widget(step_label, label_area);

            let input_area = Rect::new(
                inner.x + 2,
                inner.y + 4,
                inner.width.saturating_sub(4),
                TextInput::HEIGHT,
            );
            let list_area = Rect::new(
                inner.x,
                inner.y + 2,
                inner.width,
                inner.height.saturating_sub(3),
            );

            let help_area = Rect::new(
                inner.x,
                inner.y + inner.height.saturating_sub(1),
                inner.width,
                1,
            );
            let help_text = if step_idx == 0 {
                t!("init.help.first_step").to_string()
            } else {
                t!("init.help.next_step").to_string()
            };
            frame.render_widget(
                Paragraph::new(Span::styled(help_text, theme::secondary())),
                help_area,
            );

            match current_step {
                WizardStep::Provider => {
                    provider_list.render(frame, list_area);
                }
                WizardStep::BaseUrl => match base_url_mode {
                    SubMode::List => {
                        base_url_list.render(frame, list_area);
                    }
                    SubMode::Input => {
                        let mut display = base_url_custom_input.clone();
                        display.set_active(true);
                        display.render(frame, input_area);
                    }
                },
                WizardStep::ApiKey => {
                    let mut display = api_key_input.clone();
                    display.set_active(true);
                    display.render(frame, input_area);
                }
                WizardStep::Model => match model_mode {
                    SubMode::List => {
                        model_list.render(frame, list_area);
                    }
                    SubMode::Input => {
                        let mut display = model_input.clone();
                        display.set_active(true);
                        display.render(frame, input_area);
                    }
                },
                WizardStep::Soul => {
                    if soul_update_available {
                        let hint_area = Rect::new(
                            inner.x + 2,
                            inner.y + inner.height.saturating_sub(2),
                            inner.width.saturating_sub(4),
                            1,
                        );
                        frame.render_widget(
                            Paragraph::new(Span::styled(
                                t!("init.soul_update_available").to_string(),
                                theme::emphasis(),
                            )),
                            hint_area,
                        );
                    }
                    soul_list.render(frame, list_area);
                }
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Key(key) => {
                    if !should_process_key_event(&key) {
                        continue;
                    }
                    let is_back = key.code == KeyCode::Backspace
                        && key.modifiers.contains(KeyModifiers::CONTROL);
                    if is_back && step_idx > 0 {
                        step_idx -= 1;
                        continue;
                    }

                    match current_step {
                        WizardStep::Provider => match provider_list.handle_key(key) {
                            ListAction::Select => {
                                if let Some(selected) = provider_list.selected() {
                                    let new_provider = match selected.as_str() {
                                        "bigmodel" => ProviderType::BigModel,
                                        "xiaomi_mimo" => ProviderType::XiaomiMimo,
                                        "deepseek" => ProviderType::DeepSeek,
                                        _ => ProviderType::BigModel,
                                    };
                                    if new_provider != provider {
                                        provider = new_provider;
                                        provider_mode = None;
                                        base_url = String::new();
                                        model = String::new();
                                    }
                                    rebuild_base_url_list(
                                        &provider,
                                        &provider_mode,
                                        &mut base_url_list,
                                    );
                                    base_url_mode = SubMode::List;
                                    step_idx += 1;
                                }
                            }
                            ListAction::Cancel => return Ok(None),
                            ListAction::None => {}
                        },
                        WizardStep::BaseUrl => match base_url_mode {
                            SubMode::List => match base_url_list.handle_key(key) {
                                ListAction::Select => {
                                    if let Some(selected) = base_url_list.selected() {
                                        let choice = find_base_url_choice(
                                            &provider,
                                            &provider_mode,
                                            selected,
                                        );
                                        match choice.and_then(|c| c.url) {
                                            Some(url) => {
                                                base_url = url.to_string();
                                                provider_mode =
                                                    provider_mode_for_base_url(&base_url);
                                                step_idx += 1;
                                            }
                                            None => {
                                                base_url_mode = SubMode::Input;
                                                base_url_custom_input = TextInput::new(
                                                    t!("init.custom_base_url_label").to_string(),
                                                )
                                                .with_default(CUSTOM_BASE_URL);
                                            }
                                        }
                                    }
                                }
                                ListAction::Cancel => {
                                    if step_idx > 0 {
                                        step_idx -= 1;
                                    } else {
                                        return Ok(None);
                                    }
                                }
                                ListAction::None => {}
                            },
                            SubMode::Input => match base_url_custom_input.handle_key(key) {
                                InputAction::Submit => {
                                    base_url = base_url_custom_input.value().to_string();
                                    provider_mode = provider_mode_for_base_url(&base_url);
                                    step_idx += 1;
                                }
                                InputAction::Cancel => {
                                    base_url_mode = SubMode::List;
                                }
                                InputAction::None => {}
                            },
                        },
                        WizardStep::ApiKey => match api_key_input.handle_key(key) {
                            InputAction::Submit => {
                                api_key = api_key_input.value().to_string();
                                let settings_for_fetch = runtime_config::LlmSettings {
                                    provider: provider.clone(),
                                    provider_mode: provider_mode.clone(),
                                    model: String::new(),
                                    api_key: if api_key.is_empty() {
                                        None
                                    } else {
                                        Some(api_key.clone())
                                    },
                                    api_key_env: None,
                                    base_url: if base_url.is_empty() {
                                        None
                                    } else {
                                        Some(base_url.clone())
                                    },
                                    review_model: None,
                                };
                                let mut fetched_models = Vec::new();
                                if let Ok(client) =
                                    runtime_model::LlmClient::new(settings_for_fetch)
                                {
                                    let fetched = client.list_models_blocking(&base_url, &api_key);
                                    if let Some(first) = fetched.first() {
                                        model = first.clone();
                                    }
                                    fetched_models = fetched;
                                }
                                rebuild_model_list(&fetched_models, &mut model_list);
                                model_mode = SubMode::List;
                                model_input =
                                    TextInput::new(t!("init.step.model").to_string() + ":")
                                        .with_default(&model);
                                step_idx += 1;
                            }
                            InputAction::Cancel => {
                                if step_idx > 0 {
                                    step_idx -= 1;
                                } else {
                                    return Ok(None);
                                }
                            }
                            InputAction::None => {}
                        },
                        WizardStep::Model => match model_mode {
                            SubMode::List => match model_list.handle_key(key) {
                                ListAction::Select => {
                                    if let Some(selected) = model_list.selected() {
                                        if selected.as_str() == t!("init.manual_model") {
                                            model_mode = SubMode::Input;
                                        } else {
                                            model = selected.clone();
                                            step_idx += 1;
                                        }
                                    }
                                }
                                ListAction::Cancel => {
                                    if step_idx > 0 {
                                        step_idx -= 1;
                                    } else {
                                        return Ok(None);
                                    }
                                }
                                ListAction::None => {}
                            },
                            SubMode::Input => match model_input.handle_key(key) {
                                InputAction::Submit => {
                                    model = model_input.value().to_string();
                                    step_idx += 1;
                                }
                                InputAction::Cancel => {
                                    model_mode = SubMode::List;
                                }
                                InputAction::None => {}
                            },
                        },
                        WizardStep::Soul => {
                            if key.code == KeyCode::Char('u') && soul_update_available {
                                soul_list = SelectableList::new(
                                    t!("init.select_soul").to_string(),
                                    load_available_souls_fresh(),
                                );
                                continue;
                            }
                            match soul_list.handle_key(key) {
                                ListAction::Select => {
                                    let soul_role = if let Some(selected) = soul_list.selected() {
                                        crate::formula::activate_soul(&selected.formula.id)?;
                                        SoulRole::new(selected.formula.id.clone())
                                    } else {
                                        anyhow::bail!(t!("init.no_souls").to_string());
                                    };
                                    let settings = Settings {
                                        llm: runtime_config::LlmSettings {
                                            provider,
                                            provider_mode,
                                            model,
                                            api_key: if api_key.is_empty() {
                                                None
                                            } else {
                                                Some(api_key)
                                            },
                                            api_key_env: None,
                                            base_url: if base_url.is_empty() {
                                                None
                                            } else {
                                                Some(base_url)
                                            },
                                            review_model: None,
                                        },
                                        agent: runtime_config::AgentSettings::default(),
                                        soul: runtime_config::SoulSettings { role: soul_role },
                                        ui: runtime_config::UiSettings { locale },
                                    };
                                    settings.save_to_project_config()?;
                                    return Ok(Some(settings));
                                }
                                ListAction::Cancel => {
                                    if step_idx > 0 {
                                        step_idx -= 1;
                                    } else {
                                        return Ok(None);
                                    }
                                }
                                ListAction::None => {}
                            }
                        }
                    }
                }
                Event::Paste(text) => match current_step {
                    WizardStep::BaseUrl if base_url_mode == SubMode::Input => {
                        base_url_custom_input.paste(&text);
                    }
                    WizardStep::ApiKey => {
                        api_key_input.paste(&text);
                    }
                    WizardStep::Model if model_mode == SubMode::Input => {
                        model_input.paste(&text);
                    }
                    _ => {}
                },
                _ => {}
            }
        }
    }
}

fn load_available_souls() -> Vec<SoulChoice> {
    let formulas = match crate::formula::list_installed_souls() {
        Ok(souls) if !souls.is_empty() => souls,
        _ => match crate::formula::sync_all_souls() {
            Ok(_) => crate::formula::list_installed_souls().unwrap_or_default(),
            Err(_) => vec![],
        },
    };
    formulas
        .into_iter()
        .map(|formula| SoulChoice { formula })
        .collect()
}

fn load_available_souls_fresh() -> Vec<SoulChoice> {
    let _ = crate::formula::sync_all_souls();
    crate::formula::list_installed_souls()
        .unwrap_or_default()
        .into_iter()
        .map(|formula| SoulChoice { formula })
        .collect()
}

fn base_url_choices_strings(
    provider: &ProviderType,
    provider_mode: &Option<ProviderMode>,
) -> Vec<String> {
    base_url_choices(provider, provider_mode)
        .iter()
        .map(|c| c.to_string())
        .collect()
}

fn rebuild_base_url_list(
    provider: &ProviderType,
    provider_mode: &Option<ProviderMode>,
    list: &mut SelectableList<String>,
) {
    let choices = base_url_choices_strings(provider, provider_mode);
    *list = SelectableList::new(t!("init.select_base_url").to_string(), choices);
}

fn find_base_url_choice(
    provider: &ProviderType,
    provider_mode: &Option<ProviderMode>,
    display: &str,
) -> Option<BaseUrlChoice> {
    base_url_choices(provider, provider_mode)
        .into_iter()
        .find(|c| c.to_string() == display)
}

fn rebuild_model_list(models: &[String], list: &mut SelectableList<String>) {
    let mut items: Vec<String> = models.to_vec();
    items.push(t!("init.manual_model").to_string());
    *list = SelectableList::new(t!("init.select_model").to_string(), items);
}

fn base_url_choices(
    provider: &ProviderType,
    _provider_mode: &Option<ProviderMode>,
) -> Vec<BaseUrlChoice> {
    match provider {
        ProviderType::BigModel => vec![
            BaseUrlChoice {
                label: "OpenAI API",
                url: Some(BIGMODEL_OPENAI_BASE_URL),
            },
            BaseUrlChoice {
                label: "Anthropic API",
                url: Some(BIGMODEL_ANTHROPIC_BASE_URL),
            },
            BaseUrlChoice {
                label: "Custom",
                url: None,
            },
        ],
        ProviderType::XiaomiMimo => vec![
            BaseUrlChoice {
                label: "OpenAI API",
                url: Some(XIAOMI_MIMO_OPENAI_BASE_URL),
            },
            BaseUrlChoice {
                label: "Anthropic API",
                url: Some(XIAOMI_MIMO_ANTHROPIC_BASE_URL),
            },
            BaseUrlChoice {
                label: "Custom",
                url: None,
            },
        ],
        ProviderType::DeepSeek => vec![
            BaseUrlChoice {
                label: "OpenAI API",
                url: Some(DEEPSEEK_OPENAI_BASE_URL),
            },
            BaseUrlChoice {
                label: "Anthropic API",
                url: Some(DEEPSEEK_ANTHROPIC_BASE_URL),
            },
            BaseUrlChoice {
                label: "Custom",
                url: None,
            },
        ],
        ProviderType::Custom => vec![BaseUrlChoice {
            label: "Custom",
            url: None,
        }],
        _ => vec![BaseUrlChoice {
            label: "Custom",
            url: None,
        }],
    }
}

fn provider_mode_for_base_url(base_url: &str) -> Option<ProviderMode> {
    if base_url.contains("/anthropic") {
        Some(ProviderMode::Native)
    } else {
        Some(ProviderMode::OpenAICompatible)
    }
}

#[cfg(test)]
fn default_base_url(provider: &ProviderType, provider_mode: &Option<ProviderMode>) -> String {
    base_url_choices(provider, provider_mode)
        .into_iter()
        .find_map(|choice| choice.url)
        .unwrap_or(CUSTOM_BASE_URL)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_steps_openai() {
        let steps = build_steps(&ProviderType::Openai);
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0], WizardStep::Provider);
        assert_eq!(steps[1], WizardStep::BaseUrl);
        assert_eq!(steps[4], WizardStep::Soul);
    }

    #[test]
    fn test_build_steps_bigmodel() {
        let steps = build_steps(&ProviderType::BigModel);
        assert_eq!(steps.len(), 5);
        assert_eq!(steps[0], WizardStep::Provider);
        assert_eq!(steps[1], WizardStep::BaseUrl);
        assert_eq!(steps[4], WizardStep::Soul);
    }

    #[test]
    fn test_default_base_url_bigmodel() {
        assert_eq!(
            default_base_url(&ProviderType::BigModel, &None),
            "https://open.bigmodel.cn/api/coding/paas/v4"
        );
    }

    #[test]
    fn test_default_base_url_deepseek() {
        assert_eq!(
            default_base_url(&ProviderType::DeepSeek, &None),
            "https://api.deepseek.com"
        );
    }

    #[test]
    fn test_default_base_url_custom() {
        assert_eq!(
            default_base_url(&ProviderType::Custom, &None),
            "http://localhost:8080/v1"
        );
    }

    #[test]
    fn test_provider_selection() {
        let mut list = SelectableList::new(
            "Test".to_string(),
            PROVIDERS.iter().map(|s| s.to_string()).collect(),
        );
        assert_eq!(list.selected().map(|s| s.as_str()), Some("bigmodel"));
        list.handle_key(KeyEvent::new(KeyCode::Down, event::KeyModifiers::NONE));
        assert_eq!(list.selected().map(|s| s.as_str()), Some("xiaomi_mimo"));
    }

    #[test]
    fn test_bigmodel_base_url_choices_general() {
        let choices = base_url_choices(&ProviderType::BigModel, &None);
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[0].label, "OpenAI API");
        assert_eq!(
            choices[0].url,
            Some("https://open.bigmodel.cn/api/coding/paas/v4")
        );
        assert_eq!(choices[1].label, "Anthropic API");
        assert_eq!(
            choices[1].url,
            Some("https://open.bigmodel.cn/api/anthropic")
        );
        assert_eq!(choices[2].label, "Custom");
        assert_eq!(choices[2].url, None);
    }

    #[test]
    fn test_deepseek_base_url_choices() {
        let choices = base_url_choices(&ProviderType::DeepSeek, &None);
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[0].url, Some("https://api.deepseek.com"));
        assert_eq!(choices[1].url, Some("https://api.deepseek.com/anthropic"));
    }

    #[test]
    fn test_soul_role_creation() {
        let role = SoulRole::new("backend-developer".to_string());
        assert_eq!(role.to_string(), "backend-developer");
    }

    #[test]
    fn test_soul_choice_displays_formula_id_and_description() {
        let choice = SoulChoice {
            formula: FormulaDef {
                id: "backend-developer".to_string(),
                name: "Backend Developer".to_string(),
                version: "0.1.0".to_string(),
                formula_type: "soul".to_string(),
                description: "Backend engineering persona".to_string(),
                license: None,
                model: None,
            },
        };
        assert_eq!(
            choice.to_string(),
            "backend-developer - Backend engineering persona"
        );
    }

    #[test]
    fn test_settings_build() {
        let settings = Settings {
            llm: runtime_config::LlmSettings {
                provider: ProviderType::Openai,
                provider_mode: None,
                model: "gpt-4o".to_string(),
                api_key: Some("test-key".to_string()),
                api_key_env: None,
                base_url: Some("https://api.openai.com/v1".to_string()),
                review_model: None,
            },
            agent: runtime_config::AgentSettings::default(),
            soul: runtime_config::SoulSettings {
                role: SoulRole::new("coder".to_string()),
            },
            ui: runtime_config::UiSettings::default(),
        };
        assert_eq!(settings.llm.model, "gpt-4o");
        assert!(settings.llm.api_key.is_some());
    }
}
