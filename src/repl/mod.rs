use crate::config::{Settings, SOUL_ROLES};
use crate::error::{AliusError, Result};
use crate::llm::client::LlmClient;
use inquire::Select as InquireSelect;
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

const AVAILABLE_MODELS: &[&str] = &[
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

struct ReplCompleter;

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
            AVAILABLE_MODELS
                .iter()
                .filter(|m| m.starts_with(word))
                .map(|m| Pair {
                    display: m.to_string(),
                    replacement: m.to_string(),
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
}

impl ReplSession {
    pub fn new(settings: Settings) -> Self {
        Self {
            settings: Arc::new(RwLock::new(settings)),
            client: None,
        }
    }

    pub async fn run(&mut self) -> Result<()> {
        let config = Config::builder()
            .completion_type(rustyline::CompletionType::List)
            .build();

        let mut rl: Editor<ReplHelper, DefaultHistory> = Editor::with_config(config)
            .map_err(|e| AliusError::Repl(e.to_string()))?;

        let helper = ReplHelper {
            completer: ReplCompleter,
            hinter: HistoryHinter::new(),
            highlighter: MatchingBracketHighlighter::new(),
            validator: MatchingBracketValidator::new(),
        };
        rl.set_helper(Some(helper));

        loop {
            let model_name = self.settings.read().await.llm.model.clone();
            let prompt = format!("{}{}{}> ", ANSI_BOLD, model_name, ANSI_RESET);

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

    async fn handle_command(&mut self, input: &str) -> Result<bool> {
        let trimmed = input.trim();

        // Handle quit/exit
        if trimmed == "/quit" || trimmed == "/exit" {
            return Ok(true);
        }

        // Handle inline model command: /model <model_name>
        if let Some(model_arg) = trimmed.strip_prefix("/model ") {
            let model = model_arg.trim();
            if AVAILABLE_MODELS.contains(&model) {
                self.set_model(model).await;
            } else {
                println!(
                    "{}Unknown model: {}{}",
                    ANSI_YELLOW, model, ANSI_RESET
                );
                println!("Available models: {}", AVAILABLE_MODELS.join(", "));
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
            "/config" => self.show_config().await?,
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

        let default_index = AVAILABLE_MODELS
            .iter()
            .position(|m| m == &current_model)
            .unwrap_or(0);

        let selection = InquireSelect::new("Select a model:", AVAILABLE_MODELS.to_vec())
            .with_starting_cursor(default_index)
            .prompt();

        match selection {
            Ok(model) => {
                self.set_model(model).await;
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

    async fn show_config(&self) -> Result<()> {
        let settings = self.settings.read().await;
        println!();
        println!("{}Current Configuration:{}", ANSI_BOLD, ANSI_RESET);
        println!("  Model:    {}", settings.llm.model);
        println!("  Provider: {}", settings.llm.provider);
        println!("  API Key:  {}", settings.llm.api_key_env);
        if let Some(base_url) = &settings.llm.base_url {
            println!("  Base URL: {}", base_url);
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
        println!("  {}/config{}   - Display current configuration", ANSI_GREEN, ANSI_RESET);
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