use crate::formula::FormulaDef;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};
use protocol_interface::{ProviderMode, ProviderType};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use runtime_config::Settings;
use rust_i18n::t;

use crate::tui::app::TuiApp;
use crate::tui::components::{
    should_process_key_event, InputAction, ListAction, SelectableList, StepIndicator, TextInput,
};
use crate::tui::theme;

const PROVIDERS: &[&str] = &["bigmodel", "xiaomi_mimo", "deepseek"];
const LOCALES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh-CN", "中文 (简体)"),
    ("ja", "日本語"),
];
const BIGMODEL_OPENAI_BASE_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4";
const BIGMODEL_ANTHROPIC_BASE_URL: &str = "https://open.bigmodel.cn/api/anthropic";
const XIAOMI_MIMO_OPENAI_BASE_URL: &str = "https://api.xiaomimimo.com/v1";
const XIAOMI_MIMO_ANTHROPIC_BASE_URL: &str = "https://api.xiaomimimo.com/anthropic";
const DEEPSEEK_OPENAI_BASE_URL: &str = "https://api.deepseek.com";
const DEEPSEEK_ANTHROPIC_BASE_URL: &str = "https://api.deepseek.com/anthropic";

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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfigStep {
    Provider,
    BaseUrl,
    ApiKey,
    Model,
    Soul,
    Language,
    Confirm,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BaseUrlMode {
    List,
    Input,
}

#[derive(Clone)]
enum SoulChoice {
    KeepCurrent(String),
    Formula(FormulaDef),
}

impl std::fmt::Display for SoulChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::KeepCurrent(role) => write!(f, "{}: {}", t!("common.current_values"), role),
            Self::Formula(formula) => write!(f, "{} - {}", formula.id, formula.description),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConfirmChoice {
    Save,
    Cancel,
}

impl std::fmt::Display for ConfirmChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Save => write!(f, "{}", t!("common.save_exit")),
            Self::Cancel => write!(f, "{}", t!("common.cancel")),
        }
    }
}

pub fn run_config_panel(settings: Settings) -> Result<Option<Settings>> {
    let mut draft = settings;
    let mut app = TuiApp::enter()?;
    let result = run_loop(&mut app, &mut draft);
    app.restore()?;
    let confirmed = result?;
    Ok(confirmed.then_some(draft))
}

