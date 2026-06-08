//! REPL text rendering helpers.

use crate::repl::mode::ReplMode;

pub fn mode_switched(mode: ReplMode) -> String {
    format!("Mode switched to {}", mode.as_str())
}
