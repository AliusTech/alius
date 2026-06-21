use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use super::helpers::{char_len, count_visual_lines, truncate_chars};
use super::{PanelScroll, PlainTextLine};
use crate::tui::state::{ConversationBlock, ConversationBlockType, MainTab};
use crate::tui::theme;

const ALIUS_LOGO_FULL: &[&str] = &[
    "█████╗ ██╗     ██╗██╗   ██╗███████╗",
    "██╔══██╗██║     ██║██║   ██║██╔════╝",
    "███████║██║     ██║██║   ██║███████╗",
    "██╔══██║██║     ██║██║   ██║╚════██║",
    "██║  ██║███████╗██║╚██████╔╝███████║",
    "╚═╝  ╚═╝╚══════╝╚═╝ ╚═════╝ ╚══════╝",
];

const ALIUS_LOGO_MEDIUM: &[&str] = &[
    " █████  ██      ██ ██   ██ █████ ",
    "██   ██ ██      ██ ██   ██ ██    ",
    "███████ ██      ██ ██   ██ █████ ",
    "██   ██ ██      ██ ██   ██    ██ ",
    "██   ██ ███████ ██  █████  █████ ",
];

const ALIUS_LOGO_SMALL: &[&str] = &["▝▜████▛▘", "  ALIUS  ", "▗▟████▙▖"];

const ALIUS_LOGO_TINY: &[&str] = &["ALIUS"];
const SLOGAN: &str = "慎始如终";
const WELCOME_FRAME_MARGIN: usize = 2;
const MAX_COLLAPSED_LINES: usize = 3;

#[cfg(target_os = "macos")]
fn fold_shortcut_label() -> &'static str {
    "Control+O"
}

#[cfg(not(target_os = "macos"))]
fn fold_shortcut_label() -> &'static str {
    "Ctrl+O"
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WelcomeLayout {
    Wide,
    Medium,
    Compact,
    Tiny,
}

