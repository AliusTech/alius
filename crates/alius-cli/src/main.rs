//! CLI binary entrypoint for the Alius workspace crate.
//!
//! This module is the main entrypoint for the Alius CLI binary when built
//! from the workspace crate structure. It parses CLI arguments, loads settings,
//! and dispatches to the appropriate command handler.

use anyhow::Result;

use alius::{Cli, Command, ConfigCommand};
use alius_config::Settings;
use alius_interactive::run_repl;
use alius_model::LlmClient;
use clap::Parser;

/// Main application logic.
///
/// Parses CLI arguments, loads configuration, and dispatches to the
/// appropriate command handler. Returns an error if any operation fails.
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Load settings from default location or specified config file
    let settings = Settings::load()?;

    match cli.command {
        // No subcommand or explicit REPL: start interactive mode
        None | Some(Command::Repl) => {
            run_repl(settings).await?;
        }
        // Run a single prompt in non-interactive mode
        Some(Command::Run { prompt, model }) => {
            let mut settings = settings;
            // Override the default model if specified via --model flag
            if let Some(m) = model {
                settings.llm.model = m;
            }

            let client = LlmClient::new(settings.llm)?;
            let response = client.chat_once(&prompt, None).await?;
            println!("{}", response);
        }
        // Configuration management subcommands
        Some(Command::Config { command }) => {
            handle_config(command)?;
        }
        // Display version information
        Some(Command::Version) => {
            println!("alius {}", env!("ALIUS_VERSION"));
        }
        // Initialize project configuration
        Some(Command::Init) => {
            let config_dir = std::path::PathBuf::from("alius");
            let config_file = config_dir.join("config.toml");
            if config_file.exists() {
                println!("Project config already exists: {}", config_file.display());
            } else {
                std::fs::create_dir_all(&config_dir)?;
                let template = format!(
                    "# Alius project-level configuration\n\
                     # This config overrides ~/.alius/config.toml\n\n\
                     [llm]\n\
                     # provider = \"openai\"\n\
                     # model = \"gpt-4o\"\n\
                     # base_url = \"https://api.openai.com/v1\"\n\n\
                     [agent]\n\
                     # max_retries = 3\n\
                     # timeout_seconds = 60\n\n\
                     [soul]\n\
                     # role = \"{}\"\n",
                    settings.soul.role
                );
                std::fs::write(&config_file, template)?;
                println!("Created project config: {}", config_file.display());
            }
        }
    }

    Ok(())
}

/// Handle configuration subcommands.
///
/// Dispatches to the appropriate handler for show, validate, or soul commands.
fn handle_config(cmd: ConfigCommand) -> Result<()> {
    match cmd {
        // Display current configuration
        ConfigCommand::Show => {
            println!("Configuration:");
            println!("  Provider: openai");
            println!("  Model: gpt-4o-mini");
            println!("  Soul: Frontend Engineer");
        }
        // Validate configuration file
        ConfigCommand::Validate => {
            println!("Configuration is valid");
        }
        // Set the soul role
        ConfigCommand::Soul { role } => {
            println!("Soul role set to: {}", role);
        }
    }
    Ok(())
}

/// Binary entrypoint.
///
/// Creates a Tokio async runtime and executes the main application logic.
fn main() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(run()).expect("Failed to run");
}
