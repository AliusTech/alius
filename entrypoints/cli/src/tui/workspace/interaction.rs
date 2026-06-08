use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use super::events::{DecisionKind, WorkspaceAction};
use crate::tui::state::{InteractionMode, MainTab};
use crate::tui::theme;

// ---------------------------------------------------------------------------
// InputBuffer
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default)]
pub struct InputBuffer {
    value: String,
    cursor: usize,
}

impl InputBuffer {
    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn is_empty(&self) -> bool {
        self.value.is_empty()
    }

    pub fn clear(&mut self) {
        self.value.clear();
        self.cursor = 0;
    }

    pub fn take(&mut self) -> String {
        let value = std::mem::take(&mut self.value);
        self.cursor = 0;
        value
    }

    pub fn cursor(&self) -> usize {
        self.cursor
    }

    pub fn insert(&mut self, c: char) {
        let byte_index = self.byte_index();
        self.value.insert(byte_index, c);
        self.cursor += 1;
    }

    pub fn replace_range(&mut self, start: usize, end: usize, replacement: &str) {
        let start_byte = self.byte_index_for(start);
        let end_byte = self.byte_index_for(end);
        self.value.replace_range(start_byte..end_byte, replacement);
        self.cursor = start + replacement.chars().count();
    }

    pub fn handle_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
                'a' => self.cursor = 0,
                'e' => self.cursor = self.char_len(),
                _ => {}
            },
            KeyCode::Char(c) => self.insert(c),
            KeyCode::Backspace if self.cursor > 0 => {
                self.cursor -= 1;
                let byte_index = self.byte_index();
                self.value.remove(byte_index);
            }
            KeyCode::Delete if self.cursor < self.char_len() => {
                let byte_index = self.byte_index();
                self.value.remove(byte_index);
            }
            KeyCode::Left => {
                self.cursor = self.cursor.saturating_sub(1);
            }
            KeyCode::Right => {
                self.cursor = (self.cursor + 1).min(self.char_len());
            }
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = self.char_len(),
            _ => {}
        }
    }

    pub fn display_with_cursor(&self, placeholder: &str) -> String {
        if self.value.is_empty() {
            return format!("│{}", placeholder);
        }

        let mut out = String::new();
        for (index, c) in self.value.chars().enumerate() {
            if index == self.cursor {
                out.push('│');
            }
            out.push(c);
        }
        if self.cursor == self.char_len() {
            out.push('│');
        }
        out
    }

    fn char_len(&self) -> usize {
        self.value.chars().count()
    }

    fn byte_index(&self) -> usize {
        self.byte_index_for(self.cursor)
    }

    fn byte_index_for(&self, cursor: usize) -> usize {
        self.value
            .char_indices()
            .nth(cursor)
            .map(|(index, _)| index)
            .unwrap_or_else(|| self.value.len())
    }
}

// ---------------------------------------------------------------------------
// DecisionState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct DecisionState {
    pub title: String,
    pub description: String,
    pub options: Vec<String>,
    pub selected: usize,
    pub custom_input: InputBuffer,
    pub custom_focused: bool,
    pub kind: DecisionKind,
}

impl DecisionState {
    pub fn plan_approval() -> Self {
        Self {
            title: t!("workspace.decision.title").to_string(),
            description: t!("workspace.decision.description").to_string(),
            options: vec![
                t!("workspace.decision.approve_execute").to_string(),
                t!("workspace.decision.revise_plan").to_string(),
                t!("workspace.decision.execute_selected").to_string(),
                t!("common.cancel").to_string(),
                t!("workspace.decision.custom_response").to_string(),
            ],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::PlanApproval,
        }
    }

    pub fn node_review() -> Self {
        Self {
            title: t!("workspace.node_review.title").to_string(),
            description: t!("workspace.node_review.description").to_string(),
            options: vec![
                t!("workspace.node_review.approve").to_string(),
                t!("workspace.node_review.request_revision").to_string(),
                t!("workspace.node_review.view_evidence").to_string(),
                t!("workspace.node_review.rerun").to_string(),
                t!("workspace.decision.custom_response").to_string(),
            ],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::NodeReview,
        }
    }

    pub fn init_menu() -> Self {
        Self {
            title: t!("workspace.init_menu.title").to_string(),
            description: t!("workspace.init_menu.description").to_string(),
            options: vec![t!("common.yes").to_string(), t!("common.no").to_string()],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::InitCommand,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        if self.custom_focused {
            match key.code {
                KeyCode::Esc => {
                    self.custom_focused = false;
                    return WorkspaceAction::None;
                }
                KeyCode::Enter => return self.confirm(),
                _ => {
                    self.custom_input.handle_key(key);
                    return WorkspaceAction::None;
                }
            }
        }

        match key.code {
            KeyCode::Up => {
                if self.selected > 0 {
                    self.selected -= 1;
                }
                WorkspaceAction::None
            }
            KeyCode::Down => {
                if self.selected + 1 < self.options.len() {
                    self.selected += 1;
                }
                WorkspaceAction::None
            }
            KeyCode::Tab => {
                self.selected = self.options.len().saturating_sub(1);
                self.custom_focused = true;
                WorkspaceAction::None
            }
            KeyCode::Enter => self.confirm(),
            KeyCode::Esc => WorkspaceAction::CancelDecision,
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if self.selected + 1 == self.options.len() {
                    self.custom_focused = true;
                    self.custom_input.insert(c);
                }
                WorkspaceAction::None
            }
            _ => WorkspaceAction::None,
        }
    }