pub fn resolve_welcome_layout(width: u16, height: u16) -> WelcomeLayout {
    if width >= 100 && height >= 14 {
        WelcomeLayout::Wide
    } else if width >= 72 && height >= 12 {
        WelcomeLayout::Medium
    } else if width >= 46 && height >= 12 {
        WelcomeLayout::Compact
    } else {
        WelcomeLayout::Tiny
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WelcomeState {
    pub version: String,
    pub ready: bool,
    pub soul: Option<String>,
    pub model_plan: Option<String>,
    pub model_execute: Option<String>,
    pub model_review: Option<String>,
}

impl WelcomeState {
    fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn decode(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOverviewRow {
    pub label: String,
    pub done: bool,
    pub current: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigOverviewState {
    pub title: String,
    pub rows: Vec<ConfigOverviewRow>,
}

impl ConfigOverviewState {
    fn encode(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }

    fn decode(value: &str) -> Option<Self> {
        serde_json::from_str(value).ok()
    }
}

fn render_config_overview_lines(state: &ConfigOverviewState) -> Vec<Line<'static>> {
    let header_style = Style::default()
        .fg(theme::accent())
        .bg(theme::background())
        .add_modifier(Modifier::BOLD);
    let mut lines = vec![Line::from(vec![
        Span::styled("◆", header_style),
        Span::raw(" "),
        Span::styled(state.title.clone(), header_style),
    ])];
    for row in &state.rows {
        let icon = if row.done { "✓" } else { "○" };
        let icon_style = if row.done {
            Style::default().fg(theme::success())
        } else {
            Style::default().fg(theme::secondary_text())
        };
        lines.push(Line::from(vec![
            Span::raw("  "),
            Span::styled(icon, icon_style),
            Span::raw(" "),
            Span::styled(row.label.clone(), theme::text()),
            Span::raw("  "),
            Span::styled(row.current.clone(), theme::secondary()),
        ]));
    }
    lines
}

/// Extract plain text from a ratatui `Line`.
fn line_to_plain(line: &Line<'_>) -> String {
    line.spans
        .iter()
        .map(|s| s.content.as_ref())
        .collect::<Vec<_>>()
        .join("")
}

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    blocks: &[ConversationBlock],
    model: &str,
    tab_title: &str,
    scroll: &mut PanelScroll,
    focused: bool,
    hovered: bool,
    expanded_blocks: &std::collections::HashSet<String>,
    global_expanded: bool,
    plain_lines: &mut Vec<PlainTextLine>,
) -> HashMap<String, (u16, u16)> {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(tab_title)
        .style(theme::base())
        .border_style(theme::border_state(focused, hovered));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let only_welcome = blocks.len() == 1 && blocks[0].block_type == ConversationBlockType::Welcome;
    let mut lines = if only_welcome {
        Vec::new()
    } else {
        vec![
            Line::from(vec![
                Span::styled(
                    t!("workspace.conversation.model").to_string(),
                    theme::secondary(),
                ),
                Span::styled(model.to_string(), theme::text()),
            ]),
            Line::default(),
        ]
    };

    let mut block_row_map = HashMap::new();
    let _start_row = lines.len() as u16;

    for block in blocks {
        let block_start_row = lines.len() as u16;

        if block.block_type == ConversationBlockType::Welcome {
            if let Some(state) = WelcomeState::decode(&block.content) {
                let available_height = inner.height.saturating_sub(lines.len() as u16);
                lines.extend(render_welcome_lines(&state, inner.width, available_height));
            } else {
                for line in block.content.lines() {
                    lines.push(Line::from(line.to_string()));
                }
            }
            lines.push(Line::default());
            continue;
        }

        if block.block_type == ConversationBlockType::ConfigOverview {
            if let Some(state) = ConfigOverviewState::decode(&block.content) {
                lines.extend(render_config_overview_lines(&state));
            } else {
                for line in block.content.lines() {
                    lines.push(Line::from(line.to_string()));
                }
            }
            lines.push(Line::default());
            continue;
        }

        let is_expanded = global_expanded || expanded_blocks.contains(&block.id);
        let header_style = Style::default()
            .fg(block.block_type.color())
            .bg(theme::background())
            .add_modifier(Modifier::BOLD);
        // Tool blocks show a per-tool icon instead of the generic block-type
        // symbol, so a shell call shows ▸, a file read shows ▤, etc.
        let icon = block
            .tool_name
            .as_deref()
            .map(tool_icon)
            .unwrap_or_else(|| block.block_type.symbol());

        if block.block_type == ConversationBlockType::Request {
            let mut content_lines = block.content.lines();
            if let Some(first_line) = content_lines.next() {
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(icon, header_style),
                    Span::raw("  "),
                    Span::raw(first_line.to_string()),
                ]));
                for line in content_lines {
                    lines.push(Line::from(vec![
                        Span::raw("    "),
                        Span::raw(line.to_string()),
                    ]));
                }
            } else {
                lines.push(Line::from(vec![
                    Span::raw(" "),
                    Span::styled(icon, header_style),
                ]));
            }
            lines.push(Line::default());

            let block_end_row = lines.len() as u16;
            block_row_map.insert(block.id.clone(), (block_start_row, block_end_row));
            continue;
        }

        let title = render_title(block);

        // Check if block should be folded
        let is_empty_execution =
            block.block_type == ConversationBlockType::Execution && block.content.trim().is_empty();

        if is_empty_execution {
            // Empty execution blocks are not folded
            lines.push(header_line(icon, title.as_deref(), header_style));
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled("..", theme::loading_dot()),
                Span::styled(format!(" {}", t!("workspace.loading")), theme::secondary()),
            ]));
            lines.push(Line::default());

            let block_end_row = lines.len() as u16;
            block_row_map.insert(block.id.clone(), (block_start_row, block_end_row));
            continue;
        }

        // Render block with folding logic
        let content_lines: Vec<&str> = block.content.lines().collect();
        let total_lines = 1 + content_lines.len(); // title + content lines

        if !is_expanded && total_lines > MAX_COLLAPSED_LINES {
            // Folded: show title with first line merged, then up to 2 more lines with hint
            let mut first_content = content_lines
                .first()
                .map(|s| s.to_string())
                .unwrap_or_default();
            if first_content.is_empty() && content_lines.len() > 1 {
                first_content = content_lines[1].to_string();
            }

            let mut header = header_spans(icon, title.as_deref(), header_style);
            if !first_content.is_empty() {
                header.push(Span::raw("  "));
                header.push(Span::raw(first_content));
            }
            lines.push(Line::from(header));

            let remaining_to_show =
                (MAX_COLLAPSED_LINES - 1).min(content_lines.len().saturating_sub(1));
            for i in 1..=remaining_to_show {
                if i < content_lines.len() {
                    let line_text = content_lines[i];
                    if i == remaining_to_show && i < content_lines.len() - 1 {
                        // Last visible line with fold hint
                        let hint = format!(" … {} 全部展开", fold_shortcut_label());
                        let available_width = inner
                            .width
                            .saturating_sub(hint.len() as u16)
                            .saturating_sub(4)
                            as usize;
                        let truncated = truncate_chars(line_text, available_width);
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::raw(truncated),
                            Span::styled(hint, theme::secondary()),
                        ]));
                    } else {
                        lines.push(Line::from(vec![
                            Span::raw("    "),
                            Span::raw(line_text.to_string()),
                        ]));
                    }
                }
            }
        } else {
            // Expanded or short block: show everything
            lines.push(header_line(icon, title.as_deref(), header_style));
            for line in content_lines {
                lines.push(Line::from(vec![
                    Span::raw("    "),
                    Span::raw(line.to_string()),
                ]));
            }
        }

        lines.push(Line::default());

        let block_end_row = lines.len() as u16;
        block_row_map.insert(block.id.clone(), (block_start_row, block_end_row));
    }

    let total_visual = count_visual_lines(&lines, inner.width);
    let max_off = total_visual.saturating_sub(inner.height as usize) as u16;
    scroll.snap_to_bottom(max_off);
    scroll.clamp(max_off);

    // Build plain text lines for selection/copy.
    plain_lines.clear();
    for (i, line) in lines.iter().enumerate() {
        plain_lines.push(PlainTextLine {
            text: line_to_plain(line),
            row: i as u16,
        });
    }

    let paragraph = Paragraph::new(Text::from(lines))
        .style(theme::base())
        .wrap(Wrap { trim: false })
        .scroll((scroll.offset, 0));
    // Clear first: ratatui's set_style only sets colors, it does NOT erase
    // previous glyphs. Without this, rows below the content (after scroll or
    // when content shrinks via folding) retain stale characters from the prior
    // frame.
    frame.render_widget(ratatui::widgets::Clear, inner);
    frame.render_widget(paragraph, inner);

    block_row_map
}

