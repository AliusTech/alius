//! Interactive REPL

use anyhow::Result;
use std::sync::Arc;
use std::io::Write;
use std::borrow::Cow;

use alius_config::{Settings, system_prompt_for_role, SOUL_ROLES};
use alius_model::{LlmClient, Conversation, AliusAgent, AgentEvent};
use alius_tools::{ToolRegistry, register_builtin_tools};
use alius_store::{SessionStore, ConversationStore};
use alius_protocol::SessionMetadata;
use inquire::{Select as InquireSelect, Text, Password};
use rustyline::completion::{Completer, Pair};
use rustyline::highlight::{Highlighter, MatchingBracketHighlighter};
use rustyline::hint::HistoryHinter;
use rustyline::validate::{Validator, MatchingBracketValidator};
use rustyline::{Config, Context, Helper};

// ANSI color codes
const GREEN: &str = "\x1b[32m";
const YELLOW: &str = "\x1b[33m";
const BOLD: &str = "\x1b[1m";
const RESET: &str = "\x1b[0m";

/// Default model list when provider doesn't support model listing.
const DEFAULT_MODELS: &[&str] = &[
    "gpt-4o", "gpt-4o-mini", "gpt-4-turbo",
    "claude-sonnet-4-20250514", "claude-3-5-sonnet-20241022", "claude-3-haiku-20240307",
    "gemini-1.5-pro", "gemini-1.5-flash",
    "deepseek-chat", "deepseek-reasoner",
];

/// Available slash commands for tab completion.
const COMMANDS: &[&str] = &[
    "/model", "/soul", "/config", "/session", "/history",
    "/review", "/memory", "/confirm", "/tools", "/clear", "/help", "/quit", "/exit",
];

/// Tab-completion helper for rustyline.
struct ReplCompleter {
    models: Vec<String>,
}

impl Completer for ReplCompleter {
    type Candidate = Pair;

    fn complete(&self, line: &str, pos: usize, _ctx: &Context<'_>) -> rustyline::Result<(usize, Vec<Pair>)> {
        let line_to_pos = &line[..pos];
        let (start, word) = extract_word(line_to_pos);

        let completions: Vec<Pair> = if word.starts_with('/') {
            COMMANDS.iter()
                .filter(|cmd| cmd.starts_with(word))
                .map(|cmd| Pair { display: cmd.to_string(), replacement: cmd.to_string() })
                .collect()
        } else if is_after_command(line_to_pos, "/model") {
            self.models.iter()
                .filter(|m| m.starts_with(word))
                .map(|m| Pair { display: m.clone(), replacement: m.clone() })
                .collect()
        } else if is_after_command(line_to_pos, "/soul") {
            SOUL_ROLES.iter()
                .filter(|r| r.to_lowercase().starts_with(&word.to_lowercase()))
                .map(|r| Pair { display: r.to_string(), replacement: format!("\"{}\"", r) })
                .collect()
        } else {
            Vec::new()
        };

        Ok((start, completions))
    }
}

/// rustyline helper combining completion, hints, highlighting, and validation.
#[derive(Helper)]
struct ReplHelper {
    completer: ReplCompleter,
    hinter: HistoryHinter,
    highlighter: MatchingBracketHighlighter,
    validator: MatchingBracketValidator,
}

