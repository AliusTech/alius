use crate::config::Settings;

const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_WHITE: &str = "\x1b[37m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_RESET: &str = "\x1b[0m";

pub fn render_welcome(settings: &Settings) {
    let width = get_terminal_width();
    let version = env!("CARGO_PKG_VERSION");
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());

    let title = format!("Alius v{}", version);
    let header_width = width.saturating_sub(title.len() + 5);

    // Top border
    println!(
        "{}╭─── {} {}{}╮{}",
        ANSI_CYAN,
        ANSI_BOLD,
        title,
        "─".repeat(header_width),
        ANSI_RESET
    );

    // ASCII block logo - white color
    let logo_lines = [
        format!("{}⢀⣀⠀⡀⠀⠀⣀⣀⡀⡀⠀⡀⢀⣀⡀{}", ANSI_WHITE, ANSI_RESET),
        format!("{}⡎⠉⡆⡇⠀⠀⠉⡏⠁⡇⠀⡇⡎⠉⠁{}", ANSI_WHITE, ANSI_RESET),
        format!("{}⡷⠶⡇⡇⠀⠀⠀⡇⠀⡇⠀⡇⠱⠶⡀{}", ANSI_WHITE, ANSI_RESET),
        format!("{}⡇⠀⡇⣇⣀⡀⣀⣇⡀⢇⣀⠇⣀⣀⠇{}", ANSI_WHITE, ANSI_RESET),
        format!("{}⠁⠀⠁⠉⠉⠁⠉⠉⠁⠈⠉⠀⠉⠉{}", ANSI_WHITE, ANSI_RESET),
    ];

    // Tips section - right side
    let tips_lines = [
        format!("{}Quick Start{}", ANSI_BOLD, ANSI_RESET),
        format!("  {}alius run -p \"prompt\"{}", ANSI_GREEN, ANSI_RESET),
        format!("  {}alius config show{}", ANSI_GREEN, ANSI_RESET),
        format!("  {}alius --help{}", ANSI_GREEN, ANSI_RESET),
        "".to_string(),
        format!("{}Current Config{}", ANSI_BOLD, ANSI_RESET),
    ];

    let api_key_status = if std::env::var(&settings.llm.api_key_env).is_ok() {
        format!("{}API key configured ✓{}", ANSI_GREEN, ANSI_RESET)
    } else {
        format!("{}Set {} to start{}", ANSI_YELLOW, settings.llm.api_key_env, ANSI_RESET)
    };

    // Layout: logo on left (44 chars), tips on right
    let left_width = 44;
    let right_width = width.saturating_sub(left_width + 3);

    // Welcome text row
    println!(
        "{}│{} {}{}│{}",
        ANSI_CYAN,
        pad_right("              Welcome to Alius!", left_width),
        pad_right(&tips_lines[0], right_width),
        ANSI_RESET,
        ANSI_RESET
    );

    // Logo + tips rows
    for (i, logo) in logo_lines.iter().enumerate() {
        let tip_idx = i + 1;
        let tip = tips_lines.get(tip_idx)
            .map(|s| truncate(s, right_width))
            .unwrap_or_default();
        println!(
            "{}│{} {}{}│{}",
            ANSI_CYAN,
            pad_right(logo, left_width),
            pad_right(&tip, right_width),
            ANSI_RESET,
            ANSI_RESET
        );
    }

    // Config section rows - display Model, Provider, Role
    let config_rows = [
        format!("  Model: {}", settings.llm.model),
        format!("  Provider: {}", settings.llm.provider),
        format!("  Role: {}", settings.soul.as_ref().map(|s| s.role.as_str()).unwrap_or("Not set")),
    ];
    for row in config_rows.iter() {
        println!(
            "{}│{} {}{}│{}",
            ANSI_CYAN,
            pad_right("", left_width),
            pad_right(&truncate(row, right_width), right_width),
            ANSI_RESET,
            ANSI_RESET
        );
    }

    // Status line
    let status_left = format!("      {}alius v{} · LLM Agent CLI{}", ANSI_BOLD, version, ANSI_RESET);
    println!(
        "{}│{} {}{}│{}",
        ANSI_CYAN,
        pad_right(&status_left, left_width),
        pad_right(&api_key_status, right_width),
        ANSI_RESET,
        ANSI_RESET
    );

    // Path line
    let path_display = truncate(&cwd, width.saturating_sub(10));
    println!(
        "{}│{}{}│{}",
        ANSI_CYAN,
        pad_right(&format!("   {}", path_display), width - 2),
        ANSI_RESET,
        ANSI_RESET
    );

    // Bottom border
    println!("{}╰{}╯{}", ANSI_CYAN, "─".repeat(width.saturating_sub(2)), ANSI_RESET);
}

fn get_terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
        .max(60)
}

fn pad_right(s: &str, width: usize) -> String {
    let visible_len = visible_len(s);
    if visible_len >= width {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(width - visible_len))
    }
}

fn truncate(s: &str, max_width: usize) -> String {
    let visible = visible_len(s);
    if visible <= max_width {
        s.to_string()
    } else {
        let mut result = String::new();
        let mut count = 0;
        for c in s.chars() {
            if c == '\x1b' {
                result.push(c);
                continue;
            }
            if count >= max_width - 3 {
                result.push_str("...");
                break;
            }
            result.push(c);
            count += 1;
        }
        result
    }
}

fn visible_len(s: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in s.chars() {
        if c == '\x1b' {
            in_escape = true;
            continue;
        }
        if in_escape {
            if c == 'm' {
                in_escape = false;
            }
            continue;
        }
        len += 1;
    }
    len
}