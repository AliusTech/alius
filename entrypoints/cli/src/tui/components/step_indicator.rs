use ratatui::prelude::{Line, Modifier, Rect, Span};
use ratatui::widgets::Paragraph;

use crate::tui::theme;

pub struct StepIndicator {
    steps: Vec<String>,
    current: usize,
}

impl StepIndicator {
    pub fn new(steps: Vec<String>) -> Self {
        Self { steps, current: 0 }
    }

    pub fn set_current(&mut self, step: usize) {
        self.current = step;
    }

    pub fn render(&self, frame: &mut ratatui::Frame, area: Rect) {
        let mut spans: Vec<Span> = Vec::new();

        for (i, label) in self.steps.iter().enumerate() {
            if i > 0 {
                spans.push(Span::styled(" ── ", theme::secondary()));
            }

            let span = if i < self.current {
                Span::styled(
                    format!("✓ {}", label),
                    theme::base()
                        .fg(theme::SUCCESS)
                        .add_modifier(Modifier::BOLD),
                )
            } else if i == self.current {
                Span::styled(
                    format!("▶ {}", label),
                    theme::base().fg(theme::TEXT).add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(label.clone(), theme::secondary())
            };

            spans.push(span);
        }

        let paragraph = Paragraph::new(Line::from(spans)).style(theme::text());
        frame.render_widget(paragraph, area);
    }
}
