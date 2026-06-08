use crate::formula::FormulaDef;
use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use protocol_interface::{ProviderMode, ProviderType};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use runtime_config::Settings;
use rust_i18n::t;

use crate::tui::app::TuiApp;
use crate::tui::components::{
    should_process_key_event, InputAction, ListAction, SelectableList, TextInput,
};
use crate::tui::theme;

const MENU_ITEMS: &[&str] = &[
    "Provider",
    "Mode",
    "Base URL",
    "API Key",
    "Model",
    "Soul",
    "Language",
    "Save & Exit",
    "Cancel",
];

const PROVIDERS: &[&str] = &["openai", "anthropic", "google", "bigmodel"];
const LOCALES: &[(&str, &str)] = &[
    ("en", "English"),
    ("zh-CN", "中文 (简体)"),
    ("ja", "日本語"),
];
const OPENAI_BASE_URL: &str = "https://api.openai.com/v1";
const ANTHROPIC_BASE_URL: &str = "https://api.anthropic.com";
const GOOGLE_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1";
const BIGMODEL_GLM_BASE_URL: &str = "https://open.bigmodel.cn/api/paas/v4/";
const BIGMODEL_CODING_BASE_URL: &str = "https://open.bigmodel.cn/api/coding/paas/v4";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct BaseUrlChoice {
    label: &'static str,
    url: Option<&'static str>,
}

impl std::fmt::Display for BaseUrlChoice {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.label)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Panel {
    Menu,
    EditProvider,
    EditMode,
    SelectBaseUrl,
    EditBaseUrl,
    EditApiKey,
    EditModel,
    EditSoul,
    EditLanguage,
}

pub fn run_config_panel(mut settings: Settings) -> Result<Option<Settings>> {
    let mut app = TuiApp::enter()?;
    let result = run_loop(&mut app, &mut settings);
    app.restore()?;
    result.map(|_| Some(settings))
}

fn menu_display_items() -> Vec<String> {
    MENU_ITEMS
        .iter()
        .map(|item| match *item {
            "Provider" => t!("config.menu.provider").to_string(),
            "Mode" => t!("init.step.mode").to_string(),
            "Base URL" => t!("config.menu.base_url").to_string(),
            "API Key" => t!("config.menu.api_key").to_string(),
            "Model" => t!("config.menu.model").to_string(),
            "Soul" => t!("config.menu.soul").to_string(),
            "Language" => t!("config.menu.language").to_string(),
            "Save & Exit" => t!("config.menu.save_exit").to_string(),
            "Cancel" => t!("config.menu.cancel").to_string(),
            _ => item.to_string(),
        })
        .collect()
}

