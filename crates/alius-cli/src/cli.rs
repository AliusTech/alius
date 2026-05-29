//! CLI command definitions for the Alius workspace crate.
//!
//! This module defines the command-line interface structure using `clap` derive macros.
//! It supports global flags (model, provider, workspace, config, verbosity) and
//! subcommands (repl, run, config, version).

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Root CLI structure for the Alius command-line tool.
///
/// Supports global flags that apply to all subcommands:
/// - `--model`: Override the default LLM model
/// - `--provider`: Override the default LLM provider
/// - `--workspace`: Set the working directory context
/// - `--config`: Specify a custom configuration file
/// - `--verbose` / `-v`: Increase logging verbosity
#[derive(Parser)]
#[command(name = "alius")]
#[command(author, about, long_about = None)]
#[command(version = env!("ALIUS_VERSION"))]
pub struct Cli {
    /// Optional subcommand to execute. Defaults to REPL mode if omitted.
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Override the default LLM model (e.g., "gpt-4o", "claude-3-5-sonnet").
    #[arg(short, long)]
    pub model: Option<String>,

    /// Override the default LLM provider (e.g., "openai", "anthropic").
    #[arg(short = 'p', long)]
    pub provider: Option<String>,

    /// Set the working directory for file operations.
    #[arg(long)]
    pub workspace: Option<PathBuf>,

    /// Path to a custom configuration file. Overrides ~/.alius/config.toml.
    #[arg(short = 'c', long)]
    pub config: Option<PathBuf>,

    /// Verbosity level. Repeat for more detail: -v (info), -vv (debug), -vvv (trace).
    #[arg(short, long, action = clap::ArgAction::Count)]
    pub verbose: u8,
}

/// Available subcommands for the Alius CLI.
#[derive(Subcommand)]
pub enum Command {
    /// Start the interactive REPL (Read-Eval-Print Loop) mode.
    /// This is the default behavior when no subcommand is specified.
    Repl,

    /// Run a single prompt in non-interactive mode and print the response.
    Run {
        /// The prompt text to send to the LLM.
        #[arg(short, long)]
        prompt: String,

        /// Override the default model for this run.
        #[arg(short = 'm', long)]
        model: Option<String>,
    },

    /// Manage configuration settings (show, validate, or set soul role).
    Config {
        /// The configuration subcommand to execute.
        #[command(subcommand)]
        command: ConfigCommand,
    },

    /// Display version information (resolved from git tag or Cargo.toml).
    Version,

    /// Initialize a project-level configuration file (./alius/config.toml).
    #[command(about = "Initialize project configuration")]
    Init,

    /// Manage formula repository (alius-core).
    #[command(about = "Formula repository management")]
    Core {
        #[command(subcommand)]
        command: CoreCommand,
    },

    /// Manage Soul installation and activation.
    #[command(about = "Soul management")]
    Soul {
        #[command(subcommand)]
        command: SoulCommand,
    },

    /// Manage WASM plugins.
    #[command(about = "Plugin management")]
    Plugin {
        #[command(subcommand)]
        command: PluginCommand,
    },

    /// Manage MCP (Model Context Protocol) servers.
    #[command(about = "MCP server management")]
    Mcp {
        #[command(subcommand)]
        command: McpCommand,
    },
}

/// Subcommands for MCP server management.
#[derive(Subcommand)]
pub enum McpCommand {
    /// List configured MCP servers.
    #[command(about = "List configured MCP servers")]
    List,

    /// Start an MCP server.
    #[command(about = "Start an MCP server")]
    Start {
        /// Server name to start.
        name: String,
    },

    /// List tools offered by an MCP server.
    #[command(about = "List MCP server tools")]
    Tools {
        /// Server name to query.
        name: String,
    },
}

/// Subcommands for plugin management.
#[derive(Subcommand)]
pub enum PluginCommand {
    /// List installed plugins.
    #[command(about = "List installed plugins")]
    List,

    /// Install a plugin from a local directory.
    #[command(about = "Install a plugin")]
    Install {
        /// Path to directory containing plugin.toml + plugin.wasm.
        path: String,
    },

    /// Show details of an installed plugin.
    #[command(about = "Show plugin details")]
    Info {
        /// Plugin ID to look up.
        id: String,
    },

    /// Remove an installed plugin.
    #[command(about = "Remove a plugin")]
    Remove {
        /// Plugin ID to remove.
        id: String,
    },
}

/// Subcommands for Soul management.
#[derive(Subcommand)]
pub enum SoulCommand {
    /// List installed souls.
    #[command(about = "List installed souls")]
    List,

    /// Install a soul from the formula repository.
    #[command(about = "Install a soul")]
    Install {
        /// Soul ID to install (e.g. "coder", "researcher").
        id: String,
    },

    /// Activate a soul for the current project.
    #[command(about = "Activate a soul")]
    Use {
        /// Soul ID to activate.
        id: String,
    },

    /// Show the currently activated soul.
    #[command(about = "Show current soul")]
    Current,

    /// Remove an installed soul.
    #[command(about = "Remove a soul")]
    Remove {
        /// Soul ID to remove.
        id: String,
    },
}

/// Subcommands for formula repository management.
#[derive(Subcommand)]
pub enum CoreCommand {
    /// Clone or update the formula repository from remote.
    #[command(about = "Update formula repository")]
    Update,

    /// List available formulas (souls, plugins).
    #[command(about = "List available formulas")]
    List,

    /// Show details of a specific formula.
    #[command(about = "Show formula details")]
    Info {
        /// Formula ID to look up (e.g. "coder", "researcher").
        id: String,
    },
}

/// Subcommands for configuration management.
#[derive(Subcommand)]
pub enum ConfigCommand {
    /// Display the current merged configuration.
    Show,

    /// Validate the configuration file for correctness.
    Validate,

    /// Set the soul role (agent persona).
    ///
    /// The soul role defines the agent's behavior and expertise area.
    Soul {
        /// The role name to set (e.g., "Frontend Engineer", "Backend Developer").
        #[arg(short, long)]
        role: String,
    },
}
