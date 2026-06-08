use crossterm::event::{KeyCode, KeyEvent};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, List, ListItem, ListState, Paragraph, Wrap};
use rust_i18n::t;
use std::fmt;

use crate::tui::theme;

#[derive(Debug, PartialEq, Eq)]
pub enum ListAction {
    Select,
    Cancel,
    None,
}

pub struct SelectableList<T: Clone + fmt::Display> {
    title: String,
    items: Vec<T>,
    filtered_indices: Vec<usize>,
    selected: usize,
    scroll_offset: usize,
    search_query: String,
    search_mode: bool,
}

impl<T: Clone + fmt::Display> SelectableList<T> {
    pub fn new(title: String, items: Vec<T>) -> Self {
        let filtered_indices = (0..items.len()).collect();
        Self {
            title,
            items,
            filtered_indices,
            selected: 0,
            scroll_offset: 0,
            search_query: String::new(),
            search_mode: false,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> ListAction {
        if self.search_mode {
            match key.code {
                KeyCode::Esc => {
                    self.search_mode = false;
                    self.search_query.clear();
                    self.filtered_indices = (0..self.items.len()).collect();
                    self.selected = 0;
                    self.scroll_offset = 0;
                    ListAction::None
                }
                KeyCode::Enter => {
                    if self.selected_index().is_some() {
                        ListAction::Select
                    } else {
                        ListAction::None
                    }
                }
                KeyCode::Backspace => {
                    self.search_query.pop();
                    self.apply_filter();
                    ListAction::None
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_up();
                    ListAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_down();
                    ListAction::None
                }
                KeyCode::Char(c) => {
                    self.search_query.push(c);
                    self.apply_filter();
                    ListAction::None
                }
                _ => ListAction::None,
            }
        } else {
            match key.code {
                KeyCode::Up | KeyCode::Char('k') => {
                    self.move_up();
                    ListAction::None
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    self.move_down();
                    ListAction::None
                }
                KeyCode::Enter => {
                    if self.selected_index().is_some() {
                        ListAction::Select
                    } else {
                        ListAction::None
                    }
                }
                KeyCode::Esc => ListAction::Cancel,
                KeyCode::Char('/') => {
                    self.search_mode = true;
                    ListAction::None
                }
                _ => ListAction::None,
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame, area: Rect) {
        let search_bar_height = if self.search_mode { 1 } else { 0 };
        let title_area = Rect::new(area.x, area.y, area.width, 1);
        let search_area = Rect::new(area.x, area.y + 1, area.width, search_bar_height);
        let list_area = Rect::new(
            area.x,
            area.y + 1 + search_bar_height,
            area.width,
            area.height.saturating_sub(1 + search_bar_height),
        );

        let title_widget = Paragraph::new(Span::styled(&self.title, theme::title()));
        frame.render_widget(title_widget, title_area);

        if self.search_mode {
            let search_text = format!("/{}", self.search_query);
            let search_widget = Paragraph::new(Span::styled(search_text, theme::emphasis()));
            frame.render_widget(search_widget, search_area);
        }

        let visible_height = list_area.height.saturating_sub(2) as usize;

        if self.filtered_indices.is_empty() {
            let block = Block::default()
                .borders(Borders::ALL)
                .style(theme::base())
                .border_style(theme::border(false));
            let inner = block.inner(list_area);
            frame.render_widget(block, list_area);
            let empty_msg = Paragraph::new(t!("common.no_items_found").to_string())
                .style(theme::secondary())
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true });
            frame.render_widget(empty_msg, inner);
            return;
        }

        let total = self.filtered_indices.len();
        let max_scroll = total.saturating_sub(visible_height);
        if self.scroll_offset > max_scroll {
            self.scroll_offset = max_scroll;
        }

        let list_items: Vec<ListItem> = self
            .filtered_indices
            .iter()
            .skip(self.scroll_offset)
            .take(visible_height)
            .enumerate()
            .map(|(i, &idx)| {
                let display = self.items[idx].to_string();
                let is_selected = self.scroll_offset + i == self.selected;
                if is_selected {
                    ListItem::new(Span::styled(display, theme::selected()))
                } else {
                    ListItem::new(Span::styled(display, theme::text()))
                }
            })
            .collect();

        let scroll_indicator = if total > visible_height {
            format!(" [{}/{}]", self.selected + 1, total)
        } else {
            String::new()
        };

        let block_title = if scroll_indicator.is_empty() {
            String::new()
        } else {
            scroll_indicator
        };

        let block = Block::default()
            .borders(Borders::ALL)
            .title(Span::styled(block_title, theme::secondary()))
            .style(theme::base())
            .border_style(theme::border(false));

        let list_widget = List::new(list_items).block(block);
        let mut state = ListState::default();
        state.select(Some(self.selected.saturating_sub(self.scroll_offset)));
        frame.render_stateful_widget(list_widget, list_area, &mut state);
    }

    pub fn selected(&self) -> Option<&T> {
        self.selected_index().map(|idx| &self.items[idx])
    }

    #[allow(dead_code)]
    pub fn set_items(&mut self, items: Vec<T>) {
        self.items = items;
        self.selected = 0;
        self.scroll_offset = 0;
        self.apply_filter();
    }

    fn selected_index(&self) -> Option<usize> {
        self.filtered_indices.get(self.selected).copied()
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
            self.adjust_scroll();
        }
    }

    fn move_down(&mut self) {
        if !self.filtered_indices.is_empty() && self.selected < self.filtered_indices.len() - 1 {
            self.selected += 1;
            self.adjust_scroll();
        }
    }

    fn adjust_scroll(&mut self) {
        let visible_height = 10;
        if self.selected < self.scroll_offset {
            self.scroll_offset = self.selected;
        } else if self.selected >= self.scroll_offset + visible_height {
            self.scroll_offset = self.selected - visible_height + 1;
        }
    }

    fn apply_filter(&mut self) {
        let query = self.search_query.to_lowercase();
        self.filtered_indices = if query.is_empty() {
            (0..self.items.len()).collect()
        } else {
            self.items
                .iter()
                .enumerate()
                .filter(|(_, item)| item.to_string().to_lowercase().contains(&query))
                .map(|(i, _)| i)
                .collect()
        };
        self.selected = 0;
        self.scroll_offset = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    #[test]
    fn test_new_list() {
        let list: SelectableList<String> = SelectableList::new(
            "Test".to_string(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.filtered_indices.len(), 3);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_navigation_down() {
        let mut list: SelectableList<String> = SelectableList::new(
            "Test".to_string(),
            vec!["a".to_string(), "b".to_string(), "c".to_string()],
        );
        assert_eq!(list.handle_key(key(KeyCode::Down)), ListAction::None);
        assert_eq!(list.selected, 1);
        assert_eq!(list.handle_key(key(KeyCode::Down)), ListAction::None);
        assert_eq!(list.selected, 2);
        assert_eq!(list.handle_key(key(KeyCode::Down)), ListAction::None);
        assert_eq!(list.selected, 2);
    }

    #[test]
    fn test_navigation_up() {
        let mut list: SelectableList<String> =
            SelectableList::new("Test".to_string(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(list.handle_key(key(KeyCode::Down)), ListAction::None);
        assert_eq!(list.handle_key(key(KeyCode::Up)), ListAction::None);
        assert_eq!(list.selected, 0);
        assert_eq!(list.handle_key(key(KeyCode::Up)), ListAction::None);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_j_k_navigation() {
        let mut list: SelectableList<String> =
            SelectableList::new("Test".to_string(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(list.handle_key(key(KeyCode::Char('j'))), ListAction::None);
        assert_eq!(list.selected, 1);
        assert_eq!(list.handle_key(key(KeyCode::Char('k'))), ListAction::None);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_select() {
        let mut list: SelectableList<String> =
            SelectableList::new("Test".to_string(), vec!["a".to_string(), "b".to_string()]);
        assert_eq!(list.handle_key(key(KeyCode::Down)), ListAction::None);
        assert_eq!(list.handle_key(key(KeyCode::Enter)), ListAction::Select);
        assert_eq!(list.selected(), Some(&"b".to_string()));
    }

    #[test]
    fn test_cancel() {
        let mut list: SelectableList<String> =
            SelectableList::new("Test".to_string(), vec!["a".to_string()]);
        assert_eq!(list.handle_key(key(KeyCode::Esc)), ListAction::Cancel);
    }

    #[test]
    fn test_search_mode() {
        let mut list: SelectableList<String> = SelectableList::new(
            "Test".to_string(),
            vec![
                "apple".to_string(),
                "banana".to_string(),
                "cherry".to_string(),
            ],
        );
        assert_eq!(list.handle_key(key(KeyCode::Char('/'))), ListAction::None);
        assert!(list.search_mode);
        assert_eq!(list.handle_key(key(KeyCode::Char('a'))), ListAction::None);
        assert_eq!(list.filtered_indices.len(), 2);
        assert_eq!(list.handle_key(key(KeyCode::Esc)), ListAction::None);
        assert!(!list.search_mode);
        assert_eq!(list.filtered_indices.len(), 3);
    }

    #[test]
    fn test_search_backspace() {
        let mut list: SelectableList<String> = SelectableList::new(
            "Test".to_string(),
            vec!["apple".to_string(), "banana".to_string()],
        );
        assert_eq!(list.handle_key(key(KeyCode::Char('/'))), ListAction::None);
        assert_eq!(list.handle_key(key(KeyCode::Char('a'))), ListAction::None);
        assert_eq!(list.filtered_indices.len(), 2);
        assert_eq!(list.handle_key(key(KeyCode::Backspace)), ListAction::None);
        assert_eq!(list.filtered_indices.len(), 2);
    }

    #[test]
    fn test_set_items() {
        let mut list: SelectableList<String> =
            SelectableList::new("Test".to_string(), vec!["a".to_string()]);
        list.set_items(vec!["x".to_string(), "y".to_string(), "z".to_string()]);
        assert_eq!(list.items.len(), 3);
        assert_eq!(list.selected, 0);
    }

    #[test]
    fn test_empty_list_select() {
        let mut list: SelectableList<String> = SelectableList::new("Test".to_string(), vec![]);
        assert_eq!(list.handle_key(key(KeyCode::Enter)), ListAction::None);
        assert!(list.selected().is_none());
    }
}
