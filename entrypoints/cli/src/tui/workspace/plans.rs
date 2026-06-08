use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use super::helpers::{count_visual_lines, truncate_chars};
use super::PanelScroll;
use crate::tui::state::{AgentTeamState, PlanNode, PlanNodeStatus};
use crate::tui::theme;

#[allow(clippy::too_many_arguments)]
pub fn render(
    frame: &mut Frame,
    area: Rect,
    plans: &[PlanNode],
    has_agent_team: bool,
    agent_team: Option<&AgentTeamState>,
    scroll: &mut PanelScroll,
    focused: bool,
    hovered: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", t!("workspace.plans.title")))
        .style(theme::base())
        .border_style(theme::border_state(focused, hovered));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let mut lines = Vec::new();
    if plans.is_empty() {
        lines.push(Line::from(Span::styled(
            t!("workspace.plans.waiting").to_string(),
            theme::secondary(),
        )));
        lines.push(Line::default());
        lines.push(Line::from(t!("workspace.plans.progress").to_string()));
        lines.push(Line::from(
            t!("workspace.plans.completed_count", completed = 0, total = 0).to_string(),
        ));
    } else {
        for (index, node) in plans.iter().enumerate() {
            let (symbol, color) = node.status.symbol_style();
            let owner = node
                .owner
                .as_deref()
                .map(|owner| format!(" @{}", owner))
                .unwrap_or_default();
            let title = truncate_chars(
                &format!("{}{}", node.title, owner),
                inner.width.saturating_sub(7) as usize,
            );
            lines.push(Line::from(vec![
                Span::styled(symbol, Style::default().fg(color).bg(theme::BACKGROUND)),
                Span::styled(format!(" {}. {}", index + 1, title), theme::text()),
            ]));
        }

        lines.push(Line::default());
        if let Some(review) = plans
            .iter()
            .position(|node| node.status == PlanNodeStatus::Review)
        {
            lines.push(Line::from(t!("workspace.plans.review").to_string()));
            lines.push(Line::from(
                t!("workspace.plans.node_waiting", node = review + 1).to_string(),
            ));
        } else if has_agent_team {
            lines.push(Line::from(t!("workspace.plans.agentnet").to_string()));
            lines.push(Line::from(if agent_team.is_some() {
                t!("workspace.plans.connected").to_string()
            } else {
                t!("workspace.plans.offline").to_string()
            }));
        } else {
            let completed = plans
                .iter()
                .filter(|node| {
                    matches!(
                        node.status,
                        PlanNodeStatus::Completed | PlanNodeStatus::Approved
                    )
                })
                .count();
            lines.push(Line::from(t!("workspace.plans.progress").to_string()));
            lines.push(Line::from(
                t!(
                    "workspace.plans.completed_count",
                    completed = completed,
                    total = plans.len()
                )
                .to_string(),
            ));
        }
    }

    let total_visual = count_visual_lines(&lines, inner.width);
    let max_off = total_visual.saturating_sub(inner.height as usize) as u16;
    scroll.clamp(max_off);

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll.offset, 0));
    frame.render_widget(paragraph, inner);
}

impl PlanNode {
    pub fn new(id: impl Into<String>, title: impl Into<String>, status: PlanNodeStatus) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status,
            description: None,
            acceptance_criteria: Vec::new(),
            evidence: Vec::new(),
            owner: None,
        }
    }

    pub fn with_owner(mut self, owner: impl Into<String>) -> Self {
        self.owner = Some(owner.into());
        self
    }
}

impl PlanNodeStatus {
    pub fn symbol_style(self) -> (&'static str, Color) {
        match self {
            Self::Pending => ("○", theme::SECONDARY_TEXT),
            Self::Running => ("●", theme::WARNING),
            Self::Completed => ("✓", theme::SUCCESS),
            Self::Review => ("◎", theme::REVIEW),
            Self::Approved => ("✔", theme::SUCCESS),
            Self::Revising => ("↻", theme::WARNING),
            Self::Failed => ("×", theme::ERROR),
            Self::Blocked => ("⚠", theme::ERROR),
        }
    }
}
