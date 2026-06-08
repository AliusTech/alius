//! REPL and TUI slash command completion helpers.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlashCommand {
    pub command: &'static str,
    pub summary_key: &'static str,
}

pub const SLASH_COMMANDS: &[SlashCommand] = &[
    SlashCommand {
        command: "/init",
        summary_key: "workspace.help.init",
    },
    SlashCommand {
        command: "/mode",
        summary_key: "workspace.help.mode",
    },
    SlashCommand {
        command: "/model",
        summary_key: "workspace.help.model",
    },
    SlashCommand {
        command: "/config",
        summary_key: "workspace.help.config",
    },
    SlashCommand {
        command: "/session",
        summary_key: "workspace.help.session",
    },
    SlashCommand {
        command: "/history",
        summary_key: "workspace.help.history",
    },
    SlashCommand {
        command: "/review",
        summary_key: "workspace.help.review",
    },
    SlashCommand {
        command: "/memory",
        summary_key: "workspace.help.memory",
    },
    SlashCommand {
        command: "/doctor",
        summary_key: "workspace.help.doctor",
    },
    SlashCommand {
        command: "/trace",
        summary_key: "workspace.help.trace",
    },
    SlashCommand {
        command: "/confirm",
        summary_key: "workspace.help.confirm",
    },
    SlashCommand {
        command: "/tools",
        summary_key: "workspace.help.tools",
    },
    SlashCommand {
        command: "/clear",
        summary_key: "workspace.help.clear",
    },
    SlashCommand {
        command: "/help",
        summary_key: "workspace.help.help",
    },
    SlashCommand {
        command: "/quit",
        summary_key: "workspace.help.quit",
    },
    SlashCommand {
        command: "/exit",
        summary_key: "workspace.help.quit",
    },
];

pub const MODE_SUBCOMMANDS: &[&str] = &["chat", "plan", "toggle"];
pub const CONFIG_SUBCOMMANDS: &[&str] = &["show"];
pub const SESSION_SUBCOMMANDS: &[&str] = &["current", "new", "list", "load", "clear"];
pub const REVIEW_SUBCOMMANDS: &[&str] = &["on", "off", "true", "false"];
pub const MEMORY_SUBCOMMANDS: &[&str] = &["show", "save", "list", "clear"];
pub const CONFIRM_SUBCOMMANDS: &[&str] = &["on", "off", "yes", "no", "true", "false"];
pub const TRACE_SUBCOMMANDS: &[&str] = &["latest", "show"];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionMatch {
    pub display: String,
    pub replacement: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionResult {
    pub start: usize,
    pub end: usize,
    pub matches: Vec<CompletionMatch>,
}

pub fn command_names() -> impl Iterator<Item = &'static str> {
    SLASH_COMMANDS.iter().map(|command| command.command)
}

pub fn complete(line: &str, cursor: usize, models: &[String]) -> Option<CompletionResult> {
    let line_to_cursor = line.chars().take(cursor).collect::<String>();
    if !line_to_cursor.trim_start().starts_with('/') {
        return None;
    }

    let (start, word) = extract_word(&line_to_cursor);
    let choices = completion_choices(&line_to_cursor, models);
    let matches = choices
        .into_iter()
        .filter(|choice| choice.starts_with(word))
        .map(|choice| CompletionMatch {
            display: choice.to_string(),
            replacement: choice.to_string(),
        })
        .collect::<Vec<_>>();

    if matches.is_empty() {
        None
    } else {
        Some(CompletionResult {
            start,
            end: cursor,
            matches,
        })
    }
}

pub fn root_matches(prefix: &str) -> Vec<&'static SlashCommand> {
    if !prefix.starts_with('/') || prefix.contains(char::is_whitespace) {
        return Vec::new();
    }

    SLASH_COMMANDS
        .iter()
        .filter(|command| command.command.starts_with(prefix))
        .collect()
}

pub fn common_prefix(matches: &[CompletionMatch]) -> Option<String> {
    let mut iter = matches.iter();
    let mut prefix = iter.next()?.replacement.clone();
    for item in iter {
        prefix = common_prefix_two(&prefix, &item.replacement);
        if prefix.is_empty() {
            break;
        }
    }
    Some(prefix)
}

fn completion_choices<'a>(line_to_cursor: &'a str, models: &'a [String]) -> Vec<&'a str> {
    match command_context(line_to_cursor) {
        Some("/mode") => MODE_SUBCOMMANDS.to_vec(),
        Some("/config") => CONFIG_SUBCOMMANDS.to_vec(),
        Some("/session") => SESSION_SUBCOMMANDS.to_vec(),
        Some("/review") => REVIEW_SUBCOMMANDS.to_vec(),
        Some("/memory") => MEMORY_SUBCOMMANDS.to_vec(),
        Some("/confirm") => CONFIRM_SUBCOMMANDS.to_vec(),
        Some("/trace") => TRACE_SUBCOMMANDS.to_vec(),
        Some("/model") => models.iter().map(String::as_str).collect(),
        _ => command_names().collect(),
    }
}

fn command_context(line_to_cursor: &str) -> Option<&str> {
    let mut parts = line_to_cursor.split_whitespace();
    let command = parts.next()?;
    if !command.starts_with('/') || parts.next().is_none() {
        return None;
    }
    Some(command)
}

fn extract_word(line: &str) -> (usize, &str) {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return (0, "");
    }
    let start = trimmed
        .rfind(char::is_whitespace)
        .map(|index| index + 1)
        .unwrap_or(0);
    let start_chars = trimmed[..start].chars().count();
    (start_chars, &trimmed[start..])
}

fn common_prefix_two(left: &str, right: &str) -> String {
    left.chars()
        .zip(right.chars())
        .take_while(|(left, right)| left == right)
        .map(|(left, _)| left)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn completes_root_command_prefix() {
        let result = complete("/he", 3, &[]).unwrap();

        assert_eq!(result.start, 0);
        assert_eq!(result.matches[0].replacement, "/help");
    }

    #[test]
    fn completes_session_subcommands() {
        let result = complete("/session l", 10, &[]).unwrap();

        assert_eq!(result.start, 9);
        assert_eq!(result.matches[0].replacement, "list");
        assert!(result.matches.iter().any(|item| item.replacement == "load"));
    }

    #[test]
    fn ignores_non_command_input() {
        assert!(complete("hello /he", 9, &[]).is_none());
    }

    #[test]
    fn returns_common_prefix() {
        let result = complete("/session l", 10, &[]).unwrap();

        assert_eq!(common_prefix(&result.matches).as_deref(), Some("l"));
    }
}
