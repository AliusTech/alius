use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use super::config_task::{ConfigSection, InitNavSnapshot};
use super::events::{DecisionKind, WorkspaceAction};
use crate::tui::state::{InteractionMode, MainTab};
use crate::tui::theme;
use runtime_config::InitStage;

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

    pub fn set_value(&mut self, value: impl Into<String>) {
        self.value = value.into();
        self.cursor = self.char_len();
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

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        let byte_index = self.byte_index();
        self.value.insert_str(byte_index, text);
        self.cursor += text.chars().count();
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

    pub fn display_masked_with_cursor(&self, placeholder: &str) -> String {
        if self.value.is_empty() {
            return format!("│{}", placeholder);
        }

        let mut out = String::new();
        for index in 0..self.char_len() {
            if index == self.cursor {
                out.push('│');
            }
            out.push('•');
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
    pub scope_title: Option<String>,
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
            scope_title: None,
            options: vec![
                t!("workspace.decision.approve_execute").to_string(),
                t!("workspace.decision.revise_plan").to_string(),
                t!("common.cancel").to_string(),
            ],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::PlanApproval,
        }
    }

    pub fn plan_completion() -> Self {
        Self {
            title: t!("workspace.plan_completion.title").to_string(),
            scope_title: None,
            options: vec![t!("workspace.plan_completion.close").to_string()],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::PlanCompletion,
        }
    }

    #[allow(dead_code)]
    pub fn node_review() -> Self {
        Self {
            title: t!("workspace.node_review.title").to_string(),
            scope_title: None,
            options: vec![
                t!("workspace.node_review.approve").to_string(),
                t!("workspace.node_review.request_revision").to_string(),
                t!("workspace.node_review.view_evidence").to_string(),
                t!("workspace.node_review.rerun").to_string(),
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
            scope_title: None,
            options: vec![t!("common.yes").to_string(), t!("common.no").to_string()],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::InitCommand,
        }
    }

    pub fn execution_interrupt() -> Self {
        Self {
            title: t!("workspace.execution_interrupt.title").to_string(),
            scope_title: Some(t!("workspace.execution_interrupt.input_title").to_string()),
            options: vec![
                t!("workspace.execution_interrupt.confirm").to_string(),
                t!("workspace.execution_interrupt.continue_waiting").to_string(),
            ],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::ExecutionInterrupt,
        }
    }

    pub fn config_exit() -> Self {
        Self {
            title: "Exit configuration?".to_string(),
            scope_title: None,
            options: vec![
                "Exit without saving".to_string(),
                "Continue configuration".to_string(),
            ],
            selected: 1,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::ConfigExit,
        }
    }

    pub fn quit_confirm() -> Self {
        Self {
            title: t!("workspace.quit_confirm.title").to_string(),
            scope_title: None,
            options: vec![
                t!("workspace.quit_confirm.cancel").to_string(),
                t!("workspace.quit_confirm.confirm").to_string(),
            ],
            selected: 0,
            custom_input: InputBuffer::default(),
            custom_focused: false,
            kind: DecisionKind::QuitConfirm,
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        if self.kind == DecisionKind::QuitConfirm {
            return self.handle_quit_confirm_key(key);
        }

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
                self.custom_focused = !self.custom_focused;
                WorkspaceAction::None
            }
            KeyCode::Enter => self.confirm(),
            KeyCode::Esc if self.kind == DecisionKind::ConfigExit => {
                WorkspaceAction::ContinueConfig
            }
            KeyCode::Esc => WorkspaceAction::CancelDecision,
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.custom_focused = true;
                self.custom_input.insert(c);
                WorkspaceAction::None
            }
            _ => WorkspaceAction::None,
        }
    }

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.kind == DecisionKind::QuitConfirm {
            return;
        }
        self.custom_focused = true;
        self.custom_input.paste(text);
    }

    fn handle_quit_confirm_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        match key.code {
            KeyCode::Up | KeyCode::Down | KeyCode::Tab => {
                self.selected = 1 - self.selected.min(1);
                WorkspaceAction::None
            }
            KeyCode::Enter => self.confirm(),
            KeyCode::Esc => WorkspaceAction::CancelDecision,
            _ => WorkspaceAction::None,
        }
    }

    fn confirm(&mut self) -> WorkspaceAction {
        let custom = self.custom_input.value().trim().to_string();
        if !custom.is_empty() {
            return match self.kind {
                DecisionKind::PlanApproval => WorkspaceAction::RevisePlan(custom),
                DecisionKind::NodeReview => WorkspaceAction::RequestRevision(custom),
                DecisionKind::InitCommand => WorkspaceAction::CancelDecision,
                DecisionKind::ExecutionInterrupt => WorkspaceAction::ContinueExecution,
                DecisionKind::ConfigExit => WorkspaceAction::ContinueConfig,
                DecisionKind::QuitConfirm => WorkspaceAction::CancelDecision,
                DecisionKind::PlanCompletion => WorkspaceAction::ClosePlan,
            };
        }

        match self.kind {
            DecisionKind::PlanApproval => match self.selected {
                0 => WorkspaceAction::ApprovePlan,
                1 => WorkspaceAction::RevisePlan(String::new()),
                2 => WorkspaceAction::CancelDecision,
                _ => WorkspaceAction::CancelDecision,
            },
            DecisionKind::NodeReview => match self.selected {
                0 => WorkspaceAction::ApproveReview,
                1 => WorkspaceAction::RequestRevision(String::new()),
                2 => WorkspaceAction::ViewEvidence,
                3 => WorkspaceAction::RerunNode,
                _ => WorkspaceAction::ViewEvidence,
            },
            DecisionKind::InitCommand => match self.selected {
                0 => WorkspaceAction::InitReconfigure,
                _ => WorkspaceAction::CancelDecision,
            },
            DecisionKind::ExecutionInterrupt => match self.selected {
                0 => WorkspaceAction::InterruptExecution,
                _ => WorkspaceAction::ContinueExecution,
            },
            DecisionKind::ConfigExit => match self.selected {
                0 => WorkspaceAction::Submit("/cancel".to_string()),
                _ => WorkspaceAction::ContinueConfig,
            },
            DecisionKind::QuitConfirm => match self.selected {
                0 => WorkspaceAction::CancelDecision,
                1 => WorkspaceAction::Quit,
                _ => WorkspaceAction::CancelDecision,
            },
            DecisionKind::PlanCompletion => WorkspaceAction::ClosePlan,
        }
    }
}

