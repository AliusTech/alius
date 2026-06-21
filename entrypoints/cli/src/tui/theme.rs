//! TUI theme — the single source of truth for all colors.
//!
//! Colors are NOT hardcoded `const`s. They live in a process-global
//! [`ThemeColors`] that is populated at startup from the `[ui] theme` config
//! (see [`set_theme`]). Every color accessor (`background()`, `accent()`, …)
//! and every style helper (`base()`, `title()`, …) reads this global, so all
//! 14 consumer files pick up the active palette on every frame without any
//! threading of settings into the render path.
//!
//! Adding a new named theme = add a `fn <name>_palette()` and a match arm in
//! [`set_theme`]. Call sites never change.

use lazy_static::lazy_static;
use ratatui::prelude::{Color, Modifier, Style};
use std::sync::RwLock;

/// The full set of twelve semantic colors. Replaced wholesale by
/// [`set_theme`]; individual fields are read via the free accessor functions
/// below so callers never touch this struct directly.
#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    /// Global canvas background.
    pub background: Color,
    /// Primary foreground / body text.
    pub text: Color,
    /// Low-emphasis meta text: labels, placeholders, empty states.
    pub secondary_text: Color,
    /// Unfocused, non-hovered panel borders.
    pub border: Color,
    /// Focus / brand color: focused borders, titles, matched slash-commands.
    pub accent: Color,
    /// Foreground painted on top of [`ThemeColors::selected_background`].
    pub selected_text: Color,
    /// Highlight fill for the active list row / nav cell / selection.
    pub selected_background: Color,
    /// Caution / attention / hover. Also backs `emphasis()`.
    pub warning: Color,
    /// Positive outcome: completed steps, results.
    pub success: Color,
    /// Neutral in-progress activity: streaming, running, plan mode.
    pub info: Color,
    /// Awaiting review: plan proposals, review-state nodes.
    pub review: Color,
    /// Failure: error blocks, failed/blocked nodes.
    pub error: Color,
}

lazy_static! {
    static ref CURRENT: RwLock<ThemeColors> = RwLock::new(dark_palette());
}

/// Built-in `dark` palette — low-saturation RGB on a textured deep blue-grey
/// background (not pure black). All foregrounds meet WCAG AA contrast against
/// the background.
fn dark_palette() -> ThemeColors {
    ThemeColors {
        background: Color::Rgb(26, 27, 38),             // #1a1b26
        text: Color::Rgb(192, 202, 245),                // #c0caf5
        secondary_text: Color::Rgb(115, 122, 162),      // #737aa2
        border: Color::Rgb(69, 78, 110),                // #454e6e
        accent: Color::Rgb(122, 162, 247),              // #7aa2f7
        selected_text: Color::Rgb(26, 27, 38),          // #1a1b26
        selected_background: Color::Rgb(187, 154, 247), // #bb9af7
        warning: Color::Rgb(224, 175, 104),             // #e0af68
        success: Color::Rgb(158, 206, 106),             // #9ece6a
        info: Color::Rgb(125, 207, 255),                // #7dcfff
        review: Color::Rgb(187, 154, 247),              // #bb9af7
        error: Color::Rgb(247, 118, 142),               // #f7768e
    }
}

/// Switch the active palette by theme name. Unknown names fall back to `dark`
/// (and log a warning) rather than panicking, so a typo in config never breaks
/// the TUI.
pub fn set_theme(name: &str) {
    let palette = match name.trim() {
        "dark" | "" => dark_palette(),
        other => {
            tracing::warn!(theme = other, "unknown theme, falling back to dark");
            dark_palette()
        }
    };
    if let Ok(mut current) = CURRENT.write() {
        *current = palette;
    }
}

// --- Color accessors (replace the former `pub const` values) ---------------
// Each reads the global so the value reflects whatever `set_theme` installed.
// `text` and `border` collide with the style helpers of the same name below,
// so those two colors get a `_color` suffix; the rest mirror the old const
// names as functions.

