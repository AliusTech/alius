use clap::{Parser, Subcommand};

/// Root CLI structure for the Alius command-line tool.
///
/// Supports global flags (`--config`, `--verbose`) and an optional subcommand.
/// If no subcommand is provided, the interactive REPL mode is started.
#[derive(Parser)]
#[command(name = "alius")]
#[command(about = "LLM Agent CLI Tool", long_about = None)]
#[command(version = env!("ALIUS_VERSION"))]
pub struct Cli {
    /// Optional subcommand to execute. Defaults to REPL mode if omitted.
    #[command(subcommand)]
    pub command: Option<Commands>,

    /// Path to a custom configuration file. Overrides the default ~/.alius/config.toml.
    #[arg(short, long, global = true, value_name = "FILE")]
    pub config: Option<String>,

    /// Verbosity level. Repeat for more detail: -v (info), -vv (debug), -vvv (trace).
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Available subcommands for the Alius CLI.
#[derive(Subcommand)]
pub enum Commands {
    /// Start the interactive REPL (Read-Eval-Print Loop) mode.
    #[command(about = "Start interactive REPL mode")]
    Repl,

    /// Run a single prompt in non-interactive mode and print the response.
    #[command(about = "Run an agent task")]
    Run {
        /// The prompt text to send to the LLM.
        #[arg(short, long)]
        prompt: String,

        /// Override the default model for this run (e.g., "gpt-4o", "claude-3-5-sonnet").
        #[arg(short, long)]
        model: Option<String>,
    },

    /// Manage configuration settings (show, validate, or set soul role).
    #[command(about = "Manage configuration")]
    Config {
        /// The configuration subcommand to execute.
        #[command(subcommand)]
        action: ConfigCommands,
    },

    /// Display version information.
    #[command(about = "Show version information")]
    Version,
}

/// Subcommands for configuration management.
#[derive(Subcommand)]
pub enum ConfigCommands {
    /// Display the current merged configuration (defaults + user config + env vars).
    #[command(about = "Display current configuration")]
    Show,

    /// Validate the configuration file for correctness.
    #[command(about = "Validate configuration file")]
    Validate,

    /// Display or set the soul role (agent persona).
    ///
    /// The soul role defines the agent's behavior and expertise area.
    /// Available roles are defined in `SOUL_ROLES`.
    #[command(about = "Display or set soul role")]
    Soul {
        /// The role to set. If omitted, displays the current role and available options.
        #[arg(short, long)]
        role: Option<String>,
    },
}

/// Parse command-line arguments into the `Cli` structure.
pub fn parse() -> Cli {
    Cli::parse()
}
