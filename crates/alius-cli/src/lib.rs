//! Alius CLI - Command line entrypoint crate.
//!
//! This crate provides the main CLI interface for the Alius application.
//! It re-exports the CLI types from the `cli` module for use by the binary entrypoint.

/// CLI command definitions and argument parsing.
pub mod cli;

// Re-export CLI types for convenient access from main.rs
pub use cli::{Cli, Command, ConfigCommand, CoreCommand};