fn run_loop(app: &mut TuiApp, settings: &mut Settings) -> Result<bool> {
    let mut step_idx = 0usize;
    let mut base_url_mode = BaseUrlMode::List;

    let mut provider_list = provider_list_for_settings(settings);
    let mut base_url_list = base_url_list_for_settings(settings);
    let mut base_url_input = base_url_input_for_settings(settings);
    let mut api_key_input = TextInput::new(t!("config.api_key_label").to_string())
        .with_default(settings.llm.api_key.as_deref().unwrap_or_default());
    let mut model_input =
        TextInput::new(t!("config.model_label").to_string()).with_default(&settings.llm.model);
    let soul_formulas = crate::formula::list_installed_souls().unwrap_or_default();
    let mut soul_list = soul_list_for_settings(settings, &soul_formulas);
    let mut language_list = language_list_for_settings(settings);
    let mut confirm_list = build_confirm_list();

    loop {
        let steps = build_steps(&settings.llm.provider);
        if step_idx >= steps.len() {
            step_idx = steps.len().saturating_sub(1);
        }
        let current_step = steps[step_idx];

        app.draw(|frame| {
            let area = frame.area();
            let block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    format!(" {} ", t!("config.title")),
                    theme::title(),
                ))
                .style(theme::base())
                .border_style(theme::border(true))
                .title_alignment(Alignment::Center);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            render_step_header(frame, inner, &steps, step_idx, current_step, settings);

            let content_area = Rect::new(
                inner.x + 2,
                inner.y + 6,
                inner.width.saturating_sub(4),
                inner.height.saturating_sub(8),
            );
            let help_area = Rect::new(
                inner.x + 2,
                inner.y + inner.height.saturating_sub(1),
                inner.width.saturating_sub(4),
                1,
            );

            match current_step {
                ConfigStep::Provider => provider_list.render(frame, content_area),
                ConfigStep::BaseUrl => match base_url_mode {
                    BaseUrlMode::List => base_url_list.render(frame, content_area),
                    BaseUrlMode::Input => render_input(frame, content_area, &base_url_input),
                },
                ConfigStep::ApiKey => render_input(frame, content_area, &api_key_input),
                ConfigStep::Model => render_input(frame, content_area, &model_input),
                ConfigStep::Soul => soul_list.render(frame, content_area),
                ConfigStep::Language => language_list.render(frame, content_area),
                ConfigStep::Confirm => {
                    render_confirmation(frame, content_area, settings, &mut confirm_list)
                }
            }

            let help_text = if step_idx == 0 {
                t!("init.help.first_step").to_string()
            } else {
                t!("init.help.next_step").to_string()
            };
            frame.render_widget(
                Paragraph::new(Span::styled(help_text, theme::secondary())),
                help_area,
            );
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            match event::read()? {
                Event::Paste(text) => match current_step {
                    ConfigStep::BaseUrl if base_url_mode == BaseUrlMode::Input => {
                        base_url_input.paste(&text);
                    }
                    ConfigStep::ApiKey => api_key_input.paste(&text),
                    ConfigStep::Model => model_input.paste(&text),
                    _ => {}
                },
                Event::Key(key) => {
                    if !should_process_key_event(&key) {
                        continue;
                    }

                    if key.code == KeyCode::Backspace
                        && key.modifiers.contains(KeyModifiers::CONTROL)
                    {
                        if current_step == ConfigStep::BaseUrl
                            && base_url_mode == BaseUrlMode::Input
                        {
                            base_url_mode = BaseUrlMode::List;
                        } else if !go_back(&mut step_idx) {
                            return Ok(false);
                        }
                        continue;
                    }

                    match current_step {
                        ConfigStep::Provider => match provider_list.handle_key(key) {
                            ListAction::Select => {
                                if let Some(selected) = provider_list.selected() {
                                    let provider = parse_provider(selected);
                                    if provider != settings.llm.provider {
                                        settings.llm.provider = provider;
                                        settings.llm.provider_mode = None;
                                        settings.llm.base_url = None;
                                    }
                                    base_url_list = base_url_list_for_settings(settings);
                                    base_url_input = base_url_input_for_settings(settings);
                                    base_url_mode = BaseUrlMode::List;
                                    step_idx += 1;
                                }
                            }
                            ListAction::Cancel => return Ok(false),
                            ListAction::None => {}
                        },
                        ConfigStep::BaseUrl => match base_url_mode {
                            BaseUrlMode::List => match base_url_list.handle_key(key) {
                                ListAction::Select => {
                                    if let Some(selected) = base_url_list.selected() {
                                        if let Some(url) = selected.url {
                                            settings.llm.base_url = Some(url.to_string());
                                            settings.llm.provider_mode =
                                                provider_mode_for_base_url(url);
                                            base_url_input = base_url_input_for_settings(settings);
                                            step_idx += 1;
                                        } else {
                                            base_url_input = base_url_input_for_settings(settings);
                                            base_url_mode = BaseUrlMode::Input;
                                        }
                                    }
                                }
                                ListAction::Cancel => {
                                    if !go_back(&mut step_idx) {
                                        return Ok(false);
                                    }
                                }
                                ListAction::None => {}
                            },
                            BaseUrlMode::Input => match base_url_input.handle_key(key) {
                                InputAction::Submit => {
                                    let base_url = base_url_input.value().to_string();
                                    settings.llm.provider_mode =
                                        provider_mode_for_base_url(&base_url);
                                    settings.llm.base_url = Some(base_url);
                                    base_url_list = base_url_list_for_settings(settings);
                                    base_url_mode = BaseUrlMode::List;
                                    step_idx += 1;
                                }
                                InputAction::Cancel => {
                                    base_url_mode = BaseUrlMode::List;
                                }
                                InputAction::None => {}
                            },
                        },
                        ConfigStep::ApiKey => match api_key_input.handle_key(key) {
                            InputAction::Submit => {
                                let val = api_key_input.value().to_string();
                                settings.llm.api_key =
                                    if val.is_empty() { None } else { Some(val) };
                                step_idx += 1;
                            }
                            InputAction::Cancel => {
                                if !go_back(&mut step_idx) {
                                    return Ok(false);
                                }
                            }
                            InputAction::None => {}
                        },
                        ConfigStep::Model => match model_input.handle_key(key) {
                            InputAction::Submit => {
                                settings.llm.model = model_input.value().to_string();
                                step_idx += 1;
                            }
                            InputAction::Cancel => {
                                if !go_back(&mut step_idx) {
                                    return Ok(false);
                                }
                            }
                            InputAction::None => {}
                        },
                        ConfigStep::Soul => match soul_list.handle_key(key) {
                            ListAction::Select => {
                                if let Some(selected) = soul_list.selected() {
                                    match selected {
                                        SoulChoice::KeepCurrent(_) => {}
                                        SoulChoice::Formula(formula) => {
                                            let _ = crate::formula::activate_soul(&formula.id);
                                            settings.soul.role = protocol_interface::SoulRole::new(
                                                formula.id.clone(),
                                            );
                                        }
                                    }
                                    soul_list = soul_list_for_settings(settings, &soul_formulas);
                                    step_idx += 1;
                                }
                            }
                            ListAction::Cancel => {
                                if !go_back(&mut step_idx) {
                                    return Ok(false);
                                }
                            }
                            ListAction::None => {}
                        },
                        ConfigStep::Language => match language_list.handle_key(key) {
                            ListAction::Select => {
                                if let Some(selected) = language_list.selected() {
                                    let new_locale = locale_from_display(selected);
                                    settings.ui.locale = new_locale.clone();
                                    crate::set_locale(&new_locale);
                                    provider_list = provider_list_for_settings(settings);
                                    base_url_list = base_url_list_for_settings(settings);
                                    base_url_input = base_url_input_for_settings(settings);
                                    api_key_input =
                                        TextInput::new(t!("config.api_key_label").to_string())
                                            .with_default(
                                                settings.llm.api_key.as_deref().unwrap_or_default(),
                                            );
                                    model_input =
                                        TextInput::new(t!("config.model_label").to_string())
                                            .with_default(&settings.llm.model);
                                    soul_list = soul_list_for_settings(settings, &soul_formulas);
                                    language_list = language_list_for_settings(settings);
                                    confirm_list = build_confirm_list();
                                    step_idx += 1;
                                }
                            }
                            ListAction::Cancel => {
                                if !go_back(&mut step_idx) {
                                    return Ok(false);
                                }
                            }
                            ListAction::None => {}
                        },
                        ConfigStep::Confirm => match confirm_list.handle_key(key) {
                            ListAction::Select => {
                                if let Some(selected) = confirm_list.selected() {
                                    match selected {
                                        ConfirmChoice::Save => {
                                            settings.save_to_user_config()?;
                                            return Ok(true);
                                        }
                                        ConfirmChoice::Cancel => return Ok(false),
                                    }
                                }
                            }
                            ListAction::Cancel => {
                                if !go_back(&mut step_idx) {
                                    return Ok(false);
                                }
                            }
                            ListAction::None => {}
                        },
                    }
                }
                _ => {}
            }
        }
    }
}

