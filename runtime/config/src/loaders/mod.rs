//! Configuration file loaders.
//!
//! This module provides individual loaders for each split configuration file:
//! - `config.toml`: Project metadata and runtime settings
//! - `providers.toml`: Provider and model routing configuration
//! - `model.toml`: Plan/Execute/Review model assignment
//! - `tools.toml`: Tool registration and execution policy
//! - `permissions.toml`: Permission and capability boundaries
//! - `protocol.toml`: Protocol Interface Layer configuration
//! - `soul.toml`: Agent identity and A2A Agent Card source

mod config;
mod model_assignment;
mod permissions;
mod protocol;
mod providers;
mod soul;
mod tools;

pub use config::load_config;
pub use model_assignment::{
    load_model_assignment, load_or_migrate_model_assignment, save_model_assignment,
    validate_model_assignment,
};
pub use permissions::load_permissions;
pub use protocol::load_protocol;
pub use providers::{load_providers, save_providers};
pub use soul::load_soul;
pub use tools::load_tools;

use crate::error::{ConfigError, ConfigResult};
use std::path::Path;

/// Load a TOML file and parse it.
fn load_toml<T: serde::de::DeserializeOwned>(path: &Path) -> ConfigResult<T> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::io(path, e))?;

    toml::from_str(&content).map_err(|e| ConfigError::parse(path, e))
}

/// Load a JSON file and parse it.
#[allow(dead_code)]
fn load_json<T: serde::de::DeserializeOwned>(path: &Path) -> ConfigResult<T> {
    let content = std::fs::read_to_string(path).map_err(|e| ConfigError::io(path, e))?;

    serde_json::from_str(&content).map_err(|e| ConfigError::json_parse(path, e))
}

/// Check if a file exists.
#[allow(dead_code)]
fn file_exists(path: &Path) -> bool {
    path.exists() && path.is_file()
}

/// Optional file loader - returns None if file doesn't exist.
#[allow(dead_code)]
fn load_toml_optional<T: serde::de::DeserializeOwned>(path: &Path) -> ConfigResult<Option<T>> {
    if !file_exists(path) {
        return Ok(None);
    }
    load_toml(path).map(Some)
}