// ---------------------------------------------------------------------------
// PromptInputState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptInputKind {
    Text {
        masked: bool,
    },
    SingleSelect,
    SingleSelectWithInput {
        masked: bool,
    },
    #[allow(dead_code)]
    MultiSelect,
    #[allow(dead_code)]
    MultiSelectWithInput {
        masked: bool,
    },
}

impl PromptInputKind {
    fn allows_custom_input(&self) -> bool {
        matches!(
            self,
            Self::Text { .. }
                | Self::SingleSelectWithInput { .. }
                | Self::MultiSelectWithInput { .. }
        )
    }

    fn is_multi(&self) -> bool {
        matches!(self, Self::MultiSelect | Self::MultiSelectWithInput { .. })
    }

    fn is_masked(&self) -> bool {
        matches!(
            self,
            Self::Text { masked: true }
                | Self::SingleSelectWithInput { masked: true }
                | Self::MultiSelectWithInput { masked: true }
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptChoice {
    pub label: String,
    pub value: String,
    pub selected: bool,
}

impl PromptChoice {
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            selected: false,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PromptInputState {
    pub title: String,
    pub scope_title: Option<String>,
    pub help: String,
    pub placeholder: String,
    pub kind: PromptInputKind,
    pub choices: Vec<PromptChoice>,
    pub highlighted: usize,
    pub custom_input: InputBuffer,
    pub input_focused: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromptInputAction {
    None,
    Submit(String),
    Cancel,
}

impl PromptInputState {
    pub fn new(
        title: impl Into<String>,
        kind: PromptInputKind,
        choices: Vec<PromptChoice>,
        help: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            scope_title: None,
            help: help.into(),
            placeholder: String::new(),
            kind,
            choices,
            highlighted: 0,
            custom_input: InputBuffer::default(),
            input_focused: false,
        }
    }

    pub fn with_scope_title(mut self, scope_title: impl Into<String>) -> Self {
        self.scope_title = Some(scope_title.into());
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_input_value(mut self, value: impl Into<String>) -> Self {
        self.custom_input.set_value(value);
        self
    }

    pub fn with_highlighted_value(mut self, value: &str) -> Self {
        if let Some(index) = self
            .choices
            .iter()
            .position(|choice| choice.value.eq_ignore_ascii_case(value.trim()))
        {
            self.highlighted = index;
        }
        self
    }

    pub fn preferred_height(&self, total_height: u16) -> u16 {
        let choice_lines = self.choices.len().min(5) as u16;
        let input_lines = if self.kind.allows_custom_input() && self.choices.is_empty() {
            1
        } else if self.kind.allows_custom_input() {
            2
        } else {
            0
        };
        let preferred = 3 + choice_lines + input_lines;
        preferred.min(total_height.saturating_sub(7)).max(4)
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> PromptInputAction {
        match key.code {
            KeyCode::Esc => return PromptInputAction::Cancel,
            KeyCode::Enter => return PromptInputAction::Submit(self.submit_value()),
            KeyCode::Up if !self.choices.is_empty() => {
                self.input_focused = false;
                self.highlighted = if self.highlighted == 0 {
                    self.choices.len().saturating_sub(1)
                } else {
                    self.highlighted.saturating_sub(1)
                };
                return PromptInputAction::None;
            }
            KeyCode::Down if !self.choices.is_empty() => {
                self.input_focused = false;
                self.highlighted = (self.highlighted + 1) % self.choices.len();
                return PromptInputAction::None;
            }
            KeyCode::Tab => {
                if self.kind.allows_custom_input() && !self.choices.is_empty() {
                    self.input_focused = !self.input_focused;
                } else if !self.choices.is_empty() {
                    self.highlighted = (self.highlighted + 1) % self.choices.len();
                }
                return PromptInputAction::None;
            }
            KeyCode::Char(' ') if self.kind.is_multi() && !self.input_focused => {
                if let Some(choice) = self.choices.get_mut(self.highlighted) {
                    choice.selected = !choice.selected;
                }
                return PromptInputAction::None;
            }
            KeyCode::Char(c)
                if self.kind.allows_custom_input()
                    && !key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if !self.input_focused {
                    self.input_focused = true;
                    self.custom_input.clear();
                }
                self.custom_input.insert(c);
                return PromptInputAction::None;
            }
            KeyCode::Backspace
            | KeyCode::Delete
            | KeyCode::Left
            | KeyCode::Right
            | KeyCode::Home
            | KeyCode::End
                if self.kind.allows_custom_input() =>
            {
                self.input_focused = true;
                self.custom_input.handle_key(key);
                return PromptInputAction::None;
            }
            _ => {}
        }

        if self.input_focused || matches!(self.kind, PromptInputKind::Text { .. }) {
            self.custom_input.handle_key(key);
        }
        PromptInputAction::None
    }

    pub fn paste(&mut self, text: &str) {
        if text.is_empty() || !self.kind.allows_custom_input() {
            return;
        }
        self.input_focused = true;
        self.custom_input.paste(text);
    }

    pub fn submit_value(&self) -> String {
        match self.kind {
            PromptInputKind::Text { .. } => self.custom_input.value().to_string(),
            PromptInputKind::SingleSelect => self
                .choices
                .get(self.highlighted)
                .map(|choice| choice.value.clone())
                .unwrap_or_default(),
            PromptInputKind::SingleSelectWithInput { .. } => {
                if self.input_focused && !self.custom_input.is_empty() {
                    self.custom_input.value().to_string()
                } else {
                    self.choices
                        .get(self.highlighted)
                        .map(|choice| choice.value.clone())
                        .unwrap_or_else(|| self.custom_input.value().to_string())
                }
            }
            PromptInputKind::MultiSelect => self.selected_values().join(","),
            PromptInputKind::MultiSelectWithInput { .. } => {
                let mut values = self.selected_values();
                let custom = self.custom_input.value().trim();
                if !custom.is_empty() {
                    values.push(custom.to_string());
                }
                values.join(",")
            }
        }
    }

    pub fn selected_values(&self) -> Vec<String> {
        self.choices
            .iter()
            .filter(|choice| choice.selected)
            .map(|choice| choice.value.clone())
            .collect()
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
            Self::Decision(decision) => decision.options.len() as u16 + 9,
        };
        preferred.min(total_height.saturating_sub(7)).max(3)
    }
}

// ---------------------------------------------------------------------------
// Rendering
// ---------------------------------------------------------------------------

pub struct InteractionState<'a> {
    pub mode: InteractionMode,
    pub mode_title_override: Option<String>,
    pub active_tab: MainTab,
    pub input: &'a InputBuffer,
    pub prompt_input: Option<&'a PromptInputState>,
    pub has_agent_team: bool,
    pub command_hint: Option<String>,
    pub command_matched: bool,
    pub config_section: Option<crate::tui::workspace::config_task::ConfigSection>,
    pub init_nav: Option<InitNavSnapshot>,
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
    if let Some(prompt_input) = state.prompt_input {
        render_prompt_input(frame, area, state, prompt_input, focused, hovered);
        return;
    }

    let title = state
        .mode_title_override
        .clone()
        .unwrap_or_else(|| state.mode.title());
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Line::from(Span::styled(
            format!(" {title} "),
            mode_title_style(state),
        )))
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
    let input_style = text_input_style(state.input, state.command_matched);
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

fn text_input_style(input: &InputBuffer, command_matched: bool) -> Style {
    if input.is_empty() {
        theme::secondary()
    } else if command_matched {
        Style::default().fg(theme::accent()).bg(theme::background())
    } else if input.value().starts_with('/') {
        Style::default()
            .fg(theme::secondary_text())
            .bg(theme::background())
    } else {
        theme::text()
    }
}

fn mode_title_style(state: &InteractionState<'_>) -> Style {
    if state
        .mode_title_override
        .as_deref()
        .is_some_and(|title| title.contains("Bypass Permissions"))
        || state.mode == InteractionMode::Bypass
    {
        theme::base()
            .fg(theme::warning())
            .add_modifier(Modifier::BOLD)
    } else if state.mode == InteractionMode::Plan {
        theme::base().fg(theme::info()).add_modifier(Modifier::BOLD)
    } else {
        theme::text()
    }
}

fn prompt_title_line(scope_title: &str, prompt_title: &str) -> Line<'static> {
    if scope_title == t!("workspace.config_task.model_pool.title").as_ref() {
        return model_pool_title_line(scope_title, prompt_title);
    }

    let title = if prompt_title.trim().is_empty() {
        format!(" {scope_title} ")
    } else {
        format!(" {scope_title} - {prompt_title} ")
    };
    Line::from(title)
}

fn model_pool_title_line(scope_title: &str, prompt_title: &str) -> Line<'static> {
    let title_style = Style::default()
        .fg(theme::secondary_text())
        .add_modifier(Modifier::BOLD);
    let normal = theme::text();
    let highlight = Style::default()
        .bg(theme::selected_background())
        .fg(theme::selected_text());
    let mut spans: Vec<Span<'static>> = vec![Span::styled(format!(" {scope_title} "), title_style)];

    let parts = prompt_title
        .split(" - ")
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();
    for (index, part) in parts.iter().enumerate() {
        spans.push(Span::raw("·"));
        let style = if index + 1 == parts.len() {
            highlight
        } else {
            normal
        };
        spans.push(Span::styled(format!(" {part} "), style));
    }

    Line::from(spans)
}

/// Build the config-section nav bar as a styled title line:
/// `配置 · 模型 · 语言 · 灵魂` with the active section highlighted.
fn config_nav_line(active: ConfigSection) -> Line<'static> {
    let title_style = Style::default()
        .fg(theme::secondary_text())
        .add_modifier(Modifier::BOLD);
    let normal = theme::text();
    let highlight = Style::default()
        .bg(theme::selected_background())
        .fg(theme::selected_text());
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(
            format!(" {} ", t!("workspace.config_task.nav_title")),
            title_style,
        ),
        Span::raw("·"),
    ];
    for (idx, section) in ConfigSection::all().iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" · "));
        }
        let label = section.display_label();
        let style = if *section == active {
            highlight
        } else {
            normal
        };
        spans.push(Span::styled(format!(" {} ", label), style));
    }
    Line::from(spans)
}