    fn confirm(&mut self) -> WorkspaceAction {
        let custom = self.custom_input.value().to_string();
        match self.kind {
            DecisionKind::PlanApproval => match self.selected {
                0 => WorkspaceAction::ApprovePlan,
                1 => WorkspaceAction::RevisePlan(custom),
                2 => WorkspaceAction::ExecuteSelectedNodes,
                3 => WorkspaceAction::CancelDecision,
                _ => WorkspaceAction::RevisePlan(custom),
            },
            DecisionKind::NodeReview => match self.selected {
                0 => WorkspaceAction::ApproveReview,
                1 => WorkspaceAction::RequestRevision(custom),
                2 => WorkspaceAction::ViewEvidence,
                3 => WorkspaceAction::RerunNode,
                _ => WorkspaceAction::RequestRevision(custom),
            },
            DecisionKind::InitCommand => match self.selected {
                0 => WorkspaceAction::InitReconfigure,
                _ => WorkspaceAction::CancelDecision,
            },
        }
    }
}

// ---------------------------------------------------------------------------
// InteractionUi
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub enum InteractionUi {
    TextInput,
    Decision(DecisionState),
}

impl InteractionUi {
    pub fn height(&self, total_height: u16) -> u16 {
        let preferred = match self {
            Self::TextInput => 4,
            Self::Decision(_) => 10,
        };
        preferred.min(total_height.saturating_sub(7)).max(3)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub struct InteractionState<'a> {
    pub mode: InteractionMode,
    pub active_tab: MainTab,
    pub input: &'a InputBuffer,
    pub has_agent_team: bool,
    pub command_hint: Option<String>,
}

pub fn render_interaction(
    frame: &mut Frame,
    area: Rect,
    ui: &InteractionUi,
    state: &InteractionState<'_>,
    focused: bool,
    hovered: bool,
) {
    match ui {
        InteractionUi::TextInput => render_text_input(frame, area, state, focused, hovered),
        InteractionUi::Decision(decision) => render_decision(frame, area, decision, state, hovered),
    }
}

fn render_text_input(
    frame: &mut Frame,
    area: Rect,
    state: &InteractionState<'_>,
    focused: bool,
    hovered: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", state.mode.title()))
        .style(theme::base())
        .border_style(theme::border_state(focused, hovered));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let placeholder = state.mode.placeholder(state.active_tab);
    let input_line = state.input.display_with_cursor(&placeholder);
    let input_style = if state.input.is_empty() {
        theme::secondary()
    } else {
        theme::text()
    };
    frame.render_widget(
        Paragraph::new(input_line)
            .style(input_style)
            .wrap(Wrap { trim: false }),
        chunks[0],
    );

    let help = if let Some(command_hint) = &state.command_hint {
        command_hint.clone()
    } else if state.has_agent_team {
        t!("workspace.input_help.with_team").to_string()
    } else {
        t!("workspace.input_help.no_team").to_string()
    };
    frame.render_widget(
        Paragraph::new(help)
            .style(theme::secondary())
            .alignment(Alignment::Right),
        chunks[1],
    );
}

fn render_decision(
    frame: &mut Frame,
    area: Rect,
    decision: &DecisionState,
    state: &InteractionState<'_>,
    hovered: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} - {} ", state.mode.title(), decision.title))
        .style(theme::base())
        .border_style(if hovered {
            Style::default().fg(theme::ACCENT).bg(theme::BACKGROUND)
        } else {
            Style::default().fg(theme::WARNING).bg(theme::BACKGROUND)
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![Line::from(decision.description.clone()), Line::default()];

    for (index, option) in decision.options.iter().enumerate() {
        let selected = index == decision.selected;
        let symbol = if selected { "●" } else { "○" };
        let style = if selected {
            theme::emphasis()
        } else {
            theme::text()
        };
        lines.push(Line::from(vec![
            Span::styled(symbol, style),
            Span::raw(" "),
            Span::styled(option.as_str(), style),
        ]));

        if option.as_str() == t!("workspace.decision.custom_response") {
            let custom = decision
                .custom_input
                .display_with_cursor(if decision.custom_focused { "" } else { ">" });
            lines.push(Line::from(format!("  > {}", custom)));
        }
    }

    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        t!("workspace.decision.help").to_string(),
        theme::secondary(),
    )));

    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        inner,
    );
}

// ---------------------------------------------------------------------------
// InteractionMode helpers
// ---------------------------------------------------------------------------

impl InteractionMode {
    pub fn title(self) -> String {
        match self {
            Self::Plan => t!("workspace.mode.plan").to_string(),
            Self::Bypass => t!("workspace.mode.bypass").to_string(),
        }
    }

    pub fn placeholder(self, active_tab: MainTab) -> String {
        match (active_tab, self) {
            (MainTab::AgentTeam, Self::Plan) => t!("workspace.placeholder.team_plan").to_string(),
            (MainTab::AgentTeam, Self::Bypass) => {
                t!("workspace.placeholder.team_bypass").to_string()
            }
            (_, Self::Plan) => t!("workspace.placeholder.plan").to_string(),
            (_, Self::Bypass) => t!("workspace.placeholder.bypass").to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_buffer_handles_unicode_cursor_edits() {
        let mut input = InputBuffer::default();
        input.insert('你');
        input.insert('好');
        input.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.insert('很');

        assert_eq!(input.value(), "你很好");
    }

    #[test]
    fn input_buffer_replaces_unicode_char_range() {
        let mut input = InputBuffer::default();
        input.insert('/');
        input.insert('模');
        input.insert('式');

        input.replace_range(1, 3, "mode");

        assert_eq!(input.value(), "/mode");
        assert_eq!(input.cursor(), 5);
    }
}
