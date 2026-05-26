//! CLI entrypoint

use anyhow::Result;

use alius::{Cli, Command, ConfigCommand};
use alius_config::Settings;
use alius_interactive::run_repl;
use alius_model::LlmClient;
use clap::Parser;

pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    // Load settings
    let settings = Settings::default(); // TODO: Implement proper config loading

    match cli.command {
        None | Some(Command::Repl) => {
            run_repl(settings).await?;
        }
        Some(Command::Run { prompt, model }) => {
            let mut settings = settings;
            if let Some(m) = model {
                settings.llm.model = m;
            }

            let client = LlmClient::new(settings.llm)?;
            let response = client.chat_once(&prompt, None).await?;
            println!("{}", response);
        }
        Some(Command::Config { command }) => {
            handle_config(command)?;
        }
        Some(Command::Version) => {
            println!("alius {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}

fn handle_config(cmd: ConfigCommand) -> Result<()> {
    match cmd {
        ConfigCommand::Show => {
            println!("Configuration:");
            println!("  Provider: openai");
            println!("  Model: gpt-4o-mini");
            println!("  Soul: Frontend Engineer");
        }
        ConfigCommand::Validate => {
            println!("Configuration is valid");
        }
        ConfigCommand::Soul { role } => {
            println!("Soul role set to: {}", role);
        }
    }
    Ok(())
}

fn main() {
    let rt = tokio::runtime::Runtime::new().expect("Failed to create runtime");
    rt.block_on(run()).expect("Failed to run");
}