/// Build the `/init` stage nav bar: `初始化 · ✓语言 · 模型池 · 分配 · 角色`.
/// Completed stages get a ✓ prefix; the active stage is highlighted.
fn init_nav_line(snapshot: &InitNavSnapshot) -> Line<'static> {
    let title_style = Style::default()
        .fg(theme::secondary_text())
        .add_modifier(Modifier::BOLD);
    let normal = theme::text();
    let highlight = Style::default()
        .bg(theme::selected_background())
        .fg(theme::selected_text());
    let done_style = Style::default().fg(theme::success());
    let stage_label = |stage: InitStage| match stage {
        InitStage::Language => t!("workspace.init_task.scope.select_language").to_string(),
        InitStage::ModelPool => t!("workspace.init_task.scope.configure_model_pool").to_string(),
        InitStage::Assignment => t!("workspace.init_task.scope.configure_assignment").to_string(),
        InitStage::Soul => t!("workspace.init_task.scope.configure_soul").to_string(),
    };
    let mut spans: Vec<Span<'static>> = vec![
        Span::styled(
            format!(" {} ", t!("workspace.init_task.nav_title")),
            title_style,
        ),
        Span::raw("·"),
    ];
    for (idx, stage) in InitStage::all().iter().enumerate() {
        if idx > 0 {
            spans.push(Span::raw(" · "));
        }
        let label = stage_label(*stage);
        let is_current = snapshot.current == Some(*stage);
        let is_done = snapshot.done.get(idx).copied().unwrap_or(false);
        let mut cell = String::new();
        if is_done {
            cell.push_str("✓ ");
        }
        cell.push_str(&label);
        let style = if is_current {
            highlight
        } else if is_done {
            done_style
        } else {
            normal
        };
        spans.push(Span::styled(format!(" {} ", cell), style));
    }
    Line::from(spans)
}

