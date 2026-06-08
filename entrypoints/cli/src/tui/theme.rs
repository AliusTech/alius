use ratatui::prelude::{Color, Modifier, Style};

pub const BACKGROUND: Color = Color::Black;
pub const TEXT: Color = Color::White;
pub const SECONDARY_TEXT: Color = Color::Gray;
pub const BORDER: Color = Color::Gray;
pub const ACCENT: Color = Color::Cyan;
pub const WARNING: Color = Color::Yellow;
pub const SUCCESS: Color = Color::Green;
pub const INFO: Color = Color::Blue;
pub const REVIEW: Color = Color::Magenta;
pub const ERROR: Color = Color::Red;
pub const SELECTED_TEXT: Color = Color::Black;
pub const SELECTED_BACKGROUND: Color = Color::Cyan;

pub fn base() -> Style {
    Style::default().fg(TEXT).bg(BACKGROUND)
}

pub fn text() -> Style {
    base().fg(TEXT)
}

pub fn secondary() -> Style {
    base().fg(SECONDARY_TEXT)
}

pub fn title() -> Style {
    base().fg(ACCENT).add_modifier(Modifier::BOLD)
}

pub fn emphasis() -> Style {
    base().fg(WARNING).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .fg(SELECTED_TEXT)
        .bg(SELECTED_BACKGROUND)
        .add_modifier(Modifier::BOLD)
}

pub fn border(focused: bool) -> Style {
    base().fg(if focused { ACCENT } else { BORDER })
}

/// Border style with three states: focused (Cyan), hovered (Yellow), default (Gray).
pub fn border_state(focused: bool, hovered: bool) -> Style {
    if focused {
        base().fg(ACCENT)
    } else if hovered {
        base().fg(WARNING)
    } else {
        base().fg(BORDER)
    }
}
