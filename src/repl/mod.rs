use crate::config::{Settings, SOUL_ROLES, PROVIDERS};
use crate::error::{AliusError, Result};
use crate::llm::client::LlmClient;
use inquire::{Select as InquireSelect, Text, Password};
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::{Hinter, HistoryHinter};
use rustyline::history::DefaultHistory;
use rustyline::validate::{Validator, MatchingBracketValidator};
use rustyline::{Config, Context, Editor, Helper};
use std::borrow::Cow;
use std::sync::Arc;
use tokio::sync::RwLock;

const ANSI_CYAN: &str = "\x1b[36m";
const ANSI_GREEN: &str = "\x1b[32m";
const ANSI_YELLOW: &str = "\x1b[33m";
const ANSI_BOLD: &str = "\x1b[1m";
const ANSI_RESET: &str = "\x1b[0m";

const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o",
    "gpt-4o-mini",
    "gpt-4-turbo",
    "gpt-3.5-turbo",
    "claude-3-5-sonnet-20241022",
    "claude-3-opus-20240229",
    "claude-3-haiku-20240307",
    "gemini-1.5-pro",
    "gemini-1.5-flash",
];

const COMMANDS: &[&str] = &["/model", "/soul", "/config", "/help", "/quit", "/exit"];

struct ReplCompleter {
    models: Vec<String>,
}

impl Completer for ReplCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line_to_pos = &line[..pos];
        let (start, word) = extract_word(line_to_pos);

        let completions: Vec<Pair> = if word.starts_with('/') {
            // Command completion
            COMMANDS
                .iter()
                .filter(|cmd| cmd.starts_with(word))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect()
        } else if is_after_command(line_to_pos, "/model") {
            // Model completion after /model
            self.models
                .iter()
                .filter(|m| m.starts_with(word))
                .map(|m| Pair {
                    display: m.clone(),
                    replacement: m.clone(),
                })
                .collect()
        } else if is_after_command(line_to_pos, "/soul") {
            // Role completion after /soul
            SOUL_ROLES
                .iter()
                .filter(|r| r.to_lowercase().starts_with(&word.to_lowercase()))
                .map(|r| Pair {
                    display: r.to_string(),
                    replacement: format!("\"{}\"", r),
                })
                .collect()
        } else {
            Vec::new()
        };

        Ok((start, completions))
    }
}

fn extract_word(line: &str) -> (usize, &str) {
    let trimmed = line.trim_end();
    if trimmed.is_empty() {
        return (0, "");
    }

    // Find start of current word (after last space)
    let start = trimmed
        .rfind(|c: char| c.is_whitespace())
        .map(|i| i + 1)
        .unwrap_or(0);

    (start, &trimmed[start..])
}

fn is_after_command(line: &str, command: &str) -> bool {
    let trimmed = line.trim();
    if let Some(rest) = trimmed.strip_prefix(command) {
        rest.is_empty() || rest.starts_with(' ')
    } else {
        false
    }
}

#[derive(Helper)]
struct ReplHelper {
    completer: ReplCompleter,
    hinter: HistoryHinter,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
}

impl Hinter for ReplHelper {
    type Hint = String;

    fn hint(&self, line: &str, pos: usize, ctx: &Context<'_>) -> Option<String> {
        self.hinter.hint(line, pos, ctx)
    }
}

impl Completer for ReplHelper {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        self.completer.complete(line, pos, ctx)
    }
}

impl Highlighter for ReplHelper {
    fn highlight<'l>(&self, line: &'l str, pos: usize) -> Cow<'l, str> {
        self.highlighter.highlight(line, pos)
    }

    fn highlight_char(&self, line: &str, pos: usize, forced: bool) -> bool {
        self.highlighter.highlight_char(line, pos, forced)
    }
}

impl Validator for ReplHelper {
    fn validate(&self, ctx: &mut rustyline::validate::ValidationContext<'_>) -> rustyline::Result<rustyline::validate::ValidationResult> {
        self.validator.validate(ctx)
    }
}

