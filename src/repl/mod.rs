use crate::config::{Settings, SOUL_ROLES};
use crate::error::Result;
use crate::llm::client::LlmClient;
use dialoguer::{theme::ColorfulTheme, Input};
use inquire::Select as InquireSelect;
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
        self.print_welcome();

        loop {
            let model_name = self.settings.read().await.llm.model.clone();
            let input = Input::<String>::with_theme(&ColorfulTheme::default())
                .with_prompt(format!(
                    "{}{}{}",
                    ANSI_BOLD,
                    model_name,
                    ANSI_RESET
                ))
                .interact_text();

            match input {
                Ok(line) if !line.is_empty() => {
                    if self.handle_command(&line).await? {
                        break;
                    }
                }
                Ok(_) => continue,
                Err(_) => break,
            }
        }

        println!("\n{}Goodbye!{}", ANSI_CYAN, ANSI_RESET);
        Ok(())
    }

    fn print_welcome(&self) {
        println!(
            "{}{}Alius REPL{} - Interactive LLM Chat",
            ANSI_BOLD, ANSI_CYAN, ANSI_RESET
        );
        println!();
        println!("{}Commands:{}",
            ANSI_YELLOW, ANSI_RESET);
        println!("  {}/model{}    - Select a model", ANSI_GREEN, ANSI_RESET);
        println!("  {}/soul{}     - Select your role", ANSI_GREEN, ANSI_RESET);
        println!("  {}/config{}   - Show current config", ANSI_GREEN, ANSI_RESET);
        println!("  {}/help{}     - Show this help", ANSI_GREEN, ANSI_RESET);
        println!("  {}/quit{}     - Exit REPL", ANSI_GREEN, ANSI_RESET);
        println!();
    }

    async fn handle_command(&mut self, input: &str) -> Result<bool> {
        let trimmed = input.trim();

        match trimmed {
            "/quit" | "/exit" => return Ok(true),
            "/model" => self.select_model().await?,
            "/soul" => self.select_soul().await?,
            "/config" => self.show_config().await?,
            "/help" => self.show_help(),
            cmd if cmd.starts_with('/') => {
                println!("{}Unknown command: {}{}", ANSI_YELLOW, cmd, ANSI_RESET);
                println!("Type {}/help{} for available commands", ANSI_GREEN, ANSI_RESET);
            }
            _ => self.chat(trimmed).await?,
        }

        Ok(false)
    }

    async fn select_model(&mut self) -> Result<()> {
        let current_model = self.settings.read().await.llm.model.clone();

        // Find index of current model
        let default_index = AVAILABLE_MODELS
            .iter()
            .position(|m| m == &current_model)
            .unwrap_or(0);

        let selection = InquireSelect::new("Select a model:", AVAILABLE_MODELS.to_vec())
            .with_starting_cursor(default_index)
            .prompt();

        match selection {
            Ok(model) => {
                let mut settings = self.settings.write().await;
                settings.llm.model = model.to_string();
                self.client = None;
                println!("{}Model changed to: {}{}", ANSI_GREEN, model, ANSI_RESET);
            }
            Err(_) => {
                println!("{}Model selection cancelled{}", ANSI_YELLOW, ANSI_RESET);
            }
        }

        Ok(())
    }

    async fn select_soul(&mut self) -> Result<()> {
        let current_role = self.settings.read().await.soul.as_ref()
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
                let mut settings = self.settings.write().await;
                settings.soul = Some(crate::config::SoulSettings { role: role.to_string() });
                println!("{}Role changed to: {}{}", ANSI_GREEN, role, ANSI_RESET);
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
        println!("  {}/model{}    - Select from available models", ANSI_GREEN, ANSI_RESET);
        println!("  {}/soul{}     - Select your role (Frontend Engineer, Operations Personnel, Backend Developer)", ANSI_GREEN, ANSI_RESET);
        println!("  {}/config{}   - Display current configuration", ANSI_GREEN, ANSI_RESET);
        println!("  {}/help{}     - Show this help message", ANSI_GREEN, ANSI_RESET);
        println!("  {}/quit{}     - Exit the REPL", ANSI_GREEN, ANSI_RESET);
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