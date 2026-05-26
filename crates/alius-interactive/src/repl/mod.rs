//! Interactive REPL

use anyhow::Result;
use futures::StreamExt;
use std::io::Write;
use std::sync::Arc;

use alius_config::{Settings, system_prompt_for_role};
use alius_model::{LlmClient, Conversation, ChatEvent};
use alius_store::{SessionStore, ConversationStore};
use alius_protocol::SessionMetadata;

/// REPL session
pub struct ReplSession {
    settings: Arc<std::sync::RwLock<Settings>>,
    client: Option<LlmClient>,
    conversation: Conversation,
    session_metadata: SessionMetadata,
    session_store: SessionStore,
    conversation_store: ConversationStore,
}

impl ReplSession {
    /// Create a new REPL session
    pub fn new(settings: Settings) -> Result<Self> {
        let client = LlmClient::new(settings.llm.clone()).ok();
        let system_prompt = system_prompt_for_role(&settings.soul.role);
        let conversation = Conversation::new(Some(system_prompt));

        let session_metadata = SessionMetadata::new(settings.llm.model.clone());
        let session_store = SessionStore::new()?;
        let conversation_store = ConversationStore::new()?;

        Ok(Self {
            settings: Arc::new(std::sync::RwLock::new(settings)),
            client,
            conversation,
            session_metadata,
            session_store,
            conversation_store,
        })
    }

    /// Get current model
    pub fn model(&self) -> String {
        self.settings.read().unwrap().llm.model.clone()
    }

    /// Get current soul role
    pub fn soul(&self) -> String {
        self.settings.read().unwrap().soul.role.to_string()
    }

    /// Handle user input
    pub async fn handle_input(&mut self, input: &str) -> Result<String> {
        // Check for slash commands
        if input.starts_with('/') {
            return self.handle_command(input);
        }

        // Regular chat
        if let Some(client) = &self.client {
            self.conversation.add_user_message(input.to_string());

            let stream = client.chat_stream(&self.conversation).await?;
            let mut stream = Box::pin(stream);

            let mut stdout = std::io::stdout();
            let mut full_response = String::new();

            while let Some(event) = stream.next().await {
                match event? {
                    ChatEvent::Delta { text } => {
                        stdout.write_all(text.as_bytes())?;
                        stdout.flush()?;
                        full_response.push_str(&text);
                    }
                    ChatEvent::Done { .. } => break,
                    ChatEvent::Error { message } => {
                        return Err(anyhow::anyhow!("Error: {}", message));
                    }
                }
            }

            println!();

            self.conversation.add_assistant_message(full_response.clone());

            // Save conversation
            self.conversation_store.save_messages(
                &self.session_metadata.id,
                self.conversation.messages()
            )?;

            return Ok(full_response);
        }

        Err(anyhow::anyhow!("No LLM client available"))
    }

    /// Handle slash command
    fn handle_command(&mut self, input: &str) -> Result<String> {
        let parts: Vec<&str> = input.split_whitespace().collect();
        let cmd = parts.first().copied().unwrap_or("");

        match cmd {
            "/model" => {
                if parts.len() > 1 {
                    let model = parts[1];
                    self.settings.write().unwrap().llm.model = model.to_string();
                    self.client = LlmClient::new(self.settings.read().unwrap().llm.clone()).ok();
                    Ok(format!("Model switched to: {}", model))
                } else {
                    Ok("Use: /model <name>".to_string())
                }
            }
            "/soul" => {
                if parts.len() > 1 {
                    let role = parts[1..].join(" ");
                    self.settings.write().unwrap().soul.role = alius_protocol::SoulRole::new(role.clone());
                    let prompt = system_prompt_for_role(&self.settings.read().unwrap().soul.role);
                    self.conversation.set_system_prompt(prompt);
                    Ok(format!("Soul switched to: {}", role))
                } else {
                    Ok("Use: /soul <role>".to_string())
                }
            }
            "/clear" => {
                self.conversation.clear();
                Ok("Conversation cleared".to_string())
            }
            "/help" => {
                crate::ui::show_help();
                Ok(String::new())
            }
            "/quit" | "/exit" => {
                Ok("bye!".to_string())
            }
            _ => {
                Ok(format!("Unknown command: {}", cmd))
            }
        }
    }
}

/// Run the REPL
pub async fn run_repl(settings: Settings) -> Result<()> {
    let mut session = ReplSession::new(settings)?;

    crate::ui::show_welcome(&session.model(), "openai");

    let mut rl = rustyline::Editor::<(), rustyline::history::DefaultHistory>::new()?;

    let prompt = format!("{} ({})> ", session.soul(), session.model());

    loop {
        let readline = rl.readline(&prompt);

        match readline {
            Ok(line) => {
                if line.is_empty() {
                    continue;
                }

                rl.add_history_entry(&line)?;

                let result = session.handle_input(&line).await?;

                if result == "bye!" {
                    break;
                }

                if !result.is_empty() {
                    println!();
                }
            }
            Err(rustyline::error::ReadlineError::Interrupted) => {
                println!("^C");
                continue;
            }
            Err(rustyline::error::ReadlineError::Eof) => {
                println!("^D");
                break;
            }
            Err(e) => {
                return Err(anyhow::anyhow!("REPL error: {}", e));
            }
        }
    }

    Ok(())
}