fn render_step_header(
    frame: &mut ratatui::Frame,
    area: Rect,
    steps: &[ConfigStep],
    step_idx: usize,
    current_step: ConfigStep,
    settings: &Settings,
) {
    let indicator_area = Rect::new(area.x + 2, area.y + 1, area.width.saturating_sub(4), 1);
    let mut indicator =
        StepIndicator::new(steps.iter().map(|step| step_display_name(*step)).collect());
    indicator.set_current(step_idx);
    indicator.render(frame, indicator_area);

    let label_area = Rect::new(area.x + 2, area.y + 3, area.width.saturating_sub(4), 1);
    let label = t!(
        "init.step_label",
        current = step_idx + 1,
        total = steps.len(),
        name = step_display_name(current_step)
    );
    frame.render_widget(
        Paragraph::new(Span::styled(label.to_string(), theme::emphasis())),
        label_area,
    );

    if let Some(value) = current_value_for_step(current_step, settings) {
        let current_area = Rect::new(area.x + 2, area.y + 4, area.width.saturating_sub(4), 1);
        frame.render_widget(
            Paragraph::new(Span::styled(value, theme::secondary())),
            current_area,
        );
    }
}

fn render_input(frame: &mut ratatui::Frame, area: Rect, input: &TextInput) {
    let mut display = input.clone();
    display.set_active(true);
    let input_area = Rect::new(area.x, area.y, area.width, TextInput::HEIGHT);
    display.render(frame, input_area);
}

