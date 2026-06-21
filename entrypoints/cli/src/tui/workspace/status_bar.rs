use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use super::helpers::{char_len, truncate_chars};
use crate::tui::state::WorkspaceStatus;
use crate::tui::theme;

pub struct StatusFeedback<'a> {
    pub message: &'a str,
    pub is_error: bool,
}

pub fn render(frame: &mut Frame, area: Rect, status: &WorkspaceStatus, feedback: Option<StatusFeedback<'_>>) {
    let width = area.width as usize;
    let Some(feedback) = feedback else {
        frame.render_widget(
            Paragraph::new(status.display(width)).style(theme::secondary()),
            area,
        );
        return;
    };

    let feedback_style = if feedback.is_error {
        theme::base().fg(theme::error())
    } else {
        theme::base().fg(theme::success())
    };
    let feedback_text = truncate_chars(feedback.message, width);
    let feedback_width = char_len(&feedback_text);
    let left_width = width.saturating_sub(feedback_width.saturating_add(1));
    let left = truncate_chars(&status.display(width), left_width);
    let spacing = width.saturating_sub(char_len(&left).saturating_add(feedback_width));

    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled(left, theme::secondary()),
            Span::raw(" ".repeat(spacing)),
            Span::styled(feedback_text, feedback_style),
        ]))
        .style(theme::base()),
        area,
    );
}