fn run_loop(app: &mut TuiApp, settings: &mut Settings) -> Result<()> {
    let mut panel = Panel::Menu;
    let mut menu = SelectableList::new(t!("config.title").to_string(), menu_display_items());
    let mut current_input: Option<TextInput> = None;
    let mut provider_list: Option<SelectableList<String>> = None;
    let mut mode_list: Option<SelectableList<String>> = None;
    let mut base_url_list: Option<SelectableList<BaseUrlChoice>> = None;
    let mut soul_list: Option<SelectableList<String>> = None;
    let mut soul_formulas: Option<Vec<FormulaDef>> = None;
    let mut language_list: Option<SelectableList<String>> = None;

    loop {
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

            match panel {
                Panel::Menu => {
                    render_menu(frame, inner, settings, &mut menu);
                }
                Panel::EditProvider => {
                    if let Some(ref mut list) = provider_list {
                        list.render(frame, inner);
                    }
                }
                Panel::EditMode => {
                    if let Some(ref mut list) = mode_list {
                        list.render(frame, inner);
                    }
                }
                Panel::SelectBaseUrl => {
                    if let Some(ref mut list) = base_url_list {
                        list.render(frame, inner);
                    }
                }
                Panel::EditBaseUrl | Panel::EditApiKey | Panel::EditModel => {
                    if let Some(ref input) = current_input {
                        let mut display = input.clone();
                        display.set_active(true);
                        let y = inner.height / 2;
                        let input_area = Rect::new(
                            inner.x + 2,
                            inner.y + y,
                            inner.width.saturating_sub(4),
                            TextInput::HEIGHT,
                        );
                        display.render(frame, input_area);
                    }
                }
                Panel::EditSoul => {
                    if let Some(ref mut list) = soul_list {
                        list.render(frame, inner);
                    }
                }
                Panel::EditLanguage => {
                    if let Some(ref mut list) = language_list {
                        list.render(frame, inner);
                    }
                }
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let input_event = event::read()?;
            if let Event::Paste(text) = input_event {
                paste_into_current_input(panel, &mut current_input, &text);
                continue;
            }

            if let Event::Key(key) = input_event {
                if !should_process_key_event(&key) {
                    continue;
                }
                match panel {
                    Panel::Menu => match menu.handle_key(key) {
                        ListAction::Select => {
                            let sel = menu.selected().map(|s| s.as_str()).unwrap_or("");
                            // Map displayed text back to the MENU_ITEMS key
                            let item_key = MENU_ITEMS
                                .iter()
                                .find(|&&item| {
                                    let display = match item {
                                        "Provider" => t!("config.menu.provider").to_string(),
                                        "Mode" => t!("init.step.mode").to_string(),
                                        "Base URL" => t!("config.menu.base_url").to_string(),
                                        "API Key" => t!("config.menu.api_key").to_string(),
                                        "Model" => t!("config.menu.model").to_string(),
                                        "Soul" => t!("config.menu.soul").to_string(),
                                        "Language" => t!("config.menu.language").to_string(),
                                        "Save & Exit" => t!("config.menu.save_exit").to_string(),
                                        "Cancel" => t!("config.menu.cancel").to_string(),
                                        _ => item.to_string(),
                                    };
                                    display == sel
                                })
                                .copied()
                                .unwrap_or("");
                            match item_key {
                                "Provider" => {
                                    let current_str =
                                        format!("{:?}", settings.llm.provider).to_lowercase();
                                    let provider_items: Vec<String> =
                                        PROVIDERS.iter().map(|s| s.to_string()).collect();
                                    let mut list = SelectableList::new(
                                        t!("config.select_provider").to_string(),
                                        provider_items,
                                    );
                                    if let Some(idx) =
                                        PROVIDERS.iter().position(|p| *p == current_str)
                                    {
                                        for _ in 0..idx {
                                            list.handle_key(KeyEvent::new(
                                                KeyCode::Down,
                                                event::KeyModifiers::NONE,
                                            ));
                                        }
                                    }
                                    provider_list = Some(list);
                                    panel = Panel::EditProvider;
                                }
                                "Mode" => {
                                    if matches!(settings.llm.provider, ProviderType::BigModel) {
                                        let general_label = t!("init.mode_general").to_string();
                                        let coding_label = t!("init.mode_coding").to_string();
                                        let mut list = SelectableList::new(
                                            t!("init.select_mode").to_string(),
                                            vec![general_label.clone(), coding_label.clone()],
                                        );
                                        if matches!(
                                            settings.llm.provider_mode,
                                            Some(ProviderMode::Coding)
                                        ) {
                                            list.handle_key(KeyEvent::new(
                                                KeyCode::Down,
                                                event::KeyModifiers::NONE,
                                            ));
                                        }
                                        mode_list = Some(list);
                                        panel = Panel::EditMode;
                                    }
                                    // For non-BigModel providers, Mode menu item is a no-op
                                }
                                "Base URL" => {
                                    base_url_list = Some(base_url_list_for_provider(
                                        &settings.llm.provider,
                                        &settings.llm.provider_mode,
                                    ));
                                    panel = Panel::SelectBaseUrl;
                                }
                                "API Key" => {
                                    current_input = Some(
                                        TextInput::new(t!("config.api_key_label").to_string())
                                            .with_default(
                                                settings.llm.api_key.as_deref().unwrap_or(""),
                                            ),
                                    );
                                    panel = Panel::EditApiKey;
                                }
                                "Model" => {
                                    current_input = Some(
                                        TextInput::new(t!("config.model_label").to_string())
                                            .with_default(&settings.llm.model),
                                    );
                                    panel = Panel::EditModel;
                                }
                                "Soul" => {
                                    let formulas: Vec<FormulaDef> =
                                        crate::formula::list_installed_souls().unwrap_or_default();
                                    if formulas.is_empty() {
                                        eprintln!("No local souls found. Run 'alius soul update' to sync souls.");
                                        panel = Panel::Menu;
                                    } else {
                                        let current = crate::formula::current_project_soul()
                                            .unwrap_or_else(|| settings.soul.role.to_string());
                                        let items: Vec<String> = formulas
                                            .iter()
                                            .map(|f| format!("{} - {}", f.id, f.description))
                                            .collect();
                                        let mut list = SelectableList::new(
                                            t!("config.select_soul").to_string(),
                                            items,
                                        );
                                        if let Some(idx) =
                                            formulas.iter().position(|f| f.id == current)
                                        {
                                            for _ in 0..idx {
                                                list.handle_key(KeyEvent::new(
                                                    KeyCode::Down,
                                                    event::KeyModifiers::NONE,
                                                ));
                                            }
                                        }
                                        soul_formulas = Some(formulas);
                                        soul_list = Some(list);
                                        panel = Panel::EditSoul;
                                    }
                                }
                                "Language" => {
                                    let locale_items: Vec<String> = LOCALES
                                        .iter()
                                        .map(|(code, name)| format!("{} - {}", name, code))
                                        .collect();
                                    let mut list = SelectableList::new(
                                        t!("config.select_language").to_string(),
                                        locale_items,
                                    );
                                    // Pre-select current locale
                                    if let Some(idx) = LOCALES
                                        .iter()
                                        .position(|(code, _)| *code == settings.ui.locale)
                                    {
                                        for _ in 0..idx {
                                            list.handle_key(KeyEvent::new(
                                                KeyCode::Down,
                                                event::KeyModifiers::NONE,
                                            ));
                                        }
                                    }
                                    language_list = Some(list);
                                    panel = Panel::EditLanguage;
                                }
                                "Save & Exit" => {
                                    settings.save_to_user_config()?;
                                    return Ok(());
                                }
                                _ => {
                                    return Err(anyhow::anyhow!("{}", t!("config.cancelled")));
                                }
                            }
                        }
                        ListAction::Cancel => {
                            return Err(anyhow::anyhow!("{}", t!("config.cancelled")));
                        }
                        ListAction::None => {}
                    },
                    Panel::EditProvider => {
                        if let Some(ref mut list) = provider_list {
                            match list.handle_key(key) {
                                ListAction::Select => {
                                    if let Some(selected) = list.selected() {
                                        settings.llm.provider = match selected.as_str() {
                                            "openai" => ProviderType::Openai,
                                            "anthropic" => ProviderType::Anthropic,
                                            "google" => ProviderType::Google,
                                            "bigmodel" => ProviderType::BigModel,
                                            _ => ProviderType::Custom,
                                        };
                                        settings.llm.base_url = None;
                                        settings.llm.provider_mode = None;
                                    }
                                    base_url_list = Some(base_url_list_for_provider(
                                        &settings.llm.provider,
                                        &settings.llm.provider_mode,
                                    ));
                                    panel = Panel::SelectBaseUrl;
                                    provider_list = None;
                                }
                                ListAction::Cancel => {
                                    panel = Panel::Menu;
                                    provider_list = None;
                                }
                                ListAction::None => {}
                            }
                        }
                    }
                    Panel::EditMode => {
                        if let Some(ref mut list) = mode_list {
                            match list.handle_key(key) {
                                ListAction::Select => {
                                    let general_label = t!("init.mode_general").to_string();
                                    if let Some(selected) = list.selected() {
                                        settings.llm.provider_mode = if *selected == general_label {
                                            Some(ProviderMode::General)
                                        } else {
                                            Some(ProviderMode::Coding)
                                        };
                                        settings.llm.base_url = None;
                                    }
                                    panel = Panel::Menu;
                                    mode_list = None;
                                }
                                ListAction::Cancel => {
                                    panel = Panel::Menu;
                                    mode_list = None;
                                }
                                ListAction::None => {}
                            }
                        }
                    }
                    Panel::SelectBaseUrl => {
                        if let Some(ref mut list) = base_url_list {
                            match list.handle_key(key) {
                                ListAction::Select => {
                                    if let Some(selected) = list.selected() {
                                        if let Some(url) = selected.url {
                                            settings.llm.base_url = Some(url.to_string());
                                            panel = Panel::Menu;
                                            base_url_list = None;
                                        } else {
                                            current_input =
                                                Some(base_url_input_for_settings(settings));
                                            panel = Panel::EditBaseUrl;
                                        }
                                    }
                                }
                                ListAction::Cancel => {
                                    panel = Panel::Menu;
                                    base_url_list = None;
                                }
                                ListAction::None => {}
                            }
                        }
                    }
                    Panel::EditBaseUrl => {
                        if let Some(ref mut input) = current_input {
                            match input.handle_key(key) {
                                InputAction::Submit => {
                                    settings.llm.base_url = Some(input.value().to_string());
                                    panel = Panel::Menu;
                                    current_input = None;
                                    base_url_list = None;
                                }
                                InputAction::Cancel => {
                                    panel = Panel::Menu;
                                    current_input = None;
                                    base_url_list = None;
                                }
                                InputAction::None => {}
                            }
                        }
                    }
                    Panel::EditApiKey => {
                        if let Some(ref mut input) = current_input {
                            match input.handle_key(key) {
                                InputAction::Submit => {
                                    let val = input.value().to_string();
                                    settings.llm.api_key =
                                        if val.is_empty() { None } else { Some(val) };
                                    panel = Panel::Menu;
                                    current_input = None;
                                }
                                InputAction::Cancel => {
                                    panel = Panel::Menu;
                                    current_input = None;
                                }
                                InputAction::None => {}
                            }
                        }
                    }
                    Panel::EditModel => {
                        if let Some(ref mut input) = current_input {
                            match input.handle_key(key) {
                                InputAction::Submit => {
                                    settings.llm.model = input.value().to_string();
                                    panel = Panel::Menu;
                                    current_input = None;
                                }
                                InputAction::Cancel => {
                                    panel = Panel::Menu;
                                    current_input = None;
                                }
                                InputAction::None => {}
                            }
                        }
                    }
                    Panel::EditSoul => {
                        if let Some(ref mut list) = soul_list {
                            match list.handle_key(key) {
                                ListAction::Select => {
                                    if let (Some(selected), Some(formulas)) =
                                        (list.selected(), &soul_formulas)
                                    {
                                        let id = selected.split(" - ").next().unwrap_or("");
                                        if let Some(formula) = formulas.iter().find(|f| f.id == id)
                                        {
                                            let _ = crate::formula::activate_soul(&formula.id);
                                            settings.soul.role = protocol_interface::SoulRole::new(
                                                formula.id.clone(),
                                            );
                                        }
                                    }
                                    panel = Panel::Menu;
                                    soul_list = None;
                                    soul_formulas = None;
                                }
                                ListAction::Cancel => {
                                    panel = Panel::Menu;
                                    soul_list = None;
                                    soul_formulas = None;
                                }
                                ListAction::None => {}
                            }
                        }
                    }
                    Panel::EditLanguage => {
                        if let Some(ref mut list) = language_list {
                            match list.handle_key(key) {
                                ListAction::Select => {
                                    if let Some(selected) = list.selected() {
                                        let new_locale = LOCALES
                                            .iter()
                                            .find_map(|(code, name)| {
                                                if selected.starts_with(name) {
                                                    Some(code.to_string())
                                                } else {
                                                    None
                                                }
                                            })
                                            .unwrap_or_else(|| "en".to_string());
                                        settings.ui.locale = new_locale.clone();
                                        crate::set_locale(&new_locale);
                                        // Rebuild menu with new locale
                                        menu = SelectableList::new(
                                            t!("config.title").to_string(),
                                            menu_display_items(),
                                        );
                                    }
                                    panel = Panel::Menu;
                                    language_list = None;
                                }
                                ListAction::Cancel => {
                                    panel = Panel::Menu;
                                    language_list = None;
                                }
                                ListAction::None => {}
                            }
                        }
                    }
                }
            }
        }
    }
}

