use ratatui::prelude::*;
use ratatui::widgets::Paragraph;

use crate::tui::state::WorkspaceStatus;
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, status: &WorkspaceStatus) {
    frame.render_widget(
        Paragraph::new(status.display(area.width as usize)).style(theme::secondary()),
        area,
    );
}