fn render_confirmation(
    frame: &mut ratatui::Frame,
    area: Rect,
    settings: &Settings,
    confirm_list: &mut SelectableList<ConfirmChoice>,
) {
    let summary_height = (area.height / 2).max(6);
    let summary_area = Rect::new(area.x, area.y, area.width, summary_height.min(area.height));
    let list_y = summary_area.y + summary_area.height.saturating_add(1);
    let list_area = Rect::new(
        area.x,
        list_y,
        area.width,
        area.y.saturating_add(area.height).saturating_sub(list_y),
    );

    let summary = Paragraph::new(format_settings_summary(settings))
        .style(theme::text())
        .wrap(Wrap { trim: false });
    frame.render_widget(summary, summary_area);
    confirm_list.render(frame, list_area);
}

fn build_steps(_provider: &ProviderType) -> Vec<ConfigStep> {
    let mut steps = vec![ConfigStep::Provider];
    steps.extend_from_slice(&[
        ConfigStep::BaseUrl,
        ConfigStep::ApiKey,
        ConfigStep::Model,
        ConfigStep::Soul,
        ConfigStep::Language,
        ConfigStep::Confirm,
    ]);
    steps
}

fn step_display_name(step: ConfigStep) -> String {
    match step {
        ConfigStep::Provider => t!("init.step.provider").to_string(),
        ConfigStep::BaseUrl => t!("init.step.base_url").to_string(),
        ConfigStep::ApiKey => t!("init.step.api_key").to_string(),
        ConfigStep::Model => t!("init.step.model").to_string(),
        ConfigStep::Soul => t!("init.step.soul").to_string(),
        ConfigStep::Language => t!("init.step.language").to_string(),
        ConfigStep::Confirm => t!("common.save").to_string(),
    }
}

fn current_value_for_step(step: ConfigStep, settings: &Settings) -> Option<String> {
    let value = match step {
        ConfigStep::Provider => format!("{:?}", settings.llm.provider),
        ConfigStep::BaseUrl => settings.effective_base_url(),
        ConfigStep::ApiKey => {
            if let Some(key) = settings.llm.api_key.as_deref() {
                key.to_string()
            } else {
                t!("common.not_set").to_string()
            }
        }
        ConfigStep::Model => settings.llm.model.clone(),
        ConfigStep::Soul => current_soul(settings),
        ConfigStep::Language => locale_display(&settings.ui.locale),
        ConfigStep::Confirm => return None,
    };
    Some(format!("{}: {}", t!("common.current_values"), value))
}

fn format_settings_summary(settings: &Settings) -> String {
    let api_key_status = if let Some(key) = settings.llm.api_key.as_deref() {
        key.to_string()
    } else {
        t!("common.not_set").to_string()
    };
    let provider = format!("{:?}", settings.llm.provider);

    format!(
        "{}\n\n\
         {} {}\n\
         {} {}\n\
         {} {}\n\
         {} {}\n\
         {} {}\n\
         {} {}\n\
         {} {}",
        t!("common.current_values"),
        t!("config.show_provider"),
        provider,
        t!("init.step.mode"),
        mode_display(settings),
        t!("config.show_base_url"),
        settings.effective_base_url(),
        t!("config.show_api_key"),
        api_key_status,
        t!("config.show_model"),
        settings.llm.model,
        t!("config.show_soul"),
        current_soul(settings),
        t!("config.show_language"),
        locale_display(&settings.ui.locale),
    )
}

fn provider_list_for_settings(settings: &Settings) -> SelectableList<String> {
    let current = provider_key(&settings.llm.provider);
    let mut list = SelectableList::new(
        t!("config.select_provider").to_string(),
        PROVIDERS
            .iter()
            .map(|provider| provider.to_string())
            .collect(),
    );
    if let Some(idx) = PROVIDERS.iter().position(|provider| *provider == current) {
        select_index(&mut list, idx);
    }
    list
}