fn render_title(block: &ConversationBlock) -> Option<String> {
    match block.title.as_deref() {
        Some(title) if is_default_prompt_title(block.block_type, title) => None,
        Some(title) => Some(title.to_string()),
        None if suppresses_default_title(block.block_type) => None,
        None => Some(block.block_type.title()),
    }
}

fn suppresses_default_title(block_type: ConversationBlockType) -> bool {
    matches!(
        block_type,
        ConversationBlockType::Decision
            | ConversationBlockType::Result
            | ConversationBlockType::Error
    )
}

fn is_default_prompt_title(block_type: ConversationBlockType, title: &str) -> bool {
    if !matches!(
        block_type,
        ConversationBlockType::Decision
            | ConversationBlockType::Result
            | ConversationBlockType::Error
    ) {
        return false;
    }
    title == t!("workspace.block.decision").as_ref()
        || title == t!("workspace.block.status").as_ref()
        || title == t!("workspace.block.result").as_ref()
        || title == t!("workspace.block.error").as_ref()
}

fn header_line(icon: &str, title: Option<&str>, style: Style) -> Line<'static> {
    Line::from(header_spans(icon, title, style))
}

fn header_spans(icon: &str, title: Option<&str>, style: Style) -> Vec<Span<'static>> {
    // Pseudo-column layout: a leading space + glyph + two spaces, so all block
    // glyphs line up vertically as a left "column". Continuation lines use the
    // matching 4-space indent (see render()).
    let mut spans = vec![Span::raw(" "), Span::styled(icon.to_string(), style)];
    if let Some(title) = title.filter(|title| !title.trim().is_empty()) {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(title.to_string(), style));
    }
    spans
}

pub fn tab_title(active_tab: MainTab, has_agent_team: bool) -> String {
    let conv = t!("workspace.tab.conversation").to_string();
    let team = t!("workspace.tab.agent_team").to_string();
    let conversation_style = if active_tab == MainTab::Conversation {
        format!("[ {} ]", conv)
    } else {
        format!("  {}  ", conv)
    };
    let team_style = if active_tab == MainTab::AgentTeam {
        format!("[ {} ]", team)
    } else {
        format!("  {}  ", team)
    };

    if has_agent_team {
        format!(" {} {} ", conversation_style, team_style)
    } else {
        format!(" {} ", conv)
    }
}