impl rustyline::hint::Hinter for ReplHelper {
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

fn extract_word(line: &str) -> (usize, &str) {
    let trimmed = line.trim_end();
    if trimmed.is_empty() { return (0, ""); }
    let start = trimmed.rfind(|c: char| c.is_whitespace()).map(|i| i + 1).unwrap_or(0);
    (start, &trimmed[start..])
}

fn is_after_command(line: &str, command: &str) -> bool {
    let trimmed = line.trim();
    trimmed.strip_prefix(command).is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// REPL session
pub struct ReplSession {
    settings: Arc<std::sync::RwLock<Settings>>,
    client: Option<Arc<LlmClient>>,
    agent: Option<AliusAgent>,
    conversation: Conversation,
    registry: Arc<ToolRegistry>,
    session_metadata: SessionMetadata,
    session_store: SessionStore,
    conversation_store: ConversationStore,
    workspace: std::path::PathBuf,
    auto_confirm: bool,
    auto_review: bool,
    models: Vec<String>,
}

impl ReplSession {
    /// Create a new REPL session
    pub fn new(settings: Settings) -> Result<Self> {
        let client = LlmClient::new(settings.llm.clone()).ok().map(Arc::new);

        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);
        let registry = Arc::new(registry);

        let agent = client.as_ref().map(|c| {
            AliusAgent::new(c.clone(), registry.clone(), settings.clone())
                .with_auto_confirm(true)
        });

        // Load system prompt: prefer activated Soul prompts, fallback to hardcoded role
        let system_prompt = alius_formula::current_project_soul()
            .and_then(|id| alius_formula::load_soul_prompts(&id))
            .unwrap_or_else(|| system_prompt_for_role(&settings.soul.role));
        let conversation = Conversation::new(Some(system_prompt));

        let session_metadata = SessionMetadata::new(settings.llm.model.clone());
        let session_store = SessionStore::new()?;
        let conversation_store = ConversationStore::new()?;
        let workspace = std::env::current_dir()?;

        // Persist session metadata on creation
        session_store.save(&session_metadata)?;

        Ok(Self {
            settings: Arc::new(std::sync::RwLock::new(settings)),
            client,
            agent,
            conversation,
            registry,
            session_metadata,
            session_store,
            conversation_store,
            workspace,
            auto_confirm: true,
            auto_review: false,
            models: DEFAULT_MODELS.iter().map(|s| s.to_string()).collect(),
        })
    }

    pub fn model(&self) -> String {
        self.settings.read().unwrap().llm.model.clone()
    }

    pub fn soul(&self) -> String {
        self.settings.read().unwrap().soul.role.to_string()
    }

    /// Build system prompt: Soul prompts + memories.
    fn build_system_prompt(&self) -> String {
        let base = alius_formula::current_project_soul()
            .and_then(|id| alius_formula::load_soul_prompts(&id))
            .unwrap_or_else(|| system_prompt_for_role(&self.settings.read().unwrap().soul.role));

        // Append memories
        let mut parts = vec![base];
        if let Ok(global) = alius_store::memory::MemoryStore::global() {
            let text = global.all_text();
            if !text.is_empty() {
                parts.push(format!("User memories:\n{}", text));
            }
        }
        if let Ok(project) = alius_store::memory::MemoryStore::project() {
            let text = project.all_text();
            if !text.is_empty() {
                parts.push(format!("Project memories:\n{}", text));
            }
        }
        parts.join("\n\n")
    }

    /// Fetch available models from the provider.
    async fn fetch_models(&mut self) {
        let settings = self.settings.read().unwrap().clone();
        match LlmClient::new(settings.llm) {
            Ok(client) => match client.list_models().await {
                Ok(models) if !models.is_empty() => {
                    let chat_models: Vec<String> = models.into_iter()
                        .filter(|m| {
                            m.contains("gpt") || m.contains("claude") || m.contains("gemini")
                                || m.contains("deepseek") || m.contains("chat") || m.contains("llama")
                        })
                        .collect();
                    if !chat_models.is_empty() {
                        self.models = chat_models;
                    }
                }
                _ => { /* keep defaults */ }
            },
            Err(_) => { /* no API key, keep defaults */ }
        }
    }

    /// Rebuild client and agent after settings change.
    fn rebuild_client(&mut self) {
        let settings = self.settings.read().unwrap().clone();
        self.client = LlmClient::new(settings.llm).ok().map(Arc::new);
        self.agent = self.client.as_ref().map(|c| {
            AliusAgent::new(c.clone(), self.registry.clone(), self.settings.read().unwrap().clone())
                .with_auto_confirm(self.auto_confirm)
        });
    }

    /// Handle user input
    pub async fn handle_input(&mut self, input: &str) -> Result<String> {
        if input.starts_with('/') {
            return self.handle_command(input).await;
        }
        if input == "exit" || input == "quit" {
            return Ok("bye!".to_string());
        }

        // Chat with agent
        if let Some(agent) = &self.agent {
            let events = agent.handle_message(
                &mut self.conversation,
                input.to_string(),
                self.workspace.clone(),
                self.session_metadata.id.to_string()
            ).await;

            let mut stdout = std::io::stdout();
            let mut full_response = String::new();

            for event in events {
                match event {
                    AgentEvent::TurnStarted | AgentEvent::ModelStarted => {}
                    AgentEvent::ModelDelta { text } => {
                        stdout.write_all(text.as_bytes())?;
                        stdout.flush()?;
                        full_response.push_str(&text);
                    }
                    AgentEvent::ModelFinished { .. } => { println!(); }
                    AgentEvent::ToolCallStarted { name, args, .. } => {
                        println!("\n  Tool: {}", name);
                        if !args.is_null() {
                            println!("  Args: {}", serde_json::to_string_pretty(&args).unwrap_or_default());
                        }
                    }
                    AgentEvent::ToolConfirmationRequested { name, operation, details, .. } => {
                        println!("\n  Confirm: {} - {}", name, operation);
                        println!("  {}", details);
                        if self.auto_confirm {
                            println!("  [Auto-approved]");
                        } else {
                            let confirm = dialoguer::Confirm::new()
                                .with_prompt("Proceed?").default(false).interact()?;
                            if !confirm {
                                println!("  Denied");
                                return Err(anyhow::anyhow!("Tool denied by user"));
                            }
                        }
                    }
                    AgentEvent::ToolCallFinished { name, success, result, .. } => {
                        let status = if success { "OK" } else { "ERR" };
                        println!("  {} {}", status, name);
                        if result.len() > 200 {
                            println!("  Result: {}...", &result[..200]);
                        } else {
                            println!("  Result: {}", result);
                        }
                    }
                    AgentEvent::ToolConfirmed { .. } | AgentEvent::ToolDenied { .. } | AgentEvent::TurnFinished => {}
                    AgentEvent::Error { message } => {
                        eprintln!("\nError: {}", message);
                        return Err(anyhow::anyhow!("{}", message));
                    }
                }
            }

            self.conversation_store.save_messages(
                &self.session_metadata.id,
                self.conversation.messages()
            )?;
            let _ = self.session_store.update(&mut self.session_metadata);

            // Auto-review if enabled
            if self.auto_review {
                let review_result = self.cmd_review(vec!["/review"]).await;
                match review_result {
                    Ok(review) if !review.is_empty() => {
                        println!("\n\x1b[36m--- Review ---\x1b[0m");
                        println!("{}", review);
                        println!("\x1b[36m--- End Review ---\x1b[0m\n");
                    }
                    Err(e) => eprintln!("\nReview error: {}", e),
                    _ => {}
                }
            }

            return Ok(full_response);
        }

        Err(anyhow::anyhow!("No LLM client configured. Run /config to set up."))
    }

    /// Handle slash command
    async fn handle_command(&mut self, input: &str) -> Result<String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or("");

        match cmd {
            "/model" => self.cmd_model(parts).await,
            "/soul" => self.cmd_soul(parts).await,
            "/config" => {
                if parts.get(1) == Some(&"show") {
                    self.cmd_config_show()
                } else {
                    self.cmd_config().await
                }
            },
            "/session" => self.cmd_session(parts).await,
            "/history" => self.cmd_history(),
            "/confirm" => self.cmd_confirm(parts),
            "/review" => self.cmd_review(parts).await,
            "/memory" => self.cmd_memory(parts),
            "/tools" => Ok(format!("Available tools: {}", self.registry.list_names().join(", "))),
            "/clear" => {
                self.conversation.clear();
                Ok("Conversation cleared".to_string())
            }
            "/help" => {
                crate::ui::show_help();
                Ok(String::new())
            }
            "/quit" | "/exit" => Ok("bye!".to_string()),
            _ => Ok(format!("Unknown command: {}. Type /help for available commands.", cmd)),
        }
    }

