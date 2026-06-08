use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::tui::theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputAction {
    Submit,
    Cancel,
    None,
}

pub fn should_process_key_event(key: &KeyEvent) -> bool {
    !matches!(key.kind, KeyEventKind::Release)
}

#[derive(Clone)]
pub struct TextInput {
    value: String,
    cursor: usize,
    label: String,
    placeholder: Option<String>,
    password_mode: bool,
    mask_mode: bool,
    active: bool,
}

impl TextInput {
    pub const HEIGHT: u16 = 4;

    pub fn new(label: String) -> Self {
        Self {
            value: String::new(),
            cursor: 0,
            label,
            placeholder: None,
            password_mode: false,
            mask_mode: false,
            active: false,
        }
    }

    pub fn with_default(mut self, default: &str) -> Self {
        self.value = default.to_string();
        self.cursor = self.char_len();
        self
    }

    #[allow(dead_code)]
    pub fn with_placeholder(mut self, placeholder: &str) -> Self {
        self.placeholder = Some(placeholder.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn password(mut self) -> Self {
        self.password_mode = true;
        self
    }

    #[allow(dead_code)]
    pub fn mask(mut self) -> Self {
        self.mask_mode = true;
        self
    }

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let byte_index = self.byte_index();
        self.value.insert_str(byte_index, text);
        self.cursor += text.chars().count();
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    #[allow(dead_code)]
    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    fn char_len(&self) -> usize {
        self.value.chars().count()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> InputAction {
        match key.code {
            KeyCode::Enter => InputAction::Submit,
            KeyCode::Esc => InputAction::Cancel,
            KeyCode::Char(c) => {
                if key.modifiers.contains(KeyModifiers::CONTROL) {
                    match c.to_ascii_lowercase() {
                        'a' => {
                            self.cursor = 0;
                            return InputAction::None;
                        }
                        'e' => {
                            self.cursor = self.char_len();
                            return InputAction::None;
                        }
                        _ => {}
                    }
                }
                let byte_index = self.byte_index();
                self.value.insert(byte_index, c);
                self.cursor += 1;
                InputAction::None
            }
            KeyCode::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    let byte_index = self.byte_index();
                    self.value.remove(byte_index);
                }
                InputAction::None
            }
            KeyCode::Delete => {
                if self.cursor < self.char_len() {
                    let byte_index = self.byte_index();
                    self.value.remove(byte_index);
                }
                InputAction::None
            }
            KeyCode::Left => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                }
                InputAction::None
            }
            KeyCode::Right => {
                if self.cursor < self.char_len() {
                    self.cursor += 1;
                }
                InputAction::None
            }
            KeyCode::Home => {
                self.cursor = 0;
                InputAction::None
            }
            KeyCode::End => {
                self.cursor = self.char_len();
                InputAction::None
            }
            _ => InputAction::None,
        }
    }

    fn display_value(&self) -> String {
        if self.password_mode {
            "*".repeat(self.value.chars().count())
        } else if self.mask_mode && self.value.len() > 10 {
            let chars: Vec<char> = self.value.chars().collect();
            let head: String = chars.iter().take(4).collect();
            let tail: String = chars
                .iter()
                .rev()
                .take(3)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect();
            format!("{}...{}", head, tail)
        } else {
            self.value.clone()
        }
    }

    fn byte_index(&self) -> usize {
        self.value
            .char_indices()
            .nth(self.cursor)
            .map(|(i, _)| i)
            .unwrap_or_else(|| self.value.len())
    }

    pub fn render(&self, frame: &mut Frame, area: Rect) {
        let label = Paragraph::new(self.label.as_str()).style(theme::title());

        let input_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height.saturating_sub(1),
        };

        frame.render_widget(label, area);

        let block = Block::default()
            .borders(Borders::ALL)
            .style(theme::base())
            .border_style(theme::border(self.active));

        let display = self.display_value();

        let inner_display = if display.is_empty() {
            self.placeholder.as_deref().unwrap_or("").to_string()
        } else {
            display
        };

        let style = if self.value.is_empty() && self.placeholder.is_some() {
            theme::secondary()
        } else {
            theme::text()
        };

        let cursor_char = if self.active { "\u{2502}" } else { "" };

        let display_with_cursor = if self.active {
            let chars: Vec<char> = inner_display.chars().collect();
            let left: String = chars.iter().take(self.cursor).collect();
            let right: String = chars.iter().skip(self.cursor).collect();
            format!("{}{}{}", left, cursor_char, right)
        } else {
            inner_display
        };

        let paragraph = Paragraph::new(display_with_cursor)
            .style(style)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, input_area);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn ctrl_key(c: char) -> KeyEvent {
        KeyEvent::new(KeyCode::Char(c), KeyModifiers::CONTROL)
    }

    fn key_with_kind(code: KeyCode, kind: KeyEventKind) -> KeyEvent {
        KeyEvent::new_with_kind(code, KeyModifiers::NONE, kind)
    }

    #[test]
    fn test_new_input() {
        let input = TextInput::new("Label".to_string());
        assert_eq!(input.value(), "");
        assert!(!input.is_active());
    }

    #[test]
    fn test_with_default() {
        let input = TextInput::new("Label".to_string()).with_default("hello");
        assert_eq!(input.value(), "hello");
    }

    #[test]
    fn test_type_chars() {
        let mut input = TextInput::new("Test".to_string());
        assert_eq!(input.handle_key(key(KeyCode::Char('a'))), InputAction::None);
        assert_eq!(input.handle_key(key(KeyCode::Char('b'))), InputAction::None);
        assert_eq!(input.value(), "ab");
    }

    #[test]
    fn test_processes_press_and_repeat_key_events() {
        assert!(should_process_key_event(&key_with_kind(
            KeyCode::Char('a'),
            KeyEventKind::Press
        )));
        assert!(should_process_key_event(&key_with_kind(
            KeyCode::Char('a'),
            KeyEventKind::Repeat
        )));
    }

    #[test]
    fn test_ignores_release_key_events() {
        assert!(!should_process_key_event(&key_with_kind(
            KeyCode::Char('a'),
            KeyEventKind::Release
        )));
    }

    #[test]
    fn test_backspace() {
        let mut input = TextInput::new("Test".to_string()).with_default("abc");
        assert_eq!(input.handle_key(key(KeyCode::Backspace)), InputAction::None);
        assert_eq!(input.value(), "ab");
    }

    #[test]
    fn test_delete() {
        let mut input = TextInput::new("Test".to_string()).with_default("abc");
        assert_eq!(input.handle_key(key(KeyCode::Left)), InputAction::None);
        assert_eq!(input.handle_key(key(KeyCode::Delete)), InputAction::None);
        assert_eq!(input.value(), "ab");
    }

    #[test]
    fn test_cursor_movement() {
        let mut input = TextInput::new("Test".to_string()).with_default("abc");
        assert_eq!(input.handle_key(key(KeyCode::Home)), InputAction::None);
        assert_eq!(input.handle_key(key(KeyCode::Char('x'))), InputAction::None);
        assert_eq!(input.value(), "xabc");
        assert_eq!(input.handle_key(key(KeyCode::End)), InputAction::None);
        assert_eq!(input.handle_key(key(KeyCode::Char('y'))), InputAction::None);
        assert_eq!(input.value(), "xabcy");
    }

    #[test]
    fn test_ctrl_a_ctrl_e() {
        let mut input = TextInput::new("Test".to_string()).with_default("abc");
        assert_eq!(input.handle_key(ctrl_key('a')), InputAction::None);
        assert_eq!(input.handle_key(key(KeyCode::Char('X'))), InputAction::None);
        assert_eq!(input.value(), "Xabc");
        assert_eq!(input.handle_key(ctrl_key('e')), InputAction::None);
        assert_eq!(input.handle_key(key(KeyCode::Char('Y'))), InputAction::None);
        assert_eq!(input.value(), "XabcY");
    }

    #[test]
    fn test_unknown_control_modified_chars_are_inserted() {
        let mut input = TextInput::new("Test".to_string());

        assert_eq!(input.handle_key(ctrl_key('s')), InputAction::None);

        assert_eq!(input.value(), "s");
    }

    #[test]
    fn test_submit() {
        let mut input = TextInput::new("Test".to_string());
        assert_eq!(input.handle_key(key(KeyCode::Enter)), InputAction::Submit);
    }

    #[test]
    fn test_cancel() {
        let mut input = TextInput::new("Test".to_string());
        assert_eq!(input.handle_key(key(KeyCode::Esc)), InputAction::Cancel);
    }

    #[test]
    fn test_password_mode() {
        let input = TextInput::new("Key".to_string())
            .password()
            .with_default("secret");
        assert_eq!(input.value(), "secret");
        assert!(input.password_mode);
    }

    #[test]
    fn test_render_has_visible_content_row_at_component_height() {
        let backend = TestBackend::new(24, TextInput::HEIGHT);
        let mut terminal = Terminal::new(backend).expect("test backend");
        let mut input = TextInput::new("API Key:".to_string()).with_default("sk-test");
        input.set_active(true);

        terminal
            .draw(|frame| {
                input.render(frame, frame.area());
            })
            .expect("draw text input");

        let rendered = format!("{:?}", terminal.backend().buffer());
        assert!(rendered.contains("API Key:"));
        assert!(rendered.contains("sk-test"));
    }
}
