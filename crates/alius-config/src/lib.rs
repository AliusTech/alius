//! Alius Config - Configuration management crate.
//!
//! This crate provides configuration types and loading logic for the Alius application.
//! It includes settings for LLM providers, agent behavior, and soul roles (personas).

/// Settings structures and configuration loading.
pub mod settings;

/// Soul role definitions and system prompt mapping.
pub mod soul;

// Re-export all public types for convenient access
pub use settings::*;
pub use soul::*;