    /// /model command - interactive or direct model selection
    async fn cmd_model(&mut self, parts: Vec<&str>) -> Result<String> {
        if parts.len() > 1 {
            let model = parts[1..].join(" ");
            self.settings.write().unwrap().llm.model = model.clone();
            self.rebuild_client();
            return Ok(format!("Model switched to: {}", model));
        }

        // Interactive selection
        let current = self.model();
        let default_idx = self.models.iter().position(|m| m == &current).unwrap_or(0);
        let mut options = self.models.clone();
        options.push(format!("{}Enter model name manually{}", YELLOW, RESET));

        let selection = InquireSelect::new("Select a model:", options)
            .with_starting_cursor(default_idx)
            .prompt();

        match selection {
            Ok(choice) if choice.contains("manually") => {
                let model = Text::new("Model name:")
                    .with_default(&current)
                    .prompt()
                    .map_err(|e| anyhow::anyhow!("{}", e))?;
                self.settings.write().unwrap().llm.model = model.clone();
                self.rebuild_client();
                Ok(format!("Model set to: {}", model))
            }
            Ok(model) => {
                let model = model.replace(&format!("{}{}", YELLOW, RESET), "");
                self.settings.write().unwrap().llm.model = model.clone();
                self.rebuild_client();
                Ok(format!("Model switched to: {}", model))
            }
            Err(_) => Ok("Selection cancelled".to_string()),
        }
    }