fn render_prompt_input(
    frame: &mut Frame,
    area: Rect,
    state: &InteractionState<'_>,
    prompt: &PromptInputState,
    focused: bool,
    hovered: bool,
) {
    let scope_title = prompt
        .scope_title
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| {
            state
                .mode_title_override
                .clone()
                .unwrap_or_else(|| state.mode.title())
        });
    let block = Block::default()
        .borders(Borders::ALL)
        .style(theme::base())
        .border_style(theme::border_state(focused, hovered));
    let block = if let Some(active) = state.config_section {
        block.title(config_nav_line(active))
    } else if let Some(nav) = &state.init_nav {
        block.title(init_nav_line(nav))
    } else {
        block.title(prompt_title_line(&scope_title, &prompt.title))
    };
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(inner);

    let mut lines = Vec::new();
    for (index, choice) in prompt.choices.iter().enumerate().take(5) {
        let highlighted = index == prompt.highlighted && !prompt.input_focused;
        let symbol = if prompt.kind.is_multi() {
            if choice.selected {
                "[x]"
            } else {
                "[ ]"
            }
        } else if highlighted {
            "●"
        } else {
            "○"
        };
        let style = if highlighted {
            theme::selected()
        } else if choice.selected {
            theme::emphasis()
        } else {
            theme::text()
        };
        lines.push(Line::from(vec![
            Span::styled(symbol, style),
            Span::raw(" "),
            Span::styled(choice.label.as_str(), style),
        ]));
    }

    if prompt.kind.allows_custom_input() {
        if !lines.is_empty() {
            lines.push(Line::default());
        }
        let placeholder = if prompt.placeholder.is_empty() {
            t!("workspace.config_task.custom_input").to_string()
        } else {
            prompt.placeholder.clone()
        };
        let display = if prompt.kind.is_masked() {
            prompt.custom_input.display_masked_with_cursor(&placeholder)
        } else {
            prompt.custom_input.display_with_cursor(&placeholder)
        };
        let symbol = if prompt.input_focused { "›" } else { " " };
        let style = if prompt.input_focused {
            theme::text()
        } else {
            theme::secondary()
        };
        lines.push(Line::from(vec![
            Span::styled(symbol, style),
            Span::raw(" "),
            Span::styled(display, style),
        ]));
    }

    if lines.is_empty() {
        let placeholder = if prompt.placeholder.is_empty() {
            state.mode.placeholder(state.active_tab)
        } else {
            prompt.placeholder.clone()
        };
        let display = if prompt.kind.is_masked() {
            prompt.custom_input.display_masked_with_cursor(&placeholder)
        } else {
            prompt.custom_input.display_with_cursor(&placeholder)
        };
        lines.push(Line::from(Span::styled(display, theme::text())));
    }

    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        chunks[0],
    );
    frame.render_widget(
        Paragraph::new(prompt.help.clone())
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
    let title = decision_panel_title(decision, state);
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {title} "))
        .style(theme::base())
        .border_style(if hovered {
            Style::default().fg(theme::accent()).bg(theme::background())
        } else {
            Style::default().fg(theme::warning()).bg(theme::background())
        });
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();

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
    }

    if decision.kind != DecisionKind::QuitConfirm {
        lines.push(Line::default());
        lines.push(Line::from(Span::styled(
            t!("workspace.decision.reply").to_string(),
            if decision.custom_focused {
                theme::emphasis()
            } else {
                theme::secondary()
            },
        )));
        let custom = decision
            .custom_input
            .display_with_cursor(if decision.custom_focused { "" } else { ">" });
        lines.push(Line::from(vec![
            Span::styled("  → ", theme::secondary()),
            Span::styled(custom, theme::text()),
        ]));
        lines.push(Line::default());
    }
    lines.push(Line::from(Span::styled(
        t!("workspace.decision.help").to_string(),
        theme::secondary(),
    )));

    frame.render_widget(
        Paragraph::new(Text::from(lines)).wrap(Wrap { trim: false }),
        inner,
    );
}

