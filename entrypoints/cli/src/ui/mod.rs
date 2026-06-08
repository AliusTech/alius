//! Welcome screen and help UI for the Alius REPL.

use runtime_config::Settings;
use rust_i18n::t;

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
    let version = option_env!("ALIUS_VERSION").unwrap_or(env!("CARGO_PKG_VERSION"));
    let cwd = std::env::current_dir()
        .map(|p| p.display().to_string())
        .unwrap_or_else(|_| "~".to_string());

    let title = format!("Alius v{}", version);
    let header_pad = width.saturating_sub(title.len() + 5);

    // Top border
    println!(
        "{}╭─── {} {}{}{}╮{}",
        CYAN,
        BOLD,
        title,
        "─".repeat(header_pad),
        RESET,
        RESET
    );

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
        format!("{}{}{}", GREEN, t!("welcome.api_key_configured"), RESET)
    } else {
        format!("{}{}{}", YELLOW, t!("welcome.api_key_not_set"), RESET)
    };

    let info = vec![
        format!("{}{}{}", BOLD, t!("welcome.quick_start"), RESET),
        format!("  {}", t!("welcome.type_prompt")),
        format!("  {}{}{}", GREEN, t!("welcome.init_hint"), RESET),
        format!("  {}{}{}", GREEN, t!("welcome.model_hint"), RESET),
        format!("  {}{}{}", GREEN, t!("welcome.config_hint"), RESET),
        format!("  {}{}{}", GREEN, t!("welcome.help_hint"), RESET),
        String::new(),
        format!("  Model:    {}", settings.llm.model),
        format!("  Provider: {:?}", settings.llm.provider),
        format!("  {}", api_key_status),
    ];

    let left_w = 44;
    let right_w = width.saturating_sub(left_w + 3);

    // Welcome row
    println!(
        "{}│{} {:<left_w$} │{}",
        CYAN,
        CYAN,
        format!("              {}", t!("welcome.title")),
        RESET,
        left_w = left_w
    );

    // Logo + info interleaved
    for (i, line) in logo.iter().enumerate() {
        let tip = info
            .get(i + 1)
            .map(|s| truncate(s, right_w))
            .unwrap_or_default();
        println!(
            "{}│{} {} {:<right_w$} │{}",
            CYAN,
            CYAN,
            line,
            tip,
            RESET,
            right_w = right_w
        );
    }

    // Remaining info rows
    for row in info.iter().skip(logo.len() + 1) {
        println!(
            "{}│{} {:<left_w$} {:<right_w$} │{}",
            CYAN,
            CYAN,
            "",
            truncate(row, right_w),
            RESET,
            left_w = left_w,
            right_w = right_w
        );
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
    println!("{}{}{}", BOLD, t!("help.title"), RESET);
    println!("  {}/init{}        - {}", GREEN, RESET, t!("help.init"));
    println!("  {}/model{}       - {}", GREEN, RESET, t!("help.model"));
    println!("  {}/config{}      - {}", GREEN, RESET, t!("help.config"));
    println!("  {}/session{}     - {}", GREEN, RESET, t!("help.session"));
    println!("  {}/history{}     - {}", GREEN, RESET, t!("help.history"));
    println!("  {}/tools{}       - {}", GREEN, RESET, t!("help.tools"));
    println!("  {}/review{}      - {}", GREEN, RESET, t!("help.review"));
    println!("  {}/memory{}     - {}", GREEN, RESET, t!("help.memory"));
    println!("  {}/doctor{}     - {}", GREEN, RESET, t!("help.doctor"));
    println!("  {}/trace{}      - {}", GREEN, RESET, t!("help.trace"));
    println!("  {}/clear{}       - {}", GREEN, RESET, t!("help.clear"));
    println!("  {}/help{}        - {}", GREEN, RESET, t!("help.help"));
    println!("  {}/quit{}        - {}", GREEN, RESET, t!("help.quit"));
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