    /// /soul command - interactive or direct role selection
    async fn cmd_soul(&mut self, parts: Vec<&str>) -> Result<String> {
        if parts.len() > 1 {
            let role = parts[1..].join(" ");
            self.settings.write().unwrap().soul.role = alius_protocol::SoulRole::new(role.to_string());
            self.conversation.set_system_prompt(self.build_system_prompt());
            return Ok(format!("Soul switched to: {}", role));
        }

        let current = self.soul();
        let default_idx = SOUL_ROLES.iter().position(|r| r == &current).unwrap_or(0);

        let selection = InquireSelect::new("Select a soul role:", SOUL_ROLES.to_vec())
            .with_starting_cursor(default_idx)
            .prompt();

        match selection {
            Ok(role) => {
                self.settings.write().unwrap().soul.role = alius_protocol::SoulRole::new(role.to_string());
                self.conversation.set_system_prompt(self.build_system_prompt());
                Ok(format!("Soul switched to: {}", role))
            }
            Err(_) => Ok("Selection cancelled".to_string()),
        }
    }

    /// /config command - interactive configuration panel
    async fn cmd_config(&mut self) -> Result<String> {
        println!();
        println!("{}Configuration Panel{}", BOLD, RESET);
        println!();

        loop {
            let settings = self.settings.read().unwrap().clone();
            println!("  1. Provider:  {:?}", settings.llm.provider);
            println!("  2. Base URL:  {}", settings.effective_base_url());
            println!("  3. API Key:   {}", if settings.llm.api_key.is_some() { "***set***" } else { "not set" });
            println!("  4. Model:     {}", settings.llm.model);
            println!("  5. Soul:      {}", settings.soul.role);
            println!("  6. Save & Exit");
            println!("  7. Cancel");
            println!();

            let choice = Text::new("Select (1-7):")
                .prompt()
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            match choice.as_str() {
                "1" => {
                    let providers = vec!["openai", "anthropic", "google", "custom"];
                    let provider = InquireSelect::new("Provider:", providers)
                        .prompt()
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    let ptype = match provider {
                        "openai" => alius_protocol::ProviderType::Openai,
                        "anthropic" => alius_protocol::ProviderType::Anthropic,
                        "google" => alius_protocol::ProviderType::Google,
                        _ => alius_protocol::ProviderType::Custom,
                    };
                    self.settings.write().unwrap().llm.provider = ptype;
                    self.settings.write().unwrap().llm.base_url = None;
                }
                "2" => {
                    let current = self.settings.read().unwrap().effective_base_url();
                    let url = Text::new("Base URL:")
                        .with_default(&current)
                        .prompt()
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    self.settings.write().unwrap().llm.base_url = Some(url);
                }
                "3" => {
                    let key = Password::new("API Key:")
                        .without_confirmation()
                        .prompt()
                        .map_err(|e| anyhow::anyhow!("{}", e))?;
                    self.settings.write().unwrap().llm.api_key = Some(key);
                    self.rebuild_client();
                }
                "4" => {
                    drop(self.cmd_model(vec!["/model"]).await);
                }
                "5" => {
                    drop(self.cmd_soul(vec!["/soul"]).await);
                }
                "6" => {
                    self.settings.read().unwrap().save_to_user_config()?;
                    self.rebuild_client();
                    self.fetch_models().await;
                    let s = self.settings.read().unwrap();
                    println!();
                    println!("{}Configuration saved!{}", GREEN, RESET);
                    println!("  Provider: {:?}", s.llm.provider);
                    println!("  Model:    {}", s.llm.model);
                    println!();
                    return Ok(String::new());
                }
                "7" => {
                    return Ok("Config cancelled".to_string());
                }
                _ => println!("Invalid option"),
            }
            println!();
        }
    }

