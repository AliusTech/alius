//! CLI command definitions

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Alius CLI - Interactive AI assistant
#[derive(Parser)]
#[command(name = "alius")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Model to use
    #[arg(short, long)]
    pub model: Option<String>,

    /// Provider to use
    #[arg(short = 'p', long)]
    pub provider: Option<String>,

    /// Working directory
    #[arg(long)]
    pub workspace: Option<PathBuf>,

    /// Custom config file path
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Verbosity level
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Available commands
#[derive(Subcommand)]
pub enum Command {
    /// Start interactive REPL mode
    Repl,

    /// Run a single prompt and exit
    Run {
        /// Prompt to execute
        #[arg(short, long)]
        prompt: String,

        /// Model to use for this run
        #[arg(short = 'm', long)]
        model: Option<String>,
    },

    /// Configuration management
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Show version information
    Version,
}

/// Configuration subcommands
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Display current configuration
    Show,

    /// Validate configuration file
    Validate,

    /// Set the soul role
    Soul {
        /// Role name to set
        #[arg(short, long)]
        role: String,
    },
}