fn decision_panel_title(decision: &DecisionState, state: &InteractionState<'_>) -> String {
    if decision.kind == DecisionKind::QuitConfirm {
        return decision.title.clone();
    }

    let scope_title = decision
        .scope_title
        .as_deref()
        .map(str::to_string)
        .unwrap_or_else(|| {
            state
                .mode_title_override
                .clone()
                .unwrap_or_else(|| state.mode.title())
        });
    format!("{} - {}", scope_title, decision.title)
}

// ---------------------------------------------------------------------------
// InteractionMode helpers
// ---------------------------------------------------------------------------

impl InteractionMode {
    pub fn title(self) -> String {
        match self {
            Self::Chat => t!("workspace.mode.chat").to_string(),
            Self::Plan => t!("workspace.mode.plan").to_string(),
            Self::Bypass => t!("workspace.mode.bypass").to_string(),
        }
    }

    pub fn placeholder(self, active_tab: MainTab) -> String {
        match (active_tab, self) {
            (MainTab::AgentTeam, Self::Chat) => t!("workspace.placeholder.team_chat").to_string(),
            (MainTab::AgentTeam, Self::Plan) => t!("workspace.placeholder.team_plan").to_string(),
            (MainTab::AgentTeam, Self::Bypass) => {
                t!("workspace.placeholder.team_bypass").to_string()
            }
            (_, Self::Chat) => t!("workspace.placeholder.chat").to_string(),
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
    fn incomplete_slash_command_input_uses_secondary_color() {
        let mut input = InputBuffer::default();
        input.paste("/he");

        let style = text_input_style(&input, false);

        assert_eq!(style.fg, Some(theme::secondary_text()));
        assert_eq!(style.bg, Some(theme::background()));
    }

    #[test]
    fn complete_slash_command_input_uses_accent_color() {
        let mut input = InputBuffer::default();
        input.paste("/help");

        let style = text_input_style(&input, true);

        assert_eq!(style.fg, Some(theme::accent()));
        assert_eq!(style.bg, Some(theme::background()));
    }

    #[test]
    fn model_pool_prompt_title_uses_breadcrumb_and_highlights_current_step() {
        rust_i18n::set_locale("zh-CN");

        let line = prompt_title_line("模型池管理", "添加模型 - 选择服务商");
        let rendered = line
            .spans
            .iter()
            .map(|span| span.content.as_ref())
            .collect::<String>();

        assert_eq!(rendered, " 模型池管理 · 添加模型 · 选择服务商 ");
        let current = line.spans.last().expect("current step span");
        assert_eq!(current.style.fg, Some(theme::selected_text()));
        assert_eq!(current.style.bg, Some(theme::selected_background()));
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

    #[test]
    fn input_buffer_paste_inserts_at_cursor() {
        let mut input = InputBuffer::default();
        input.set_value("sk-end");
        input.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));
        input.handle_key(KeyEvent::new(KeyCode::Left, KeyModifiers::NONE));

        input.paste("中文-");

        assert_eq!(input.value(), "sk-中文-end");
        assert_eq!(input.cursor(), 6);
    }

    #[test]
    fn decision_options_do_not_include_custom_reply_choice() {
        let decision = DecisionState::plan_approval();

        assert_eq!(decision.options.len(), 3);
        assert!(!decision
            .options
            .iter()
            .any(|option| option == t!("workspace.decision.custom_response").as_ref()));
    }

    #[test]
    fn decision_tab_focuses_bottom_reply_input() {
        let mut decision = DecisionState::plan_approval();
        assert_eq!(decision.selected, 0);

        assert!(matches!(
            decision.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE)),
            WorkspaceAction::None
        ));

