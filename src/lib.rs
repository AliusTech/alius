/// CLI argument parsing and command definitions.
pub mod cli;

/// Configuration management (loading, saving, validation).
pub mod config;

/// Error types and result type alias.
pub mod error;

/// LLM client for communicating with AI providers.
pub mod llm;

// Re-export the Result type for convenience.
pub use error::Result;
