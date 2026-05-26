use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "alius")]
#[command(about = "LLM Agent CLI Tool", long_about = None)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short, long, global = true, value_name = "FILE")]
    pub config: Option<String>,

    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

#[derive(Subcommand)]
pub enum Commands {
    #[command(about = "Run an agent task")]
    Run {
        #[arg(short, long)]
        prompt: String,

        #[arg(short, long)]
        model: Option<String>,
    },

    #[command(about = "Manage configuration")]
    Config {
        #[command(subcommand)]
        action: ConfigCommands,
    },

    #[command(about = "Show version information")]
    Version,
}

#[derive(Subcommand)]
pub enum ConfigCommands {
    #[command(about = "Display current configuration")]
    Show,

    #[command(about = "Validate configuration file")]
    Validate,
}

pub fn parse() -> Cli {
    Cli::parse()
}