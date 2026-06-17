//! Test utilities for the TUI subsystem.
//!
//! Provides shared helpers for TUI state-machine tests: key event
//! construction, text input simulation, and execution mode helpers.
//!
//! These helpers are only available in test builds because they access
//! private methods on `WorkspaceState`.

#![allow(dead_code)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use super::workspace::{WorkspaceAction, WorkspaceState};

/// Create a `KeyEvent` with no modifiers.
///
/// # Examples
///
/// ```ignore
/// let k = key(KeyCode::Enter);
/// let action = state.handle_key(k, &[]);
/// ```
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Create a `KeyEvent` with a modifier.
///
/// # Examples
///
/// ```ignore
/// let k = key_with(KeyCode::Tab, KeyModifiers::SHIFT);
/// ```
pub fn key_with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// Simulate typing text character by character into a `WorkspaceState`.
///
/// Each character is inserted via `handle_key` with `KeyCode::Char`.
pub fn type_text(state: &mut WorkspaceState, value: &str) {
    for ch in value.chars() {
        let _ = state.handle_key(key(KeyCode::Char(ch)), &[]);
    }
}

/// Simulate typing text and pressing Enter, returning the resulting action.
///
/// Returns the `WorkspaceAction` produced by the Enter key press.
pub fn submit_input(state: &mut WorkspaceState, value: &str) -> WorkspaceAction {
    type_text(state, value);
    state.handle_key(key(KeyCode::Enter), &[])
}

/// Handle a key event in execution mode.
///
/// Convenience wrapper that calls `handle_key` and expects the state
/// to be in execution mode.
pub fn handle_execution_key(state: &mut WorkspaceState, key_event: KeyEvent) -> WorkspaceAction {
    state.handle_key(key_event, &[])
}
