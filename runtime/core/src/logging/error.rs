//! Error types for the logging subsystem.

use std::path::PathBuf;

/// Errors that can occur during logging operations.
#[derive(Debug, thiserror::Error)]
pub enum LoggingError {
    /// An I/O error occurred while writing logs.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to serialize a log record to JSON.
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// The log directory could not be found or created.
    #[error("Log directory not accessible: {0}")]
    DirectoryNotFound(PathBuf),

    /// The broadcast channel is closed.
    #[error("Log broadcast channel closed")]
    ChannelClosed,
}