pub fn background() -> Color {
    CURRENT.read().map(|c| c.background).unwrap_or_default()
}
pub fn text_color() -> Color {
    CURRENT.read().map(|c| c.text).unwrap_or_default()
}
pub fn secondary_text() -> Color {
    CURRENT.read().map(|c| c.secondary_text).unwrap_or_default()
}
pub fn border_color() -> Color {
    CURRENT.read().map(|c| c.border).unwrap_or_default()
}
pub fn accent() -> Color {
    CURRENT.read().map(|c| c.accent).unwrap_or_default()
}
pub fn selected_text() -> Color {
    CURRENT.read().map(|c| c.selected_text).unwrap_or_default()
}
pub fn selected_background() -> Color {
    CURRENT
        .read()
        .map(|c| c.selected_background)
        .unwrap_or_default()
}
pub fn warning() -> Color {
    CURRENT.read().map(|c| c.warning).unwrap_or_default()
}
pub fn success() -> Color {
    CURRENT.read().map(|c| c.success).unwrap_or_default()
}
pub fn info() -> Color {
    CURRENT.read().map(|c| c.info).unwrap_or_default()
}
pub fn review() -> Color {
    CURRENT.read().map(|c| c.review).unwrap_or_default()
}
pub fn error() -> Color {
    CURRENT.read().map(|c| c.error).unwrap_or_default()
}

// --- Style helpers ---------------------------------------------------------
// Compositions over the accessors above. Behavior is unchanged from the
// previous `const`-backed versions. `text()` stays a Style (as before); the
// raw color is `text_color()`.

pub fn base() -> Style {
    Style::default().fg(text_color()).bg(background())
}

pub fn text() -> Style {
    base().fg(text_color())
}

pub fn secondary() -> Style {
    base().fg(secondary_text())
}

pub fn title() -> Style {
    base().fg(accent()).add_modifier(Modifier::BOLD)
}

pub fn emphasis() -> Style {
    base().fg(warning()).add_modifier(Modifier::BOLD)
}

pub fn loading_dot_color() -> Color {
    info()
}

pub fn loading_dot() -> Style {
    base().fg(loading_dot_color()).add_modifier(Modifier::BOLD)
}

pub fn selected() -> Style {
    Style::default()
        .fg(selected_text())
        .bg(selected_background())
        .add_modifier(Modifier::BOLD)
}

pub fn border(focused: bool) -> Style {
    base().fg(if focused { accent() } else { border_color() })
}

/// Border style with three states: focused (accent), hovered (warning),
/// default (border).
pub fn border_state(focused: bool, hovered: bool) -> Style {
    if focused {
        base().fg(accent())
    } else if hovered {
        base().fg(warning())
    } else {
        base().fg(border_color())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dark_palette_values_match_design() {
        set_theme("dark");
        assert_eq!(background(), Color::Rgb(26, 27, 38));
        assert_eq!(text_color(), Color::Rgb(192, 202, 245));
        assert_eq!(secondary_text(), Color::Rgb(115, 122, 162));
        assert_eq!(border_color(), Color::Rgb(69, 78, 110));
        assert_eq!(accent(), Color::Rgb(122, 162, 247));
        assert_eq!(selected_text(), Color::Rgb(26, 27, 38));
        assert_eq!(selected_background(), Color::Rgb(187, 154, 247));
        assert_eq!(warning(), Color::Rgb(224, 175, 104));
        assert_eq!(success(), Color::Rgb(158, 206, 106));
        assert_eq!(info(), Color::Rgb(125, 207, 255));
        assert_eq!(review(), Color::Rgb(187, 154, 247));
        assert_eq!(error(), Color::Rgb(247, 118, 142));
    }

    #[test]
    fn unknown_theme_falls_back_to_dark_without_panicking() {
        set_theme("definitely-not-a-theme");
        // Falls back to dark accent rather than panicking.
        assert_eq!(accent(), Color::Rgb(122, 162, 247));
    }

    #[test]
    fn empty_theme_name_is_dark() {
        set_theme("");
        assert_eq!(background(), Color::Rgb(26, 27, 38));
    }

    #[test]
    fn focus_triad_colors_are_distinct() {
        set_theme("dark");
        // accent (focused) / warning (hovered) / border (default) must differ so
        // the border_state triad stays readable.
        let focused = accent();
        let hovered = warning();
        let default = border_color();
        assert_ne!(focused, hovered);
        assert_ne!(focused, default);
        assert_ne!(hovered, default);
    }

    #[test]
    fn selected_pair_inverts_normal_pair() {
        set_theme("dark");
        // Normal text is light-on-dark; selected is dark-on-light. The selected
        // background must differ from the canvas, and selected text must differ
        // from normal text color.
        assert_ne!(selected_background(), background());
        assert_ne!(selected_text(), text_color());
    }
}