    /// /config show — display config with sources
    fn cmd_config_show(&self) -> Result<String> {
        let s = self.settings.read().unwrap();
        let mut out = String::new();
        out.push_str(&format!("{}Configuration:{}\n", BOLD, RESET));
        out.push_str(&format!("  Provider:  {:?}\n", s.llm.provider));
        out.push_str(&format!("  Model:     {}\n", s.llm.model));
        out.push_str(&format!("  Base URL:  {}\n", s.effective_base_url()));
        out.push_str(&format!("  API Key:   {}\n", if s.llm.api_key.is_some() { "***set***" } else { "not set" }));
        if let Some(ref rm) = s.llm.review_model {
            out.push_str(&format!("  Review:    {}\n", rm));
        }
        out.push_str(&format!("  Soul:      {}\n", s.soul.role));
        out.push_str(&format!("\n{}Config files:{}\n", BOLD, RESET));
        let user_path = dirs_or_home().join(".alius").join("config.toml");
        out.push_str(&format!("  User:    {} {}\n", user_path.display(),
            if user_path.exists() { "✓" } else { "(not found)" }));
        if let Some(proj) = find_project_config_from_cwd() {
            out.push_str(&format!("  Project: {} ✓\n", proj.display()));
        } else {
            out.push_str("  Project: (not found)\n");
        }
        out.push_str("  Env:     ALIUS_* variables\n");
        Ok(out)
    }

