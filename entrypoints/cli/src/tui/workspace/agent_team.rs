use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use super::helpers::{count_visual_lines, truncate_chars};
use super::PanelScroll;
use crate::tui::state::{
    A2ADirection, A2AMessageStatus, A2AMessageType, AgentEndpoint, AgentTeamState,
};
use crate::tui::theme;

pub fn render(
    frame: &mut Frame,
    area: Rect,
    team: Option<&AgentTeamState>,
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
        Line::from(Span::styled(
            t!("workspace.agent_team.title").to_string(),
            theme::title(),
        )),
        Line::default(),
    ];

    match team {
        Some(team) if !team.messages.is_empty() => {
            for view in &team.messages {
                let time = time_part(&view.message.created_at);
                let direction = match view.direction {
                    A2ADirection::In => t!("workspace.a2a.direction_in").to_string(),
                    A2ADirection::Out => t!("workspace.a2a.direction_out").to_string(),
                };
                let from = endpoint_label(&view.message.from);
                let to = endpoint_label(&view.message.to);
                lines.push(Line::from(format!(
                    "{}  {}  {} → {}",
                    time, direction, from, to
                )));
                lines.push(Line::from(format!(
                    "{}  {}",
                    a2a_type_label(view.message.message_type),
                    a2a_status_label(view.message.status)
                )));
                lines.push(Line::from(truncate_chars(&view.message.content, 96)));
                lines.push(Line::default());
            }
        }
        Some(_) => {
            lines.push(Line::from(Span::styled(
                t!("workspace.agent_team.no_messages").to_string(),
                theme::secondary(),
            )));
        }
        None => {
            lines.push(Line::from(Span::styled(
                t!("workspace.agent_team.not_connected").to_string(),
                theme::secondary(),
            )));
        }
    }

    let total_visual = count_visual_lines(&lines, inner.width);
    let max_off = total_visual.saturating_sub(inner.height as usize) as u16;
    scroll.snap_to_bottom(max_off);
    scroll.clamp(max_off);

    // Clear stale glyphs before repaint (see conversation.rs for rationale), and
    // set the theme background so unfilled rows are themed, not default.
    frame.render_widget(ratatui::widgets::Clear, inner);
    frame.render_widget(
        Paragraph::new(Text::from(lines))
            .style(theme::base())
            .wrap(Wrap { trim: false })
            .scroll((scroll.offset, 0)),
        inner,
    );
}

fn endpoint_label(endpoint: &AgentEndpoint) -> String {
    endpoint
        .role
        .as_deref()
        .filter(|role| !role.trim().is_empty())
        .unwrap_or(&endpoint.soul)
        .to_string()
}

fn a2a_type_label(message_type: A2AMessageType) -> &'static str {
    match message_type {
        A2AMessageType::PlanRequest => "A2A.plan.request",
        A2AMessageType::PlanResponse => "A2A.plan.response",
        A2AMessageType::TaskDelegate => "A2A.task.delegate",
        A2AMessageType::TaskResult => "A2A.task.result",
        A2AMessageType::ReviewRequest => "A2A.review.request",
        A2AMessageType::ReviewResponse => "A2A.review.response",
        A2AMessageType::ContextShare => "A2A.context.share",
        A2AMessageType::Error => "A2A.error",
        A2AMessageType::Heartbeat => "A2A.heartbeat",
    }
}

fn a2a_status_label(status: A2AMessageStatus) -> String {
    match status {
        A2AMessageStatus::Sending => t!("workspace.a2a.status.sending").to_string(),
        A2AMessageStatus::Sent => t!("workspace.a2a.status.sent").to_string(),
        A2AMessageStatus::Delivered => t!("workspace.a2a.status.delivered").to_string(),
        A2AMessageStatus::Acknowledged => t!("workspace.a2a.status.acknowledged").to_string(),
        A2AMessageStatus::Failed => t!("workspace.a2a.status.failed").to_string(),
    }
}

fn time_part(created_at: &str) -> String {
    created_at
        .split('T')
        .nth(1)
        .and_then(|time| time.split('.').next())
        .or_else(|| created_at.split_whitespace().last())
        .map(|time| time.chars().take(8).collect())
        .unwrap_or_else(|| created_at.chars().take(8).collect())
}