        assert!(decision.custom_focused);
        assert_eq!(decision.selected, 0);
    }

    #[test]
    fn decision_typing_uses_bottom_reply_input() {
        let mut decision = DecisionState::plan_approval();
        decision.selected = 0;

        decision.handle_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        decision.handle_key(KeyEvent::new(KeyCode::Char('o'), KeyModifiers::NONE));

        assert!(decision.custom_focused);
        assert_eq!(decision.custom_input.value(), "no");
        match decision.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)) {
            WorkspaceAction::RevisePlan(text) => assert_eq!(text, "no"),
            action => panic!("expected custom plan revision, got {action:?}"),
        }
    }

    #[test]
    fn quit_confirmation_uses_plain_title_and_two_actions() {
        rust_i18n::set_locale("zh-CN");
        let input = InputBuffer::default();
        let decision = DecisionState::quit_confirm();
        let state = InteractionState {
            mode: InteractionMode::Plan,
            mode_title_override: None,
            active_tab: MainTab::Conversation,
            input: &input,
            prompt_input: None,
            has_agent_team: false,
            command_hint: None,
            command_matched: false,
            config_section: None,
            init_nav: None,
        };

        assert_eq!(decision.title, "确定退出 Alius？");
        assert_eq!(
            decision.options,
            vec!["取消".to_string(), "确定".to_string()]
        );
        assert_eq!(decision.selected, 0);
        assert_eq!(decision_panel_title(&decision, &state), "确定退出 Alius？");
    }

    #[test]
    fn quit_confirmation_confirm_requires_second_option() {
        let mut decision = DecisionState::quit_confirm();

        assert!(matches!(
            decision.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            WorkspaceAction::CancelDecision
        ));

        decision.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        assert!(matches!(
            decision.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            WorkspaceAction::Quit
        ));
    }

    #[test]
    fn decision_enter_without_reply_confirms_selected_choice() {
        let mut decision = DecisionState::plan_approval();

        assert!(matches!(
            decision.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            WorkspaceAction::ApprovePlan
        ));
    }

    #[test]
    fn prompt_single_select_moves_and_submits_highlighted_choice() {
        let mut input = PromptInputState::new(
            "Provider",
            PromptInputKind::SingleSelect,
            vec![
                PromptChoice::new("OpenAI", "openai"),
                PromptChoice::new("Anthropic", "anthropic"),
            ],
            "help",
        );

        assert!(matches!(
            input.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE)),
            PromptInputAction::None
        ));
        assert_eq!(input.highlighted, 1);
        assert_eq!(
            input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptInputAction::Submit("anthropic".to_string())
        );
    }

    #[test]
    fn prompt_single_select_with_input_accepts_custom_text() {
        let mut input = PromptInputState::new(
            "Model",
            PromptInputKind::SingleSelectWithInput { masked: false },
            vec![PromptChoice::new("Default", "gpt-4o-mini")],
            "help",
        );

        for ch in "custom-model".chars() {
            input.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert!(input.input_focused);
        assert_eq!(
            input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptInputAction::Submit("custom-model".to_string())
        );
    }

    #[test]
    fn prompt_single_select_with_input_accepts_paste() {
        let mut input = PromptInputState::new(
            "API key",
            PromptInputKind::SingleSelectWithInput { masked: false },
            vec![PromptChoice::new("Saved", "sk-old")],
            "help",
        );

        input.paste("sk-pasted");

        assert!(input.input_focused);
        assert_eq!(
            input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptInputAction::Submit("sk-pasted".to_string())
        );
    }

    #[test]
    fn prompt_select_with_input_reserves_custom_input_line() {
        let input = PromptInputState::new(
            "Base URL",
            PromptInputKind::SingleSelectWithInput { masked: false },
            vec![
                PromptChoice::new("Default", "https://api.example.com"),
                PromptChoice::new("Custom", "https://custom.example.com"),
            ],
            "help",
        );

        assert_eq!(input.preferred_height(24), 7);
    }

    #[test]
    fn prompt_multi_select_toggles_choices_with_space() {
        let mut input = PromptInputState::new(
            "Tools",
            PromptInputKind::MultiSelect,
            vec![
                PromptChoice::new("Filesystem", "filesystem"),
                PromptChoice::new("Shell", "shell"),
            ],
            "help",
        );

        input.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        input.handle_key(KeyEvent::new(KeyCode::Down, KeyModifiers::NONE));
        input.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));

        assert_eq!(
            input.selected_values(),
            vec!["filesystem".to_string(), "shell".to_string()]
        );
        assert_eq!(
            input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptInputAction::Submit("filesystem,shell".to_string())
        );
    }

    #[test]
    fn prompt_multi_select_with_input_appends_custom_value() {
        let mut input = PromptInputState::new(
            "Permissions",
            PromptInputKind::MultiSelectWithInput { masked: false },
            vec![PromptChoice::new("Filesystem", "filesystem")],
            "help",
        );

        input.handle_key(KeyEvent::new(KeyCode::Char(' '), KeyModifiers::NONE));
        input.handle_key(KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        for ch in "network".chars() {
            input.handle_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::NONE));
        }

        assert_eq!(
            input.handle_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)),
            PromptInputAction::Submit("filesystem,network".to_string())
        );
    }
}