    /// /session command
    async fn cmd_session(&mut self, parts: Vec<&str>) -> Result<String> {
        let sub = parts.get(1).copied().unwrap_or("current");
        match sub {
            "current" => {
                Ok(format!("Session: {} | Model: {} | Messages: {}",
                    &self.session_metadata.id.as_str()[..8],
                    self.session_metadata.model,
                    self.conversation.len()))
            }
            "new" => {
                let model = self.settings.read().unwrap().llm.model.clone();
                self.session_metadata = SessionMetadata::new(model);
                self.conversation = Conversation::new(
                    Some(self.build_system_prompt())
                );
                self.session_store.save(&self.session_metadata)?;
                Ok(format!("New session: {}", &self.session_metadata.id.as_str()[..8]))
            }
            "list" => {
                let sessions = self.session_store.list()?;
                if sessions.is_empty() {
                    return Ok("No saved sessions".to_string());
                }
                let mut out = String::from("Sessions:\n");
                for s in &sessions {
                    out.push_str(&format!("  {} | {} | {} | {}\n",
                        &s.id.as_str()[..8],
                        s.model,
                        s.created_at.format("%m-%d %H:%M"),
                        s.updated_at.format("%m-%d %H:%M"),
                    ));
                }
                Ok(out.trim_end().to_string())
            }
            "load" => {
                let id_str = parts.get(2)
                    .ok_or_else(|| anyhow::anyhow!("Usage: /session load <id>"))?;
                // Find matching session by prefix
                let sessions = self.session_store.list()?;
                let session = sessions.iter()
                    .find(|s| s.id.as_str().starts_with(id_str))
                    .ok_or_else(|| anyhow::anyhow!("No session found with prefix: {}", id_str))?;
                let messages = self.conversation_store.load_messages(&session.id)?;
                let system_prompt = messages.iter()
                    .find(|m| m.role == alius_protocol::MessageRole::System)
                    .map(|m| m.content.clone());
                let non_system: Vec<_> = messages.into_iter()
                    .filter(|m| m.role != alius_protocol::MessageRole::System)
                    .collect();
                self.session_metadata = session.clone();
                self.conversation = Conversation::from_messages(
                    system_prompt,
                    non_system,
                );
                Ok(format!("Loaded session: {} ({} messages)",
                    &self.session_metadata.id.as_str()[..8],
                    self.conversation.len()))
            }
            "clear" => {
                self.conversation.clear();
                Ok("Session cleared".to_string())
            }
            _ => Ok("Usage: /session [current|new|list|load <id>|clear]".to_string()),
        }
    }

    /// /history command
    fn cmd_history(&self) -> Result<String> {
        let msgs = self.conversation.messages();
        if msgs.is_empty() {
            return Ok("No messages in history".to_string());
        }
        for (i, msg) in msgs.iter().enumerate() {
            let preview: String = msg.content.chars().take(80).collect();
            let role = match msg.role {
                alius_protocol::MessageRole::System => "SYS",
                alius_protocol::MessageRole::User => "USR",
                alius_protocol::MessageRole::Assistant => "AST",
                alius_protocol::MessageRole::Summary => "SUM",
            };
            println!("  {:3}. [{}] {}", i + 1, role, preview);
            if msg.content.len() > 80 { println!("      ..."); }
        }
        Ok(String::new())
    }

    /// /confirm command
    fn cmd_confirm(&mut self, parts: Vec<&str>) -> Result<String> {
        if let Some(mode) = parts.get(1) {
            match *mode {
                "on" | "yes" | "true" => {
                    self.auto_confirm = true;
                    self.rebuild_client();
                    Ok("Auto-confirm enabled".to_string())
                }
                "off" | "no" | "false" => {
                    self.auto_confirm = false;
                    self.rebuild_client();
                    Ok("Interactive confirmation enabled".to_string())
                }
                _ => Ok("Usage: /confirm on|off".to_string()),
            }
        } else {
            let status = if self.auto_confirm { "on" } else { "off" };
            Ok(format!("Confirm mode: {} (use /confirm on|off)", status))
        }
    }

    /// /review command — use review_model to critique last assistant answer
    async fn cmd_review(&mut self, parts: Vec<&str>) -> Result<String> {
        // Handle on/off toggle
        if let Some(mode) = parts.get(1) {
            match *mode {
                "on" | "true" => {
                    self.auto_review = true;
                    return Ok("Auto-review enabled".to_string());
                }
                "off" | "false" => {
                    self.auto_review = false;
                    return Ok("Auto-review disabled".to_string());
                }
                _ => {}
            }
        }

        // Get last assistant message
        let last_assistant = self.conversation.messages().iter().rev()
            .find(|m| m.role == alius_protocol::MessageRole::Assistant);

        let assistant_text = match last_assistant {
            Some(m) => m.content.clone(),
            None => return Ok("No assistant response to review".to_string()),
        };

        // Build review prompt
        let review_prompt = format!(
            "Please review the following assistant response for correctness, completeness, and quality. \
             Point out any issues, errors, or areas for improvement. Be concise.\n\n\
             Assistant response:\n{}",
            assistant_text
        );

        // Create review client (use review_model if set, otherwise main model)
        let review_settings = {
            let s = self.settings.read().unwrap();
            let mut review_s = s.clone();
            if let Some(ref rm) = s.llm.review_model {
                review_s.llm.model = rm.clone();
            }
            review_s
        };

        let review_client = LlmClient::new(review_settings.llm)?;
        let review_system = Some("You are a code review assistant. Review responses for quality and correctness.");

        let response = review_client.chat_once(&review_prompt, review_system).await?;
        Ok(response)
    }