fn base_url_list_for_settings(settings: &Settings) -> SelectableList<BaseUrlChoice> {
    let choices = base_url_choices(&settings.llm.provider, &settings.llm.provider_mode);
    let selected_idx = settings
        .llm
        .base_url
        .as_deref()
        .and_then(|url| {
            choices.iter().position(|choice| {
                choice
                    .url
                    .map(|known_url| known_url.trim() == url.trim())
                    .unwrap_or(false)
            })
        })
        .unwrap_or_else(|| {
            if settings.llm.base_url.is_some() {
                choices
                    .iter()
                    .position(|choice| choice.url.is_none())
                    .unwrap_or(0)
            } else {
                0
            }
        });

    let mut list = SelectableList::new(t!("config.select_base_url").to_string(), choices);
    select_index(&mut list, selected_idx);
    list
}

fn base_url_input_for_settings(settings: &Settings) -> TextInput {
    let input = TextInput::new(t!("config.base_url_label").to_string());
    if let Some(base_url) = editable_custom_base_url(settings) {
        input.with_default(base_url)
    } else {
        input.with_placeholder(&settings.effective_base_url())
    }
}

fn editable_custom_base_url(settings: &Settings) -> Option<&str> {
    let base_url = settings.llm.base_url.as_deref()?.trim();
    if base_url.is_empty()
        || is_builtin_base_url(
            base_url,
            &settings.llm.provider,
            &settings.llm.provider_mode,
        )
    {
        None
    } else {
        Some(base_url)
    }
}

fn is_builtin_base_url(
    url: &str,
    provider: &ProviderType,
    provider_mode: &Option<ProviderMode>,
) -> bool {
    base_url_choices(provider, provider_mode)
        .iter()
        .filter_map(|choice| choice.url)
        .any(|known_url| known_url.trim() == url.trim())
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

fn soul_list_for_settings(
    settings: &Settings,
    formulas: &[FormulaDef],
) -> SelectableList<SoulChoice> {
    let current = current_soul(settings);
    let mut choices = vec![SoulChoice::KeepCurrent(current)];
    choices.extend(formulas.iter().cloned().map(SoulChoice::Formula));

    SelectableList::new(t!("config.select_soul").to_string(), choices)
}

fn language_list_for_settings(settings: &Settings) -> SelectableList<String> {
    let locale_items: Vec<String> = LOCALES
        .iter()
        .map(|(code, name)| format!("{} - {}", name, code))
        .collect();
    let mut list = SelectableList::new(t!("config.select_language").to_string(), locale_items);
    if let Some(idx) = LOCALES
        .iter()
        .position(|(code, _)| *code == settings.ui.locale)
    {
        select_index(&mut list, idx);
    }
    list
}

fn build_confirm_list() -> SelectableList<ConfirmChoice> {
    SelectableList::new(
        t!("config.confirm_title").to_string(),
        vec![ConfirmChoice::Save, ConfirmChoice::Cancel],
    )
}

fn parse_provider(provider: &str) -> ProviderType {
    match provider {
        "bigmodel" => ProviderType::BigModel,
        "xiaomi_mimo" => ProviderType::XiaomiMimo,
        "deepseek" => ProviderType::DeepSeek,
        _ => ProviderType::BigModel,
    }
}

fn provider_key(provider: &ProviderType) -> &'static str {
    match provider {
        ProviderType::BigModel => "bigmodel",
        ProviderType::XiaomiMimo => "xiaomi_mimo",
        ProviderType::DeepSeek => "deepseek",
        _ => "bigmodel",
    }
}

fn provider_mode_for_base_url(base_url: &str) -> Option<ProviderMode> {
    if base_url.contains("/anthropic") {
        Some(ProviderMode::Native)
    } else {
        Some(ProviderMode::OpenAICompatible)
    }
}

fn locale_from_display(display: &str) -> String {
    LOCALES
        .iter()
        .find_map(|(code, name)| {
            if display.starts_with(name) {
                Some(code.to_string())
            } else {
                None
            }
        })
        .unwrap_or_else(|| "en".to_string())
}

fn locale_display(locale: &str) -> String {
    LOCALES
        .iter()
        .find(|(code, _)| *code == locale)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| locale.to_string())
}