pub struct ReplSession {
    settings: Arc<RwLock<Settings>>,
    client: Option<LlmClient>,
    models: Vec<String>,
}

impl ReplSession {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Arc::new(RwLock::new(settings)),
            client: None,
            models: DEFAULT_MODELS.iter().map(|s| s.to_string()).collect(),
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        // Check if soul is configured, if not prompt for selection
        {
            let settings = self.settings.read().await;
            if settings.soul.is_none() {
                drop(settings); // Release lock before prompting
                self.select_soul().await?;
            }
        }

        // Try to fetch models from server
        self.fetch_models().await;

        let config = Config::builder()
            .completion_type(rustyline::CompletionType::List)
            .build();

        let mut rl: Editor<ReplHelper, DefaultHistory> = Editor::with_config(config)
            .map_err(|e| AliusError::Repl(e.to_string()))?;

        let helper = ReplHelper {
            completer: ReplCompleter { models: self.models.clone() },
            hinter: HistoryHinter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            validator: MatchingBracketValidator::new(),
        };
        rl.set_helper(Some(helper));

        loop {
            let (role, model) = {
                let settings = self.settings.read().await;
                let role = settings.soul.as_ref().map(|s| s.role.clone()).unwrap_or_else(|| "User".to_string());
                let model = settings.llm.model.clone();
                (role, model)
            };
            let prompt = format!("{}{} ({})> {}", ANSI_BOLD, role, model, ANSI_RESET);

            let input = rl.readline(&prompt);

            match input {
                Ok(line) if !line.trim().is_empty() => {
                    let _ = rl.add_history_entry(&line);
                    if self.handle_command(&line).await? {
                        break;
                    }
                }
                Ok(_) => continue,
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    continue;
                }
                Err(ReadlineError::Eof) => {
                    println!("^D");
                    break;
                }
                Err(e) => {
                    return Err(AliusError::Repl(e.to_string()));
                }
            }
        }

        println!("\n{}Goodbye!{}", ANSI_CYAN, ANSI_RESET);
        Ok(())
    }

    async fn fetch_models(&mut self) {
        let settings = self.settings.read().await.clone();

        match LlmClient::for_model_list(&settings) {
            Ok(client) => {
                match client.list_models().await {
                    Ok(models) => {
                        if !models.is_empty() {
                            // Filter out non-chat models and sort
                            let chat_models: Vec<String> = models
                                .into_iter()
                                .filter(|m| {
                                    m.contains("gpt") ||
                                    m.contains("claude") ||
                                    m.contains("gemini") ||
                                    m.contains("chat")
                                })
                                .collect();
                            if !chat_models.is_empty() {
                                self.models = chat_models;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("{}Warning: Could not fetch models from server: {}{}", ANSI_YELLOW, e, ANSI_RESET);
                        eprintln!("{}Using default model list.{}", ANSI_YELLOW, ANSI_RESET);
                    }
                }
            }
            Err(_) => {
                // API key not configured, use defaults
            }
        }
    }

    async fn handle_command(&mut self, input: &str) -> Result<bool> {
        let trimmed = input.trim();

        // Handle quit/exit
        if trimmed == "/quit" || trimmed == "/exit" {
            return Ok(true);
        }

        // Handle inline model command: /model <model_name>
        if let Some(model_arg) = trimmed.strip_prefix("/model ") {
            let model = model_arg.trim();
            if self.models.contains(&model.to_string()) {
                self.set_model(model).await;
            } else {
                println!(
                    "{}Unknown model: {}{}",
                    ANSI_YELLOW, model, ANSI_RESET
                );
                println!("Available models: {}", self.models.join(", "));
            }
            return Ok(false);
        }

        // Handle inline soul command: /soul <role>
        if let Some(role_arg) = trimmed.strip_prefix("/soul ") {
            let role = role_arg.trim_matches('"').trim();
            if SOUL_ROLES.contains(&role) {
                self.set_soul(role).await;
            } else {
                println!(
                    "{}Unknown role: {}{}",
                    ANSI_YELLOW, role, ANSI_RESET
                );
                println!("Available roles: {}", SOUL_ROLES.join(", "));
            }
            return Ok(false);
        }

        // Handle standalone commands
        match trimmed {
            "/model" => self.select_model().await?,
            "/soul" => self.select_soul().await?,
            "/config" => self.config_panel().await?,
            "/help" => self.show_help(),
            cmd if cmd.starts_with('/') => {
                println!(
                    "{}Unknown command: {}{}",
                    ANSI_YELLOW, cmd, ANSI_RESET
                );
                println!("Type {}/help{} for available commands", ANSI_GREEN, ANSI_RESET);
            }
            _ => self.chat(trimmed).await?,
        }

        Ok(false)
    }

    async fn set_model(&mut self, model: &str) {
        let mut settings = self.settings.write().await;
        settings.llm.model = model.to_string();
        self.client = None;
        println!("{}Model changed to: {}{}", ANSI_GREEN, model, ANSI_RESET);
    }

    async fn set_soul(&mut self, role: &str) {
        let mut settings = self.settings.write().await;
        settings.soul = Some(crate::config::SoulSettings {
            role: role.to_string(),
        });
        println!("{}Role changed to: {}{}", ANSI_GREEN, role, ANSI_RESET);
    }

    async fn select_model(&mut self) -> Result<()> {
        let current_model = self.settings.read().await.llm.model.clone();

        let default_index = self.models
            .iter()
            .position(|m| m == &current_model)
            .unwrap_or(0);

        let selection = InquireSelect::new("Select a model:", self.models.clone())
            .with_starting_cursor(default_index)
            .prompt();

        match selection {
            Ok(model) => {
                self.set_model(&model).await;
            }
            Err(_) => {
                println!("{}Model selection cancelled{}", ANSI_YELLOW, ANSI_RESET);
            }
        }

        Ok(())
    }

    async fn select_soul(&mut self) -> Result<()> {
        let current_role = self
            .settings
            .read()
            .await
            .soul
            .as_ref()
            .map(|s| s.role.clone())
            .unwrap_or_else(|| SOUL_ROLES[0].to_string());

        let default_index = SOUL_ROLES
            .iter()
            .position(|r| r == &current_role)
            .unwrap_or(0);

        let selection = InquireSelect::new("Select your role:", SOUL_ROLES.to_vec())
            .with_starting_cursor(default_index)
            .prompt();

        match selection {
            Ok(role) => {
                self.set_soul(role).await;
            }
            Err(_) => {
                println!("{}Role selection cancelled{}", ANSI_YELLOW, ANSI_RESET);
            }
        }

        Ok(())
    }

    async fn config_panel(&mut self) -> Result<()> {
        println!();
        println!("{}Configuration Panel{}", ANSI_BOLD, ANSI_RESET);
        println!();

        loop {
            let settings = self.settings.read().await;
            println!("{}Current Settings:{}", ANSI_CYAN, ANSI_RESET);
            println!("  1. Provider:  {}", settings.llm.provider);
            println!("  2. Base URL:  {}", settings.effective_base_url());
            println!("  3. API Key:   {}", if settings.llm.api_key.is_some() { "***configured***" } else { "not set" });
            println!("  4. Model:     {}", settings.llm.model);
            println!("  5. Role:      {}", settings.soul.as_ref().map(|s| s.role.as_str()).unwrap_or("not set"));
            println!("  6. Save & Exit");
            println!("  7. Cancel");
            println!();

            drop(settings);

            let choice: Result<String> = Text::new("Select option (1-7):")
                .prompt()
                .map_err(|e| AliusError::Repl(e.to_string()));

            let choice = choice?;

            match choice.as_str() {
                "1" => {
                    let provider = InquireSelect::new("Select provider:", PROVIDERS.to_vec())
                        .prompt()
                        .map_err(|e| AliusError::Repl(e.to_string()))?;
                    let mut settings = self.settings.write().await;
                    settings.llm.provider = provider.to_string();
                    // Reset base_url to use default for new provider
                    settings.llm.base_url = None;
                }
                "2" => {
                    let current = self.settings.read().await.effective_base_url();
                    let base_url: String = Text::new("Enter base URL:")
                        .with_default(&current)
                        .prompt()
                        .map_err(|e| AliusError::Repl(e.to_string()))?;
                    let mut settings = self.settings.write().await;
                    settings.llm.base_url = Some(base_url);
                }
                "3" => {
                    let api_key = Password::new("Enter API key:")
                        .without_confirmation()
                        .prompt()
                        .map_err(|e| AliusError::Repl(e.to_string()))?;
                    let mut settings = self.settings.write().await;
                    settings.llm.api_key = Some(api_key);
                    self.client = None;
                }
                "4" => {
                    drop(self.select_model().await);
                }
                "5" => {
                    drop(self.select_soul().await);
                }
                "6" => {
                    let settings = self.settings.read().await.clone();
                    settings.save_to_user_config()?;
                    println!("{}Configuration saved!{}", ANSI_GREEN, ANSI_RESET);
                    // Refresh models with new config
                    self.fetch_models().await;
                    break;
                }
                "7" => {
                    println!("{}Configuration cancelled{}", ANSI_YELLOW, ANSI_RESET);
                    break;
                }
                _ => {
                    println!("{}Invalid option{}", ANSI_YELLOW, ANSI_RESET);
                }
            }
            println!();
        }

        Ok(())
    }

    async fn show_config(&self) -> Result<()> {
        let settings = self.settings.read().await;
        println!();
        println!("{}Current Configuration:{}", ANSI_BOLD, ANSI_RESET);
        println!("  Provider: {}", settings.llm.provider);
        println!("  Base URL: {}", settings.effective_base_url());
        println!("  Model:    {}", settings.llm.model);
        if settings.llm.api_key.is_some() {
            println!("  API Key:  ***configured***");
        } else {
            println!("  API Key:  {} (env: {})",
                if std::env::var(&settings.llm.api_key_env).is_ok() { "***from env***" } else { "not set" },
                settings.llm.api_key_env);
        }
        if let Some(soul) = &settings.soul {
            println!("  Role:     {}", soul.role);
        }
        println!();
        Ok(())
    }

    fn show_help(&self) {
        println!();
        println!("{}Available Commands:{}", ANSI_BOLD, ANSI_RESET);
        println!(
            "  {}/model{}    - Select model (or use: /model <name>)",
            ANSI_GREEN, ANSI_RESET
        );
        println!(
            "  {}/soul{}     - Select role (or use: /soul \"<role>\")",
            ANSI_GREEN, ANSI_RESET
        );
        println!("  {}/config{}   - Open configuration panel", ANSI_GREEN, ANSI_RESET);
        println!("  {}/help{}     - Show this help message", ANSI_GREEN, ANSI_RESET);
        println!("  {}/quit{}     - Exit the REPL", ANSI_GREEN, ANSI_RESET);
        println!();
        println!("{}Press TAB for auto-completion{}", ANSI_YELLOW, ANSI_RESET);
        println!();
        println!("{}Or just type your prompt to chat with the LLM{}", ANSI_YELLOW, ANSI_RESET);
        println!();
    }

    async fn chat(&mut self, prompt: &str) -> Result<()> {
        let settings = self.settings.read().await;

        if self.client.is_none() {
            self.client = Some(LlmClient::new(&settings)?);
        }

        let client = self.client.as_ref().unwrap();
        println!();

        let response = client.chat(prompt).await?;
        println!("{}{}", ANSI_RESET, response);
        println!();

        Ok(())
    }
}