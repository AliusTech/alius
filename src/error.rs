use thiserror::Error;

/// Central error type for the Alius application.
///
/// All error variants are user-facing and include descriptive messages.
/// Uses `thiserror` for automatic `Display` and `Error` trait implementations.
#[derive(Error, Debug)]
pub enum AliusError {
    /// Configuration-related errors (invalid config, missing fields, file I/O).
    #[error("Configuration error: {0}")]
    Config(String),

    /// LLM API communication errors (network, parsing, rate limiting).
    #[error("LLM API error: {0}")]
    Llm(String),

    /// Standard I/O errors (file operations, network sockets).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// HTTP client errors (request failures, invalid responses).
    #[error("HTTP error: {0}")]
    Http(String),

    /// Missing required configuration (e.g., API key not set).
    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    /// Agent execution errors (tool failures, reasoning errors).
    #[error("Agent execution error: {0}")]
    Agent(String),

    /// REPL (Read-Eval-Print Loop) errors (input parsing, display issues).
    #[error("REPL error: {0}")]
    Repl(String),
}

/// Type alias for `Result<T, AliusError>` used throughout the application.
pub type Result<T> = std::result::Result<T, AliusError>;
