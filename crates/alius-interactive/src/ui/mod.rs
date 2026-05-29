//! Welcome screen and help UI for the Alius REPL.

use alius_config::Settings;

// ANSI color codes
const CYAN: &str = "\x1b[36m";
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const WHITE: &str = "\x1b[37m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Render the welcome screen with logo, version, and current config.
pub fn show_welcome(settings: &Settings) {
    let width = terminal_width();
    let version = option_env!("ALIUS_VERSION")
        .unwrap_or(env!("CARGO_PKG_VERSION"));
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());

    let title = format!("Alius v{}", version);
    let header_pad = width.saturating_sub(title.len() + 5);

    // Top border
    println!("{}╭─── {} {}{}{}╮{}", CYAN, BOLD, title, "─".repeat(header_pad), RESET, RESET);

    // Logo rows
    let logo = [
        format!("{}⢀⣀⠀⡀⠀⠀⣀⣀⡀⡀⠀⡀⢀⣀⡀{}", WHITE, RESET),
        format!("{}⡎⠉⡆⡇⠀⠀⠉⡏⠁⡇⠀⡇⡎⠉⠁{}", WHITE, RESET),
        format!("{}⡷⠶⡇⡇⠀⠀⠀⡇⠀⡇⠀⡇⠱⠶⡀{}", WHITE, RESET),
        format!("{}⡇⠀⡇⣇⣀⡀⣀⣇⡀⢇⣀⠇⣀⣀⠇{}", WHITE, RESET),
        format!("{}⠁⠀⠁⠉⠉⠁⠉⠉⠁⠈⠉⠀⠉⠉{}", WHITE, RESET),
    ];

    // Right side info
    let api_key_status = if settings.api_key().is_ok() {
        format!("{}API key configured ✓{}", GREEN, RESET)
    } else {
        format!("{}API key not set{}", YELLOW, RESET)
    };

    let info = vec![
        format!("{}Quick Start{}", BOLD, RESET),
        format!("  Type your prompt to chat"),
        format!("  {}/model{} to switch model", GREEN, RESET),
        format!("  {}/config{} for settings", GREEN, RESET),
        format!("  {}/help{} for all commands", GREEN, RESET),
        String::new(),
        format!("  Model:    {}", settings.llm.model),
        format!("  Provider: {:?}", settings.llm.provider),
        format!("  {}", api_key_status),
    ];

    let left_w = 44;
    let right_w = width.saturating_sub(left_w + 3);

    // Welcome row
    println!("{}│{} {:<left_w$} │{}", CYAN, CYAN, "              Welcome to Alius!", RESET, left_w = left_w);

    // Logo + info interleaved
    for (i, line) in logo.iter().enumerate() {
        let tip = info.get(i + 1).map(|s| truncate(s, right_w)).unwrap_or_default();
        println!("{}│{} {} {:<right_w$} │{}", CYAN, CYAN, line, tip, RESET, right_w = right_w);
    }

    // Remaining info rows
    for row in info.iter().skip(logo.len() + 1) {
        println!("{}│{} {:<left_w$} {:<right_w$} │{}", CYAN, CYAN, "", truncate(row, right_w), RESET, left_w = left_w, right_w = right_w);
    }

    // CWD
    let path_display = truncate(&cwd, width.saturating_sub(10));
    println!("{}│{}   {} {}│{}", CYAN, CYAN, path_display, RESET, RESET);

    // Bottom border
    println!("{}╰{}╯{}", CYAN, "─".repeat(width.saturating_sub(2)), RESET);
    println!();
}

/// Show help with all available commands.
pub fn show_help() {
    println!();
    println!("{}Available Commands:{}", BOLD, RESET);
    println!("  {}/model{}       - Select model (interactive)", GREEN, RESET);
    println!("  {}/soul{}        - Select soul role (interactive)", GREEN, RESET);
    println!("  {}/config{}      - Config panel (/config show for details)", GREEN, RESET);
    println!("  {}/session{}     - Session (current|new|list|load|clear)", GREEN, RESET);
    println!("  {}/history{}     - Show conversation history", GREEN, RESET);
    println!("  {}/tools{}       - List available tools", GREEN, RESET);
    println!("  {}/review{}      - Review last answer (review_model)", GREEN, RESET);
    println!("  {}/memory{}     - Memory (show|save|list|clear)", GREEN, RESET);
    println!("  {}/doctor{}     - System health check", GREEN, RESET);
    println!("  {}/trace{}      - Show conversation trace", GREEN, RESET);
    println!("  {}/clear{}       - Clear conversation history", GREEN, RESET);
    println!("  {}/help{}        - Show this help", GREEN, RESET);
    println!("  {}/quit{}        - Exit Alius", GREEN, RESET);
    println!();
}

fn terminal_width() -> usize {
    terminal_size::terminal_size()
        .map(|(w, _)| w.0 as usize)
        .unwrap_or(80)
        .max(60)
}

fn truncate(s: &str, max: usize) -> String {
    let visible = visible_len(s);
    if visible <= max {
        s.to_string()
    } else {
        let mut result = String::new();
        let mut count = 0;
        for c in s.chars() {
            if c == '\x1b' {
                result.push(c);
                continue;
            }
            if count >= max.saturating_sub(3) {
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