use ratatui::prelude::{Color, Modifier, Style};
use std::time::{SystemTime, UNIX_EPOCH};

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

pub fn loading_dot_color() -> Color {
    let phase = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| (duration.as_millis() / 180) % 8)
        .unwrap_or(0);

    // Amber breathing-light ramp: same hue, changing brightness depth.
    match phase {
        0 => Color::Rgb(82, 64, 18),
        1 => Color::Rgb(128, 96, 24),
        2 => Color::Rgb(184, 138, 32),
        3 => Color::Rgb(244, 183, 43),
        4 => Color::Rgb(255, 214, 88),
        5 => Color::Rgb(244, 183, 43),
        6 => Color::Rgb(184, 138, 32),
        _ => Color::Rgb(128, 96, 24),
    }
}

pub fn loading_dot() -> Style {
    base().fg(loading_dot_color()).add_modifier(Modifier::BOLD)
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
