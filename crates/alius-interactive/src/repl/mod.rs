//! Interactive REPL

use anyhow::Result;
use std::sync::Arc;
use std::io::Write;

use alius_config::{Settings, system_prompt_for_role};
use alius_model::{LlmClient, Conversation, AliusAgent, AgentEvent};
use alius_tools::{ToolRegistry, register_builtin_tools};
use alius_store::{SessionStore, ConversationStore};
use alius_protocol::SessionMetadata;

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
}

impl ReplSession {
    /// Create a new REPL session
    pub fn new(settings: Settings) -> Result<Self> {
        let client = LlmClient::new(settings.llm.clone()).ok().map(Arc::new);

        // Create tool registry with built-in tools
        let mut registry = ToolRegistry::new();
        register_builtin_tools(&mut registry);
        let registry = Arc::new(registry);

        // Create agent if client is available
        let agent = client.as_ref().map(|c| {
            AliusAgent::new(c.clone(), registry.clone(), settings.clone())
        });

        let system_prompt = system_prompt_for_role(&settings.soul.role);
        let conversation = Conversation::new(Some(system_prompt));

        let session_metadata = SessionMetadata::new(settings.llm.model.clone());
        let session_store = SessionStore::new()?;
        let conversation_store = ConversationStore::new()?;
        let workspace = std::env::current_dir()?;

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

        // Regular chat with agent
        if let Some(agent) = &self.agent {
            let events = agent.handle_message(
                &mut self.conversation,
                input.to_string(),
                self.workspace.clone(),
                self.session_metadata.id.to_string()
            ).await;

            // Process and display events
            let mut stdout = std::io::stdout();
            let mut full_response = String::new();

            for event in events {
                match event {
                    AgentEvent::TurnStarted => {
                        // Silent
                    }
                    AgentEvent::ModelStarted => {
                        // Silent
                    }
                    AgentEvent::ModelDelta { text } => {
                        stdout.write_all(text.as_bytes())?;
                        stdout.flush()?;
                        full_response.push_str(&text);
                    }
                    AgentEvent::ModelFinished { .. } => {
                        println!();
                    }
                    AgentEvent::ToolCallStarted { id, name, args } => {
                        println!("\n🔧 Tool: {} ({})", name, id);
                        if !args.is_null() {
                            println!("   Args: {}", serde_json::to_string_pretty(&args).unwrap_or_default());
                        }
                    }
                    AgentEvent::ToolCallFinished { name, success, result, .. } => {
                        let status = if success { "✅" } else { "❌" };
                        println!("{} {} done", status, name);
                        if result.len() > 200 {
                            println!("   Result: {}...", &result[..200]);
                        } else {
                            println!("   Result: {}", result);
                        }
                    }
                    AgentEvent::TurnFinished => {
                        // Silent
                    }
                    AgentEvent::Error { message } => {
                        println!("\n❌ Error: {}", message);
                        return Err(anyhow::anyhow!("Error: {}", message));
                    }
                }
            }

            // Save conversation
            self.conversation_store.save_messages(
                &self.session_metadata.id,
                self.conversation.messages()
            )?;

            return Ok(full_response);
        }

        Err(anyhow::anyhow!("No agent available"))
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
                    self.client = LlmClient::new(self.settings.read().unwrap().llm.clone()).ok().map(Arc::new);
                    self.agent = self.client.as_ref().map(|c| {
                        AliusAgent::new(c.clone(), self.registry.clone(), self.settings.read().unwrap().clone())
                    });
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
            "/tools" => {
                let tools = self.registry.list_names();
                Ok(format!("Available tools: {}", tools.join(", ")))
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
    rl.set_helper(Some(()));

    loop {
        let prompt = format!("{} ({})> ", session.soul(), session.model());
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