impl ConversationBlock {
    pub fn welcome_state(state: WelcomeState) -> Self {
        Self::new(ConversationBlockType::Welcome, state.encode())
    }

    pub fn request(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Request, content)
    }

    pub fn understanding(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Understanding, content)
    }

    pub fn plan_proposal(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::PlanProposal, content)
    }

    pub fn execution(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Execution, content)
    }

    pub fn streaming(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Streaming, content)
    }

    pub fn is_streaming(&self) -> bool {
        self.block_type == ConversationBlockType::Streaming
    }

    pub fn is_execution(&self) -> bool {
        self.block_type == ConversationBlockType::Execution
    }

    /// Append text to the content of this block.
    pub fn append_content(&mut self, delta: &str) {
        self.content.push_str(delta);
    }

    /// Replace this block's type and content (e.g. Streaming → Result).
    pub fn convert_to(&mut self, block_type: ConversationBlockType, content: String) {
        self.block_type = block_type;
        self.content = content;
    }

    pub fn decision(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Decision, content)
    }

    pub fn status(content: impl Into<String>) -> Self {
        Self {
            id: Self::generate_id(),
            block_type: ConversationBlockType::Decision,
            title: Some(t!("workspace.block.status").to_string()),
            content: content.into(),
            tool_name: None,
        }
    }

    pub fn config_overview(state: ConfigOverviewState) -> Self {
        Self {
            id: Self::generate_id(),
            block_type: ConversationBlockType::ConfigOverview,
            title: None,
            content: state.encode(),
            tool_name: None,
        }
    }

    pub fn result(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Result, content)
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Error, content)
    }

    fn new(block_type: ConversationBlockType, content: impl Into<String>) -> Self {
        Self {
            id: Self::generate_id(),
            block_type,
            title: None,
            content: content.into(),
            tool_name: None,
        }
    }

    fn generate_id() -> String {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        format!("block-{}", COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

fn render_welcome_lines(state: &WelcomeState, width: u16, height: u16) -> Vec<Line<'static>> {
    let layout = resolve_welcome_layout(width, height);
    let rows = match layout {
        WelcomeLayout::Wide => render_wide_welcome(state, width),
        WelcomeLayout::Medium => render_medium_welcome(state, width),
        WelcomeLayout::Compact => render_compact_welcome(state, width),
        WelcomeLayout::Tiny => render_tiny_welcome(state, width),
    };
    rows.into_iter().map(Line::from).collect()
}

fn render_wide_welcome(state: &WelcomeState, width: u16) -> Vec<String> {
    let target = welcome_frame_width(width);
    let left_width = 46;
    let right_width = target.saturating_sub(left_width + 7);
    let values = welcome_values(state, false);
    let left_rows = vec![
        String::new(),
        ALIUS_LOGO_FULL[0].to_string(),
        ALIUS_LOGO_FULL[1].to_string(),
        ALIUS_LOGO_FULL[2].to_string(),
        ALIUS_LOGO_FULL[3].to_string(),
        ALIUS_LOGO_FULL[4].to_string(),
        ALIUS_LOGO_FULL[5].to_string(),
        String::new(),
        format!("Version  {}", state.version),
        SLOGAN.to_string(),
        String::new(),
    ];
    let right_rows = vec![
        String::new(),
        String::new(),
        welcome_field("SOUL", &values.soul, 9, right_width),
        String::new(),
        welcome_field("Plan", &values.plan, 9, right_width),
        welcome_field("Execute", &values.execute, 9, right_width),
        welcome_field("Review", &values.review, 9, right_width),
        String::new(),
        values.enter,
        String::new(),
        String::new(),
    ];
    frame_split_rows(
        &left_rows,
        &right_rows,
        left_width,
        right_width,
        target,
        width,
    )
}

fn render_medium_welcome(state: &WelcomeState, width: u16) -> Vec<String> {
    let target = welcome_frame_width(width);
    let left_width = 38.min(target.saturating_sub(16));
    let right_width = target.saturating_sub(left_width + 7);
    let values = welcome_values(state, true);
    let left_rows = vec![
        String::new(),
        ALIUS_LOGO_MEDIUM[0].to_string(),
        ALIUS_LOGO_MEDIUM[1].to_string(),
        ALIUS_LOGO_MEDIUM[2].to_string(),
        ALIUS_LOGO_MEDIUM[3].to_string(),
        ALIUS_LOGO_MEDIUM[4].to_string(),
        String::new(),
        format!("Version  {}", state.version),
        SLOGAN.to_string(),
        String::new(),
    ];
    let right_rows = vec![
        String::new(),
        String::new(),
        welcome_field("SOUL", &values.soul, 7, right_width),
        String::new(),
        welcome_field("Plan", &values.plan, 7, right_width),
        welcome_field("Execute", &values.execute, 7, right_width),
        welcome_field("Review", &values.review, 7, right_width),
        String::new(),
        welcome_field("Enter", values.enter_short, 7, right_width),
        String::new(),
    ];
    frame_split_rows(
        &left_rows,
        &right_rows,
        left_width,
        right_width,
        target,
        width,
    )
}

fn render_compact_welcome(state: &WelcomeState, width: u16) -> Vec<String> {
    let target = welcome_frame_width(width);
    let inner = target.saturating_sub(4);
    let values = welcome_values(state, true);
    let rows = vec![
        String::new(),
        centered(ALIUS_LOGO_SMALL[0], inner),
        centered(ALIUS_LOGO_SMALL[1], inner),
        centered(ALIUS_LOGO_SMALL[2], inner),
        String::new(),
        centered(&format!("Version {}", state.version), inner),
        centered(SLOGAN, inner),
        String::new(),
        centered(&welcome_field("SOUL", &values.soul, 7, inner), inner),
        String::new(),
        centered(&welcome_field("Plan", &values.plan, 7, inner), inner),
        centered(&welcome_field("Execute", &values.execute, 7, inner), inner),
        centered(&welcome_field("Review", &values.review, 7, inner), inner),
        String::new(),
        centered(&values.enter, inner),
        String::new(),
    ];
    frame_single_rows(&rows, inner, target, width)
}

fn render_tiny_welcome(state: &WelcomeState, width: u16) -> Vec<String> {
    let values = welcome_values(state, true);
    let max = width as usize;
    [
        ALIUS_LOGO_TINY[0].to_string(),
        format!("Version {}", state.version),
        SLOGAN.to_string(),
        String::new(),
        welcome_field("SOUL", &values.soul, 7, max),
        welcome_field("Plan", &values.plan, 7, max),
        welcome_field("Execute", &values.execute, 7, max),
        welcome_field("Review", &values.review, 7, max),
        String::new(),
        values.enter,
    ]
    .into_iter()
    .map(|line| truncate_chars(&line, max))
    .collect()
}

#[derive(Debug, Clone)]
struct WelcomeValues {
    soul: String,
    plan: String,
    execute: String,
    review: String,
    enter: String,
    enter_short: &'static str,
}

fn welcome_values(state: &WelcomeState, compact_models: bool) -> WelcomeValues {
    let not_selected = "Not selected".to_string();
    let not_configured = "Not configured".to_string();
    let model = |value: &Option<String>| {
        value
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(|value| {
                if compact_models {
                    compact_model_name(value)
                } else {
                    value.to_string()
                }
            })
            .unwrap_or_else(|| not_configured.clone())
    };
    WelcomeValues {
        soul: state
            .soul
            .clone()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(not_selected),
        plan: model(&state.model_plan),
        execute: model(&state.model_execute),
        review: model(&state.model_review),
        enter: if state.ready {
            "Press Enter to start".to_string()
        } else {
            "Press Enter to continue".to_string()
        },
        enter_short: if state.ready { "Start" } else { "Continue" },
    }
}

pub fn compact_model_name(model: &str) -> String {
    if let Some(start) = model.find('(') {
        if let Some(end) = model.rfind(')') {
            if end > start {
                return model[start + 1..end].to_string();
            }
        }
    }
    model.to_string()
}

fn welcome_field(label: &str, value: &str, label_width: usize, max_width: usize) -> String {
    let prefix = format!("{label:<label_width$}  ");
    let value_width = max_width.saturating_sub(char_len(&prefix));
    format!("{prefix}{}", truncate_chars(value, value_width))
}

fn welcome_frame_width(width: u16) -> usize {
    (width as usize).saturating_sub(WELCOME_FRAME_MARGIN * 2)
}

fn frame_split_rows(
    left_rows: &[String],
    right_rows: &[String],
    left_width: usize,
    right_width: usize,
    target: usize,
    available_width: u16,
) -> Vec<String> {
    let height = left_rows.len().max(right_rows.len());
    let mut rows = Vec::with_capacity(height + 2);
    rows.push(centered(
        &format!("╭{}╮", "─".repeat(target - 2)),
        available_width as usize,
    ));
    for index in 0..height {
        let left = left_rows.get(index).map(String::as_str).unwrap_or("");
        let right = right_rows.get(index).map(String::as_str).unwrap_or("");
        rows.push(centered(
            &format!(
                "│ {} │ {} │",
                pad_visual(left, left_width),
                pad_visual(right, right_width)
            ),
            available_width as usize,
        ));
    }
    rows.push(centered(
        &format!("╰{}╯", "─".repeat(target - 2)),
        available_width as usize,
    ));
    rows
}

fn frame_single_rows(
    content_rows: &[String],
    inner_width: usize,
    target: usize,
    available_width: u16,
) -> Vec<String> {
    let mut rows = Vec::with_capacity(content_rows.len() + 2);
    rows.push(centered(
        &format!("╭{}╮", "─".repeat(target - 2)),
        available_width as usize,
    ));
    rows.extend(content_rows.iter().map(|row| {
        centered(
            &format!("│ {} │", pad_visual(row, inner_width)),
            available_width as usize,
        )
    }));
    rows.push(centered(
        &format!("╰{}╯", "─".repeat(target - 2)),
        available_width as usize,
    ));
    rows
}

fn centered(value: &str, width: usize) -> String {
    let value = truncate_chars(value, width);
    let visual = char_len(&value);
    if visual >= width {
        return value;
    }
    let left = (width - visual) / 2;
    format!("{}{}", " ".repeat(left), value)
}

fn pad_visual(value: &str, width: usize) -> String {
    let value = truncate_chars(value, width);
    let visual = char_len(&value);
    format!("{}{}", value, " ".repeat(width.saturating_sub(visual)))
}

impl ConversationBlockType {
    pub fn symbol(self) -> &'static str {
        // Single-cell-width Unicode glyphs (terminal-safe, no emoji). They
        // form a left "column" via the header/continuation layout in render().
        match self {
            Self::Welcome => "◆",
            Self::Request => "▶",
            Self::Understanding => "◈",
            Self::PlanProposal => "☰",
            Self::Execution => "●",
            Self::Streaming => "◐",
            Self::Decision => "◆",
            Self::Result => "✓",
            Self::Error => "✗",
            Self::ConfigOverview => "▤",
        }
    }

    pub fn title(self) -> String {
        match self {
            Self::Welcome => String::new(),
            Self::Request => t!("workspace.block.request").to_string(),
            Self::Understanding => t!("workspace.block.understanding").to_string(),
            Self::PlanProposal => t!("workspace.block.plan_proposal").to_string(),
            Self::Execution => t!("workspace.block.execution").to_string(),
            Self::Streaming => "...".to_string(),
            Self::Decision => t!("workspace.block.decision").to_string(),
            Self::Result => t!("workspace.block.result").to_string(),
            Self::Error => t!("workspace.block.error").to_string(),
            Self::ConfigOverview => t!("workspace.block.config_overview").to_string(),
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Welcome => theme::accent(),
            Self::Request => theme::accent(),
            Self::Understanding => theme::info(),
            Self::PlanProposal => theme::review(),
            Self::Execution => theme::warning(),
            Self::Streaming => theme::info(),
            Self::Decision => theme::warning(),
            Self::Result => theme::success(),
            Self::Error => theme::error(),
            Self::ConfigOverview => theme::accent(),
        }
    }
}

