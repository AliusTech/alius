//! REPL prompt rendering.

use crate::repl::mode::ReplMode;

pub fn build_prompt(mode: ReplMode, model: &str) -> String {
    format!("Alius[{}] {} > ", mode.as_str(), model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prompt_includes_mode_and_model() {
        assert_eq!(
            build_prompt(ReplMode::Chat, "gpt-4o-mini"),
            "Alius[chat] gpt-4o-mini > "
        );
        assert_eq!(
            build_prompt(ReplMode::Plan, "gpt-4o-mini"),
            "Alius[plan] gpt-4o-mini > "
        );
    }
}
