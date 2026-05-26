use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod config;
mod error;
mod llm;
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
            ui::welcome::render_welcome(&settings);
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
        },
        Some(Commands::Version) => {
            println!("alius {}", env!("CARGO_PKG_VERSION"));
        }
    }

    Ok(())
}