/// Icon for a tool call, keyed by the tool's registered name. Returns a
/// terminal-safe single-cell Unicode glyph; unknown/MCP tools get a generic
/// glyph. Used in place of the block-type symbol when a block carries a
/// `tool_name`, so each tool family has a distinct icon.
pub fn tool_icon(tool_name: &str) -> &'static str {
    match tool_name {
        "shell" => "▸",
        "read_file" => "▤",
        "write_file" | "edit_file" => "✎",
        "list_dir" => "▣",
        "search_code" => "⚲",
        "run_local_service" | "local_service_status" => "◉",
        _ => "◈",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::collections::HashSet;

    fn render_blocks_for_test(blocks: Vec<ConversationBlock>) -> String {
        render_blocks_for_test_with_global_expand(blocks, true)
    }

    fn render_blocks_for_test_with_global_expand(
        blocks: Vec<ConversationBlock>,
        global_expanded: bool,
    ) -> String {
        let backend = TestBackend::new(90, 24);
        let mut terminal = Terminal::new(backend).expect("test backend");
        let mut scroll = PanelScroll::new();
        let expanded = HashSet::new();
        let mut plain_lines = Vec::new();

        terminal
            .draw(|frame| {
                let _ = render(
                    frame,
                    frame.area(),
                    &blocks,
                    "test-model",
                    "Conversation",
                    &mut scroll,
                    false,
                    false,
                    &expanded,
                    global_expanded,
                    &mut plain_lines,
                );
            })
            .expect("draw conversation");

        format!("{:?}", terminal.backend().buffer())
    }

    #[test]
    fn request_blocks_use_arrow_symbol() {
        assert_eq!(ConversationBlockType::Request.symbol(), "▶");
    }

    #[test]
    fn runtime_blocks_use_semantic_symbols() {
        assert_eq!(ConversationBlockType::Understanding.symbol(), "◈");
        assert_eq!(ConversationBlockType::PlanProposal.symbol(), "☰");
        assert_eq!(ConversationBlockType::Execution.symbol(), "●");
        assert_eq!(ConversationBlockType::Streaming.symbol(), "◐");
        assert_eq!(ConversationBlockType::Decision.symbol(), "◆");
        assert_eq!(ConversationBlockType::Result.symbol(), "✓");
        assert_eq!(ConversationBlockType::Error.symbol(), "✗");
    }

    #[test]
    fn tool_icon_maps_known_tool_names() {
        assert_eq!(tool_icon("shell"), "▸");
        assert_eq!(tool_icon("read_file"), "▤");
        assert_eq!(tool_icon("write_file"), "✎");
        assert_eq!(tool_icon("edit_file"), "✎");
        assert_eq!(tool_icon("list_dir"), "▣");
        assert_eq!(tool_icon("search_code"), "⚲");
        assert_eq!(tool_icon("run_local_service"), "◉");
        // Unknown / MCP tool names fall back to the generic glyph.
        assert_eq!(tool_icon("mcp_search"), "◈");
        assert_eq!(tool_icon("custom_plugin"), "◈");
    }

    #[test]
    fn folded_blocks_only_hint_global_expand_shortcut() {
        rust_i18n::set_locale("en");
        let rendered = render_blocks_for_test_with_global_expand(
            vec![ConversationBlock::result("one\ntwo\nthree\nfour\nfive")],
            false,
        );

        assert!(!rendered.contains("点击展开"));
        assert!(rendered.contains(fold_shortcut_label()));
    }

    #[test]
    fn internal_decision_and_result_blocks_render_as_prompt_and_output() {
        rust_i18n::set_locale("en");

        assert_eq!(ConversationBlockType::Decision.title(), "Prompt");
        assert_eq!(ConversationBlockType::Result.title(), "Output");
    }

    #[test]
    fn default_prompt_titles_are_not_rendered() {
        rust_i18n::set_locale("en");
        let rendered = render_blocks_for_test(vec![
            ConversationBlock::decision("choose one"),
            ConversationBlock::status("waiting"),
            ConversationBlock::result("done"),
            ConversationBlock::error("disk failed"),
        ]);

        assert!(rendered.contains("choose one"));
        assert!(rendered.contains("waiting"));
        assert!(rendered.contains("done"));
        assert!(rendered.contains("disk failed"));
        assert!(!rendered.contains("Prompt"));
        assert!(!rendered.contains("Status"));
        assert!(!rendered.contains("Output"));
        assert!(!rendered.contains("Error"));
    }

    #[test]
    fn custom_prompt_title_is_preserved() {
        rust_i18n::set_locale("en");
        let mut block = ConversationBlock::error("exit=1");
        block.title = Some("Tool(shell) failed".to_string());

        let rendered = render_blocks_for_test(vec![block]);

        assert!(rendered.contains("Tool(shell) failed"));
        assert!(rendered.contains("exit=1"));
        assert!(!rendered.contains("Error"));
    }

    #[test]
    fn welcome_layout_breakpoints_match_design() {
        assert_eq!(resolve_welcome_layout(100, 14), WelcomeLayout::Wide);
        assert_eq!(resolve_welcome_layout(72, 12), WelcomeLayout::Medium);
        assert_eq!(resolve_welcome_layout(46, 12), WelcomeLayout::Compact);
        assert_eq!(resolve_welcome_layout(45, 12), WelcomeLayout::Tiny);
        assert_eq!(resolve_welcome_layout(100, 11), WelcomeLayout::Tiny);
    }

    #[test]
    fn compact_model_name_removes_provider_wrapper() {
        assert_eq!(
            compact_model_name("BigModel(glm-4.5-coding)"),
            "glm-4.5-coding"
        );
        assert_eq!(compact_model_name("glm-4.5"), "glm-4.5");
    }

    #[test]
    fn welcome_wide_renders_full_provider_models() {
        let state = WelcomeState {
            version: "v0.1.0".to_string(),
            ready: true,
            soul: Some("rust-engineer".to_string()),
            model_plan: Some("BigModel(glm-4.5-coding)".to_string()),
            model_execute: Some("BigModel(glm-4.5)".to_string()),
            model_review: Some("BigModel(glm-4-flash)".to_string()),
        };

        let text = render_welcome_lines(&state, 110, 16)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("慎始如终"));
        assert!(text.contains("Version  v0.1.0"));
        assert!(text.contains("SOUL"));
        assert!(text.contains("BigModel(glm-4.5-coding)"));
        assert!(text.contains("Press Enter to start"));
        assert!(!text.contains("Agent Workspace"));
        assert!(!text.contains("Protocol"));
    }

    #[test]
    fn welcome_bordered_layout_expands_to_available_width() {
        let state = WelcomeState {
            version: "v0.1.0".to_string(),
            ready: true,
            soul: Some("rust-engineer".to_string()),
            model_plan: Some("BigModel(glm-4.5-coding)".to_string()),
            model_execute: Some("BigModel(glm-4.5)".to_string()),
            model_review: Some("BigModel(glm-4-flash)".to_string()),
        };

        let wide_110 = render_welcome_lines(&state, 110, 16);
        let wide_140 = render_welcome_lines(&state, 140, 16);
        let medium = render_welcome_lines(&state, 80, 13);
        let compact = render_welcome_lines(&state, 50, 12);

        assert_centered_frame(wide_110.first().unwrap(), 110);
        assert_centered_frame(wide_140.first().unwrap(), 140);
        assert_centered_frame(medium.first().unwrap(), 80);
        assert_centered_frame(compact.first().unwrap(), 50);
        assert!(wide_140.first().unwrap().width() > wide_110.first().unwrap().width());
    }

    #[test]
    fn welcome_medium_and_compact_use_short_model_names() {
        let state = WelcomeState {
            version: "v0.1.0".to_string(),
            ready: true,
            soul: Some("rust-engineer".to_string()),
            model_plan: Some("BigModel(glm-4.5-coding)".to_string()),
            model_execute: Some("BigModel(glm-4.5)".to_string()),
            model_review: Some("BigModel(glm-4-flash)".to_string()),
        };

        let medium = render_welcome_lines(&state, 80, 13)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let compact = render_welcome_lines(&state, 50, 12)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(medium.contains("glm-4.5-coding"));
        assert!(!medium.contains("BigModel("));
        assert!(compact.contains("ALIUS"));
        assert!(compact.contains("Press Enter to start"));
        assert!(!compact.contains("BigModel("));
    }

    #[test]
    fn welcome_tiny_uses_plain_text_without_border() {
        let state = WelcomeState {
            version: "v0.1.0".to_string(),
            ready: false,
            soul: None,
            model_plan: None,
            model_execute: None,
            model_review: None,
        };

        let text = render_welcome_lines(&state, 40, 10)
            .into_iter()
            .map(|line| line.to_string())
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.starts_with("ALIUS"));
        assert!(text.contains("Not selected"));
        assert!(text.contains("Not configured"));
        assert!(text.contains("Press Enter to continue"));
        assert!(!text.contains('╭'));
        assert!(!text.contains('│'));
    }

    fn assert_centered_frame(line: &Line<'_>, available_width: usize) {
        let text = line.to_string();
        let left = text.chars().take_while(|ch| *ch == ' ').count();
        let right = available_width.saturating_sub(line.width());

        assert_eq!(left, WELCOME_FRAME_MARGIN);
        assert_eq!(right, WELCOME_FRAME_MARGIN);
        assert!(text.trim_start().starts_with('╭'));
    }
}