fn paste_into_current_input(panel: Panel, current_input: &mut Option<TextInput>, text: &str) {
    if matches!(
        panel,
        Panel::EditBaseUrl | Panel::EditApiKey | Panel::EditModel
    ) {
        if let Some(input) = current_input {
            input.paste(text);
        }
    }
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

fn base_url_list_for_provider(
    provider: &ProviderType,
    provider_mode: &Option<ProviderMode>,
) -> SelectableList<BaseUrlChoice> {
    SelectableList::new(
        t!("config.select_base_url").to_string(),
        base_url_choices(provider, provider_mode),
    )
}

fn base_url_choices(
    provider: &ProviderType,
    provider_mode: &Option<ProviderMode>,
) -> Vec<BaseUrlChoice> {
    match provider {
        ProviderType::Openai => vec![
            BaseUrlChoice {
                label: "OpenAI",
                url: Some(OPENAI_BASE_URL),
            },
            BaseUrlChoice {
                label: "Custom",
                url: None,
            },
        ],
        ProviderType::Anthropic => vec![
            BaseUrlChoice {
                label: "Anthropic",
                url: Some(ANTHROPIC_BASE_URL),
            },
            BaseUrlChoice {
                label: "Custom",
                url: None,
            },
        ],
        ProviderType::Google => vec![
            BaseUrlChoice {
                label: "Google",
                url: Some(GOOGLE_BASE_URL),
            },
            BaseUrlChoice {
                label: "Custom",
                url: None,
            },
        ],
        ProviderType::BigModel => {
            let url = match provider_mode {
                Some(ProviderMode::Coding) => BIGMODEL_CODING_BASE_URL,
                _ => BIGMODEL_GLM_BASE_URL,
            };
            vec![
                BaseUrlChoice {
                    label: "GLM",
                    url: Some(url),
                },
                BaseUrlChoice {
                    label: "Custom",
                    url: None,
                },
            ]
        }
        ProviderType::Custom => vec![BaseUrlChoice {
            label: "Custom",
            url: None,
        }],
    }
}

fn render_menu(
    frame: &mut ratatui::Frame,
    area: Rect,
    settings: &Settings,
    list: &mut SelectableList<String>,
) {
    let (left, right) = {
        let mid = area.width / 2;
        (
            Rect::new(area.x, area.y, mid, area.height),
            Rect::new(area.x + mid, area.y, area.width - mid, area.height),
        )
    };

    list.render(frame, left);

    let api_key_status = if settings.llm.api_key.is_some() {
        t!("config.api_key_set_mask").to_string()
    } else {
        t!("common.not_set").to_string()
    };

    let soul_display =
        crate::formula::current_project_soul().unwrap_or_else(|| settings.soul.role.to_string());

    let locale_display = LOCALES
        .iter()
        .find(|(code, _)| *code == settings.ui.locale)
        .map(|(_, name)| name.to_string())
        .unwrap_or_else(|| settings.ui.locale.clone());

    let mode_display = match (&settings.llm.provider, &settings.llm.provider_mode) {
        (ProviderType::BigModel, Some(ProviderMode::Coding)) => "Coding Plan".to_string(),
        (ProviderType::BigModel, _) => "General API".to_string(),
        _ => "-".to_string(),
    };

    let info = format!(
        "{}\n\n\
         {}:  {:?}\n\
         {}:  {}\n\
         {}:  {}\n\
         {}:   {}\n\
         {}:     {}\n\
         {}:      {}\n\
         {}:  {}\n\n\
         {}",
        t!("common.current_values"),
        t!("config.show_provider"),
        settings.llm.provider,
        t!("init.step.mode"),
        mode_display,
        t!("config.show_base_url"),
        settings.effective_base_url(),
        t!("config.show_api_key"),
        api_key_status,
        t!("config.show_model"),
        settings.llm.model,
        t!("config.show_soul"),
        soul_display,
        t!("config.show_language"),
        locale_display,
        t!("config.instructions"),
    );

    let info_widget = Paragraph::new(info)
        .style(theme::text())
        .block(
            Block::default()
                .borders(Borders::LEFT)
                .title(Span::styled(
                    format!(" {} ", t!("common.current_values")),
                    theme::title(),
                ))
                .style(theme::base())
                .border_style(theme::border(false)),
        )
        .wrap(ratatui::widgets::Wrap { trim: false });
    frame.render_widget(info_widget, right);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn test_menu_items_count() {
        assert_eq!(MENU_ITEMS.len(), 9);
    }

    #[test]
    fn test_provider_options() {
        assert_eq!(PROVIDERS.len(), 4);
        assert!(PROVIDERS.contains(&"openai"));
        assert!(PROVIDERS.contains(&"anthropic"));
        assert!(PROVIDERS.contains(&"google"));
        assert!(PROVIDERS.contains(&"bigmodel"));
    }

    #[test]
    fn test_provider_parse() {
        let p = match "bigmodel" {
            "openai" => ProviderType::Openai,
            "anthropic" => ProviderType::Anthropic,
            "google" => ProviderType::Google,
            "bigmodel" => ProviderType::BigModel,
            _ => ProviderType::Custom,
        };
        assert!(matches!(p, ProviderType::BigModel));
    }

    #[test]
    fn test_bigmodel_base_url_options() {
        let choices = base_url_choices(&ProviderType::BigModel, &None);
        assert_eq!(choices.len(), 2);
        assert_eq!(
            choices[0].url,
            Some("https://open.bigmodel.cn/api/paas/v4/")
        );
        assert_eq!(choices[1].url, None);
    }

    #[test]
    fn test_bigmodel_coding_base_url_options() {
        let choices = base_url_choices(&ProviderType::BigModel, &Some(ProviderMode::Coding));
        assert_eq!(choices.len(), 2);
        assert_eq!(
            choices[0].url,
            Some("https://open.bigmodel.cn/api/coding/paas/v4")
        );
    }

    #[test]
    fn test_base_url_input_starts_empty_for_builtin_default() {
        let mut settings = test_settings();
        settings.llm.provider = ProviderType::Openai;
        settings.llm.base_url = Some(OPENAI_BASE_URL.to_string());

        let input = base_url_input_for_settings(&settings);

        assert_eq!(input.value(), "");
    }

    #[test]
    fn test_base_url_input_preserves_existing_custom_url() {
        let mut settings = test_settings();
        settings.llm.provider = ProviderType::Openai;
        settings.llm.base_url = Some("http://localhost:11434/v1".to_string());

        let input = base_url_input_for_settings(&settings);

        assert_eq!(input.value(), "http://localhost:11434/v1");
    }

    #[test]
    fn test_base_url_input_accepts_paste() {
        let mut input = Some(TextInput::new("Base URL".to_string()));

        paste_into_current_input(Panel::EditBaseUrl, &mut input, "http://localhost:11434/v1");

        assert_eq!(
            input.as_ref().map(|input| input.value()),
            Some("http://localhost:11434/v1")
        );
    }

    #[test]
    fn test_paste_ignored_outside_text_input_panel() {
        let mut input = Some(TextInput::new("Base URL".to_string()));

        paste_into_current_input(Panel::Menu, &mut input, "http://localhost:11434/v1");

        assert_eq!(input.as_ref().map(|input| input.value()), Some(""));
    }

    #[test]
    fn test_settings_default_values() {
        let s = test_settings();
        assert_eq!(s.llm.model, "gpt-4o-mini");
        assert!(s.llm.api_key.is_none());
    }

    #[test]
    fn test_menu_selection_save() {
        let items: Vec<String> = MENU_ITEMS.iter().map(|s| s.to_string()).collect();
        let mut list = SelectableList::new("Test".to_string(), items);
        for _ in 0..7 {
            list.handle_key(KeyEvent::new(KeyCode::Down, event::KeyModifiers::NONE));
        }
        assert_eq!(list.selected().map(|s| s.as_str()), Some("Save & Exit"));
    }

    #[test]
    fn test_menu_selection_cancel() {
        let items: Vec<String> = MENU_ITEMS.iter().map(|s| s.to_string()).collect();
        let mut list = SelectableList::new("Test".to_string(), items);
        for _ in 0..8 {
            list.handle_key(KeyEvent::new(KeyCode::Down, event::KeyModifiers::NONE));
        }
        assert_eq!(list.selected().map(|s| s.as_str()), Some("Cancel"));
    }

    #[test]
    fn test_settings_modification() {
        let mut settings = test_settings();
        settings.llm.provider = ProviderType::BigModel;
        settings.llm.model = "glm-5.1".to_string();
        assert_eq!(settings.llm.model, "glm-5.1");
        assert!(matches!(settings.llm.provider, ProviderType::BigModel));
    }

    #[test]
    fn test_soul_menu_item_exists() {
        assert!(MENU_ITEMS.contains(&"Soul"));
        assert!(!MENU_ITEMS.contains(&"Soul Role"));
    }

    #[test]
    fn test_language_menu_item_exists() {
        assert!(MENU_ITEMS.contains(&"Language"));
    }
}
