//! Alius error types

use thiserror::Error;

#[derive(Error, Debug)]
pub enum AliusError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("LLM error: {0}")]
    Llm(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Missing configuration: {0}")]
    MissingConfig(String),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("REPL error: {0}")]
    Repl(String),

    #[error("Store error: {0}")]
    Store(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}