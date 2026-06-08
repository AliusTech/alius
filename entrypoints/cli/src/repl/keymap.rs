//! REPL keymap actions.
//!
//! Shift+Tab maps to `ToggleMode` where terminal support is available. `/mode`
//! remains the command fallback for terminals that do not expose BackTab.

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplAction {
    Submit(String),
    ToggleMode,
    Command(String),
    Cancel,
}