fn current_soul(settings: &Settings) -> String {
    crate::formula::current_project_soul().unwrap_or_else(|| settings.soul.role.to_string())
}

fn mode_display(settings: &Settings) -> String {
    match settings.llm.provider_mode {
        Some(ProviderMode::Native) => "Anthropic API".to_string(),
        Some(ProviderMode::OpenAICompatible) => "OpenAI API".to_string(),
        _ if settings.effective_base_url().contains("/anthropic") => "Anthropic API".to_string(),
        _ => "OpenAI API".to_string(),
    }
}

fn go_back(step_idx: &mut usize) -> bool {
    if *step_idx > 0 {
        *step_idx -= 1;
        true
    } else {
        false
    }
}

fn select_index<T: Clone + std::fmt::Display>(list: &mut SelectableList<T>, idx: usize) {
    for _ in 0..idx {
        let _ = list.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn test_build_steps_openai() {
        let steps = build_steps(&ProviderType::Openai);
        assert_eq!(
            steps,
            vec![
                ConfigStep::Provider,
                ConfigStep::BaseUrl,
                ConfigStep::ApiKey,
                ConfigStep::Model,
                ConfigStep::Soul,
                ConfigStep::Language,
                ConfigStep::Confirm,
            ]
        );
    }

    #[test]
    fn test_build_steps_bigmodel_includes_mode() {
        let steps = build_steps(&ProviderType::BigModel);
        assert_eq!(steps[0], ConfigStep::Provider);
        assert_eq!(steps[1], ConfigStep::BaseUrl);
        assert_eq!(steps.last(), Some(&ConfigStep::Confirm));
    }

    #[test]
    fn test_provider_options() {
        assert_eq!(PROVIDERS.len(), 3);
        assert!(PROVIDERS.contains(&"bigmodel"));
        assert!(PROVIDERS.contains(&"xiaomi_mimo"));
        assert!(PROVIDERS.contains(&"deepseek"));
    }

    #[test]
    fn test_provider_parse() {
        let p = parse_provider("bigmodel");
        assert!(matches!(p, ProviderType::BigModel));
    }

    #[test]
    fn test_bigmodel_base_url_options() {
        let choices = base_url_choices(&ProviderType::BigModel, &None);
        assert_eq!(choices.len(), 3);
        assert_eq!(
            choices[0].url,
            Some("https://open.bigmodel.cn/api/coding/paas/v4")
        );
        assert_eq!(
            choices[1].url,
            Some("https://open.bigmodel.cn/api/anthropic")
        );
        assert_eq!(choices[2].url, None);
    }

    #[test]
    fn test_deepseek_base_url_options() {
        let choices = base_url_choices(&ProviderType::DeepSeek, &None);
        assert_eq!(choices.len(), 3);
        assert_eq!(choices[0].url, Some("https://api.deepseek.com"));
        assert_eq!(choices[1].url, Some("https://api.deepseek.com/anthropic"));
    }

    #[test]
    fn test_base_url_input_starts_empty_for_builtin_default() {
        let mut settings = test_settings();
        settings.llm.provider = ProviderType::BigModel;
        settings.llm.base_url = Some(BIGMODEL_OPENAI_BASE_URL.to_string());

        let input = base_url_input_for_settings(&settings);

        assert_eq!(input.value(), "");
    }

    #[test]
    fn test_base_url_input_preserves_existing_custom_url() {
        let mut settings = test_settings();
        settings.llm.provider = ProviderType::BigModel;
        settings.llm.base_url = Some("http://localhost:11434/v1".to_string());

        let input = base_url_input_for_settings(&settings);

        assert_eq!(input.value(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_settings_default_values() {
        let s = test_settings();
        assert_eq!(s.llm.model, "gpt-4o-mini");
        assert!(s.llm.api_key.is_none());
    }

    #[test]
    fn test_soul_choices_keep_current() {
        let settings = test_settings();
        let list = soul_list_for_settings(&settings, &[]);

        assert!(matches!(list.selected(), Some(SoulChoice::KeepCurrent(_))));
    }

    #[test]
    fn test_language_parse() {
        assert_eq!(locale_from_display("中文 (简体) - zh-CN"), "zh-CN");
        assert_eq!(locale_from_display("English - en"), "en");
    }
}
