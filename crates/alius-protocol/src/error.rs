//! Error types for the Alius application.
//!
//! Defines the central error enum `AliusError` with variants for each
//! error category. Uses `thiserror` for automatic `Display` and `Error`
//! trait implementations.

use thiserror::Error;

/// Central error type for the Alius application.
///
/// All error variants are user-facing and include descriptive messages.
/// This enum is used across all Alius crates for consistent error handling.
#[derive(Error, Debug)]
pub enum AliusError {
    /// Configuration-related errors (invalid config, missing fields, file I/O).
    #[error("Configuration error: {0}")]
    Config(String),

    /// LLM API communication errors (network, parsing, rate limiting).
    #[error("LLM error: {0}")]
    Llm(String),

    /// Standard I/O errors (file operations, network sockets).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP client errors (request failures, invalid responses).
    #[error("HTTP error: {0}")]
    Http(String),

    /// Missing required configuration (e.g., API key not set).
    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    /// Agent execution errors (tool failures, reasoning errors).
    #[error("Agent error: {0}")]
    Agent(String),

    /// REPL (Read-Eval-Print Loop) errors (input parsing, display issues).
    #[error("REPL error: {0}")]
    Repl(String),

    /// Storage/persistence errors (database, file system).
    #[error("Store error: {0}")]
    Store(String),

    /// JSON serialization/deserialization errors.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
