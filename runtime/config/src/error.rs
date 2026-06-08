//! Configuration error types.
//!
//! This module defines all error types used by the Config Manager
//! for configuration parsing, validation, and migration.

use std::path::PathBuf;
use thiserror::Error;

/// Configuration error type.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// TOML or JSON parsing error.
    #[error("Failed to parse config file '{path}': {source}")]
    ParseError {
        /// Path to the config file.
        path: PathBuf,
        /// The underlying parse error.
        #[source]
        source: toml::de::Error,
    },

    /// JSON parsing error for MCP config.
    #[error("Failed to parse JSON config file '{path}': {source}")]
    JsonParseError {
        /// Path to the config file.
        path: PathBuf,
        /// The underlying JSON error.
        #[source]
        source: serde_json::Error,
    },

    /// TOML serialization error.
    #[error("Failed to serialize config file '{path}': {source}")]
    SerializeError {
        /// Path to the config file.
        path: PathBuf,
        /// The underlying serialization error.
        #[source]
        source: toml::ser::Error,
    },

    /// Configuration validation error.
    #[error("Config validation error for field '{field}': {message}")]
    ValidationError {
        /// The field that failed validation.
        field: String,
        /// The validation error message.
        message: String,
    },

    /// Configuration migration error.
    #[error("Failed to migrate config from '{from}' to '{to}': {message}")]
    MigrationError {
        /// Source path.
        from: PathBuf,
        /// Destination path.
        to: PathBuf,
        /// Error message.
        message: String,
    },

    /// Configuration conflict error.
    #[error("Config conflict for field '{field}': {values:?}")]
    Conflict {
        /// The conflicting field.
        field: String,
        /// The conflicting values.
        values: Vec<String>,
    },

    /// IO error.
    #[error("IO error for '{path}': {source}")]
    IoError {
        /// Path to the file or directory.
        path: PathBuf,
        /// The underlying IO error.
        #[source]
        source: std::io::Error,
    },

    /// Config file not found.
    #[error("Config file not found: {0}")]
    NotFound(PathBuf),

    /// Missing required field.
    #[error("Missing required config field: {0}")]
    MissingField(String),

    /// Invalid value for a field.
    #[error("Invalid value for field '{field}': expected {expected}, got '{actual}'")]
    InvalidValue {
        /// Field name.
        field: String,
        /// Expected value description.
        expected: String,
        /// Actual value.
        actual: String,
    },

    /// Project root not found.
    #[error("Could not find project root (no .alius directory found)")]
    ProjectRootNotFound,

    /// Environment variable error.
    #[error("Environment variable error: {0}")]
    EnvError(String),
}

impl ConfigError {
    /// Create a parse error.
    pub fn parse(path: impl Into<PathBuf>, source: toml::de::Error) -> Self {
        Self::ParseError {
            path: path.into(),
            source,
        }
    }

    /// Create a JSON parse error.
    pub fn json_parse(path: impl Into<PathBuf>, source: serde_json::Error) -> Self {
        Self::JsonParseError {
            path: path.into(),
            source,
        }
    }

    /// Create a TOML serialization error.
    pub fn serialize(path: impl Into<PathBuf>, source: toml::ser::Error) -> Self {
        Self::SerializeError {
            path: path.into(),
            source,
        }
    }

    /// Create a validation error.
    pub fn validation(field: impl Into<String>, message: impl Into<String>) -> Self {
        Self::ValidationError {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Create a migration error.
    pub fn migration(
        from: impl Into<PathBuf>,
        to: impl Into<PathBuf>,
        message: impl Into<String>,
    ) -> Self {
        Self::MigrationError {
            from: from.into(),
            to: to.into(),
            message: message.into(),
        }
    }

    /// Create an IO error.
    pub fn io(path: impl Into<PathBuf>, source: std::io::Error) -> Self {
        Self::IoError {
            path: path.into(),
            source,
        }
    }

    /// Create a not found error.
    pub fn not_found(path: impl Into<PathBuf>) -> Self {
        Self::NotFound(path.into())
    }

    /// Create a missing field error.
    pub fn missing_field(field: impl Into<String>) -> Self {
        Self::MissingField(field.into())
    }

    /// Create an invalid value error.
    pub fn invalid_value(
        field: impl Into<String>,
        expected: impl Into<String>,
        actual: impl Into<String>,
    ) -> Self {
        Self::InvalidValue {
            field: field.into(),
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

/// Result type for Config Manager operations.
pub type ConfigResult<T> = std::result::Result<T, ConfigError>;
