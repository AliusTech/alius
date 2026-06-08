use anyhow::Result;
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders};
use rust_i18n::t;

use crate::tui::app::TuiApp;
use crate::tui::components::{
    should_process_key_event, InputAction, ListAction, SelectableList, TextInput,
};
use crate::tui::theme;

/// Sentinel used to detect manual-entry selection. Not translated.
const MANUAL_ENTRY_SENTINEL: &str = "< Enter model name manually >";

pub async fn select_model_from_models(
    models: Vec<String>,
    current: &str,
) -> Result<Option<String>> {
    let current = current.to_string();
    tokio::task::spawn_blocking(move || {
        let mut app = TuiApp::enter()?;
        let result = run_select_loop(&mut app, models, &current);
        app.restore()?;
        result
    })
    .await?
}

fn run_select_loop(app: &mut TuiApp, models: Vec<String>, current: &str) -> Result<Option<String>> {
    let mut list = if models.is_empty() {
        SelectableList::new(
            t!("model_select.select_model_empty").to_string(),
            vec![MANUAL_ENTRY_SENTINEL.to_string()],
        )
    } else {
        let mut list =
            SelectableList::new(t!("model_select.select_model").to_string(), models.clone());
        if let Some(idx) = models.iter().position(|m| m == current) {
            for _ in 0..idx {
                list.handle_key(KeyEvent::new(
                    KeyCode::Down,
                    crossterm::event::KeyModifiers::NONE,
                ));
            }
        }
        list
    };
    let mut manual_input: Option<TextInput> = None;

    loop {
        app.draw(|frame| {
            let area = frame.area();
            let block = Block::default()
                .borders(Borders::ALL)
                .title(Span::styled(
                    format!(" {} ", t!("model_select.title")),
                    theme::title(),
                ))
                .style(theme::base())
                .border_style(theme::border(true))
                .title_alignment(Alignment::Center);
            let inner = block.inner(area);
            frame.render_widget(block, area);

            if let Some(ref input) = manual_input {
                let input_area = Rect::new(
                    inner.x + 1,
                    inner.y + inner.height / 2,
                    inner.width.saturating_sub(2),
                    TextInput::HEIGHT,
                );
                let mut display = input.clone();
                display.set_active(true);
                display.render(frame, input_area);
            } else {
                list.render(frame, inner);
            }
        })?;

        if event::poll(std::time::Duration::from_millis(100))? {
            let input_event = event::read()?;
            if let Event::Paste(text) = input_event {
                if let Some(ref mut input) = manual_input {
                    input.paste(&text);
                }
                continue;
            }
            if let Event::Key(key) = input_event {
                if !should_process_key_event(&key) {
                    continue;
                }
                if let Some(ref mut input) = manual_input {
                    match input.handle_key(key) {
                        InputAction::Submit => {
                            let value = input.value().to_string();
                            if !value.is_empty() {
                                return Ok(Some(value));
                            }
                        }
                        InputAction::Cancel => {
                            manual_input = None;
                        }
                        InputAction::None => {}
                    }
                } else {
                    match list.handle_key(key) {
                        ListAction::Select => {
                            if let Some(selected) = list.selected() {
                                if selected == MANUAL_ENTRY_SENTINEL {
                                    manual_input = Some(
                                        TextInput::new(
                                            t!("model_select.model_name_label").to_string(),
                                        )
                                        .with_default(current),
                                    );
                                } else {
                                    return Ok(Some(selected.clone()));
                                }
                            }
                        }
                        ListAction::Cancel => return Ok(None),
                        ListAction::None => {}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_creation_with_models() {
        let models = vec!["gpt-4o".to_string(), "gpt-4o-mini".to_string()];
        let list = SelectableList::new("Test".to_string(), models.clone());
        assert_eq!(list.selected(), Some(&"gpt-4o".to_string()));
    }

    #[test]
    fn test_list_creation_empty() {
        let list: SelectableList<String> = SelectableList::new(
            "Select Model (no models from API, enter manually)".to_string(),
            vec![MANUAL_ENTRY_SENTINEL.to_string()],
        );
        assert_eq!(list.selected(), Some(&MANUAL_ENTRY_SENTINEL.to_string()));
    }

    #[test]
    fn test_manual_input_select() {
        let mut list: SelectableList<String> =
            SelectableList::new("Test".to_string(), vec![MANUAL_ENTRY_SENTINEL.to_string()]);
        let action = list.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, ListAction::Select);
        assert_eq!(list.selected(), Some(&MANUAL_ENTRY_SENTINEL.to_string()));
    }

    #[test]
    fn test_manual_input_submit() {
        let mut input = TextInput::new("Model:".to_string()).with_default("my-model");
        let action = input.handle_key(KeyEvent::new(
            KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, InputAction::Submit);
        assert_eq!(input.value(), "my-model");
    }

    #[test]
    fn test_manual_input_cancel() {
        let mut input = TextInput::new("Model:".to_string());
        let action = input.handle_key(KeyEvent::new(
            KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        ));
        assert_eq!(action, InputAction::Cancel);
    }

    #[test]
    fn test_current_model_positioning() {
        let models = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut list = SelectableList::new("Test".to_string(), models);
        for _ in 0..1 {
            list.handle_key(KeyEvent::new(
                KeyCode::Down,
                crossterm::event::KeyModifiers::NONE,
            ));
        }
        assert_eq!(list.selected(), Some(&"b".to_string()));
    }
}
