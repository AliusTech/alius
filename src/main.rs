use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod config;
mod error;
mod llm;
mod repl;
mod ui;

use cli::{Cli, Commands, ConfigCommands};

fn init_logging(verbose: u8) {
    let filter = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("ALIUS_LOG").unwrap_or_else(|_| filter.to_string()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();
}

fn load_settings(config_path: Option<&str>) -> error::Result<config::Settings> {
    match config_path {
        Some(path) => config::Settings::load_from_path(path),
        None => config::Settings::load(),
    }
}

#[tokio::main]
async fn main() -> error::Result<()> {
    let args = Cli::parse();

    init_logging(args.verbose);

    let settings = load_settings(args.config.as_deref())?;

    match args.command {
        None => {
            // Default: enter REPL mode
            let mut session = repl::ReplSession::new(settings);
            session.run().await?;
        }
        Some(Commands::Repl) => {
            let mut session = repl::ReplSession::new(settings);
            session.run().await?;
        }
        Some(Commands::Run { prompt, model }) => {
            let mut settings = settings;
            if let Some(m) = model {
                settings.llm.model = m;
            }

            let client = llm::client::LlmClient::new(&settings)?;
            let response = client.chat(&prompt).await?;
            println!("{}", response);
        }
        Some(Commands::Config { action }) => match action {
            ConfigCommands::Show => {
                println!("{}", toml::to_string_pretty(&settings).unwrap());
            }
            ConfigCommands::Validate => {
                println!("Configuration is valid");
            }
            ConfigCommands::Soul { role } => {
                if let Some(r) = role {
                    if config::SOUL_ROLES.contains(&r.as_str()) {
                        let mut settings = settings;
                        settings.soul = Some(config::SoulSettings { role: r.clone() });
                        settings.save_to_user_config()?;
                        println!("Soul role set to: {}", r);
                    } else {
                        println!("Invalid role. Available roles:");
                        for r in config::SOUL_ROLES {
                            println!("  - {}", r);
                        }
                    }
                } else {
                    let current_role = settings.soul.as_ref()
                        .map(|s| s.role.clone())
                        .unwrap_or_else(|| "Not set".to_string());
                    println!("Current soul role: {}", current_role);
                    println!();
                    println!("Available roles:");
                    for r in config::SOUL_ROLES {
                        println!("  - {}", r);
                    }
                    println!();
                    println!("Usage: alius config soul --role \"<role>\"");
                }
            }
        },
        Some(Commands::Version) => {
            println!("alius {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}