    /// /memory command
    fn cmd_memory(&self, parts: Vec<&str>) -> Result<String> {
        let sub = parts.get(1).copied().unwrap_or("show");
        match sub {
            "save" => {
                let text = parts[2..].join(" ");
                if text.is_empty() {
                    return Ok("Usage: /memory save <text>".to_string());
                }
                let mut store = alius_store::memory::MemoryStore::global()?;
                store.save(&text)?;
                Ok(format!("Memory saved: {}", text))
            }
            "list" | "show" => {
                let global = alius_store::memory::MemoryStore::global()?;
                let project = alius_store::memory::MemoryStore::project()?;
                let mut out = String::new();
                if !global.list().is_empty() {
                    out.push_str("Global memories:\n");
                    for (i, e) in global.list().iter().enumerate() {
                        out.push_str(&format!("  {}. {}\n", i + 1, e.text));
                    }
                }
                if !project.list().is_empty() {
                    out.push_str("Project memories:\n");
                    for (i, e) in project.list().iter().enumerate() {
                        out.push_str(&format!("  {}. {}\n", i + 1, e.text));
                    }
                }
                if out.is_empty() {
                    out = "No memories saved. Use /memory save <text>".to_string();
                }
                Ok(out.trim_end().to_string())
            }
            "clear" => {
                let mut store = alius_store::memory::MemoryStore::global()?;
                store.clear()?;
                Ok("Global memories cleared".to_string())
            }
            _ => Ok("Usage: /memory [show|save <text>|list|clear]".to_string()),
        }
    }
}

fn dirs_or_home() -> std::path::PathBuf {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("~"))
}

fn find_project_config_from_cwd() -> Option<std::path::PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let mut dir = cwd.as_path();
    loop {
        let candidate = dir.join("alius").join("config.toml");
        if candidate.exists() {
            return Some(candidate);
        }
        dir = dir.parent()?;
    }
}

/// Run the REPL
pub async fn run_repl(settings: Settings) -> Result<()> {
    crate::ui::show_welcome(&settings);
    let mut session = ReplSession::new(settings)?;

    // Fetch models from provider
    session.fetch_models().await;

    // Configure rustyline with completion
    let rl_config = Config::builder()
        .completion_type(rustyline::CompletionType::List)
        .build();

    let helper = ReplHelper {
        completer: ReplCompleter { models: session.models.clone() },
        hinter: HistoryHinter::new(),
        highlighter: MatchingBracketHighlighter::new(),
        validator: MatchingBracketValidator::new(),
    };

    let mut rl: rustyline::Editor<ReplHelper, rustyline::history::DefaultHistory> =
        rustyline::Editor::with_config(rl_config)
            .map_err(|e| anyhow::anyhow!("REPL error: {}", e))?;
    rl.set_helper(Some(helper));

    loop {
        let prompt_str = format!("{}{} ({}{})> {} ", BOLD, session.soul(), GREEN, session.model(), RESET);
        let readline = rl.readline(&prompt_str);

        match readline {
            Ok(line) if !line.trim().is_empty() => {
                let _ = rl.add_history_entry(&line);
                match session.handle_input(&line).await {
                    Ok(result) if result == "bye!" => break,
                    Ok(_) => {}
                    Err(e) => eprintln!("Error: {}", e),
                }
            }
            Ok(_) => continue,
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(e) => return Err(anyhow::anyhow!("REPL error: {}", e)),
        }
    }

    println!("\nGoodbye!");
    Ok(())
}