use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod cli;
mod config;
mod error;
mod llm;
mod repl;
mod ui;

use cli::{Cli, Commands, ConfigCommands};

/// Initialize the logging system based on verbosity level.
///
/// Verbosity mapping:
///   0 → warn (default, only warnings and errors)
///   1 → info (informational messages)
///   2 → debug (debug-level details)
///   3+ → trace (maximum detail)
///
/// The `ALIUS_LOG` environment variable can override the verbosity level
/// with a custom filter string (e.g., "alius=debug").
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

/// Load application settings from the specified config path or the default location.
///
/// If `config_path` is provided, settings are loaded from that file.
/// Otherwise, the default config resolution chain is used:
///   1. Embedded default config (compiled into the binary)
///   2. User config at ~/.alius/config.toml
///   3. Environment variables with `ALIUS_` prefix
fn load_settings(config_path: Option<&str>) -> error::Result<config::Settings> {
    match config_path {
        Some(path) => config::Settings::load_from_path(path),
        None => config::Settings::load(),
    }
}

/// Application entry point.
///
/// Parses CLI arguments, initializes logging, loads settings, and dispatches
/// to the appropriate command handler. If no subcommand is provided, the
/// interactive REPL mode is started by default.
#[tokio::main]
async fn main() -> error::Result<()> {
    let args = Cli::parse();

    init_logging(args.verbose);

    let settings = load_settings(args.config.as_deref())?;

    match args.command {
        // No subcommand: start interactive REPL mode (default behavior)
        None => {
            let mut session = repl::ReplSession::new(settings);
            session.run().await?;
        }
        // Explicit REPL subcommand
        Some(Commands::Repl) => {
            let mut session = repl::ReplSession::new(settings);
            session.run().await?;
        }
        // Run a single prompt in non-interactive mode
        Some(Commands::Run { prompt, model }) => {
            let mut settings = settings;
            // Override the default model if specified via --model flag
            if let Some(m) = model {
                settings.llm.model = m;
            }

            let client = llm::client::LlmClient::new(&settings)?;
            let response = client.chat(&prompt).await?;
            println!("{}", response);
        }
        // Configuration management subcommands
        Some(Commands::Config { action }) => match action {
            // Display the current merged configuration
            ConfigCommands::Show => {
                println!("{}", toml::to_string_pretty(&settings).unwrap());
            }
            // Validate the configuration file (currently a no-op placeholder)
            ConfigCommands::Validate => {
                println!("Configuration is valid");
            }
            // Get or set the soul role (agent persona)
            ConfigCommands::Soul { role } => {
                if let Some(r) = role {
                    // Set a new soul role if it's valid
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
                    // Display current soul role and available options
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
        // Display version information (resolved from git tag or Cargo.toml)
        Some(Commands::Version) => {
            println!("alius {}", env!("ALIUS_VERSION"));
        }
    }

    Ok(())
}
