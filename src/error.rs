use thiserror::Error;

#[derive(Error, Debug)]
pub enum AliusError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("LLM API error: {0}")]
    Llm(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Missing required configuration: {0}")]
    MissingConfig(String),

    #[error("Agent execution error: {0}")]
    Agent(String),

    #[error("REPL error: {0}")]
    Repl(String),
}

pub type Result<T> = std::result::Result<T, AliusError>;