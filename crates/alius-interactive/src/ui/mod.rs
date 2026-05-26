//! Welcome screen UI

pub fn show_welcome(model: &str, provider: &str) {
    println!();
    println!("╭─── Alius v0.2.0 ───────────────────────────────╮");
    println!("│                                               │");
    println!("│  Welcome to Alius                             │");
    println!("│                                               │");
    println!("│  Model: {:<35} │", model);
    println!("│  Provider: {:<33} │", provider);
    println!("│                                               │");
    println!("│  Type /help for available commands            │");
    println!("│  Type /quit or /exit to leave                 │");
    println!("│                                               │");
    println!("╰────────────────────────────────────────────────╯");
    println!();
}

pub fn show_help() {
    println!();
    println!("Available commands:");
    println!("  /model [name]  - Switch model (interactive if no name)");
    println!("  /soul [role]   - Switch soul role");
    println!("  /config        - Show configuration panel");
    println!("  /clear         - Clear conversation history");
    println!("  /help          - Show this help");
    println!("  /quit, /exit   - Exit Alius");
    println!();
}