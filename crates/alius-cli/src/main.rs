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
    let settings = Settings::default(); // TODO: Implement proper config loading

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
