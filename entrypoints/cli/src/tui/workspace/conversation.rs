use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use super::helpers::count_visual_lines;
use super::PanelScroll;
use crate::tui::state::{ConversationBlock, ConversationBlockType, MainTab};
use crate::tui::theme;

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
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(tab_title)
        .style(theme::base())
        .border_style(theme::border_state(focused, hovered));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                t!("workspace.conversation.model").to_string(),
                theme::secondary(),
            ),
            Span::styled(model.to_string(), theme::text()),
        ]),
        Line::default(),
    ];

    for block in blocks {
        lines.push(Line::from(Span::styled(
            block.block_type.title(),
            Style::default()
                .fg(block.block_type.color())
                .bg(theme::BACKGROUND)
                .add_modifier(Modifier::BOLD),
        )));
        for line in block.content.lines() {
            lines.push(Line::from(line.to_string()));
        }
        lines.push(Line::default());
    }

    let total_visual = count_visual_lines(&lines, inner.width);
    let max_off = total_visual.saturating_sub(inner.height as usize) as u16;
    scroll.snap_to_bottom(max_off);
    scroll.clamp(max_off);

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll.offset, 0));
    frame.render_widget(paragraph, inner);
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

    pub fn decision(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Decision, content)
    }

    pub fn result(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Result, content)
    }

    pub fn error(content: impl Into<String>) -> Self {
        Self::new(ConversationBlockType::Error, content)
    }

    fn new(block_type: ConversationBlockType, content: impl Into<String>) -> Self {
        Self {
            block_type,
            title: None,
            content: content.into(),
        }
    }
}

impl ConversationBlockType {
    pub fn title(self) -> String {
        match self {
            Self::Request => t!("workspace.block.request").to_string(),
            Self::Understanding => t!("workspace.block.understanding").to_string(),
            Self::PlanProposal => t!("workspace.block.plan_proposal").to_string(),
            Self::Execution => t!("workspace.block.execution").to_string(),
            Self::Decision => t!("workspace.block.decision").to_string(),
            Self::Result => t!("workspace.block.result").to_string(),
            Self::Error => t!("workspace.block.error").to_string(),
        }
    }

    pub fn color(self) -> Color {
        match self {
            Self::Request => theme::ACCENT,
            Self::Understanding => theme::INFO,
            Self::PlanProposal => theme::REVIEW,
            Self::Execution => theme::WARNING,
            Self::Decision => theme::WARNING,
            Self::Result => theme::SUCCESS,
            Self::Error => theme::ERROR,
        }
    }
}
