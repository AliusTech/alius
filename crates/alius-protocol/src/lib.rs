//! Alius Protocol - Shared types and protocols crate.
//!
//! This crate defines the core data types used across all Alius crates:
//! - Error types (`AliusError`)
//! - Message types (`Message`, `MessageRole`)
//! - Shared types (`SessionId`, `ProviderType`, `SoulRole`, `SessionMetadata`)

/// Error types for the Alius application.
pub mod error;

/// Message types for conversations (user, assistant, system, summary).
pub mod message;

/// Shared types (session IDs, provider types, soul roles, metadata).
pub mod types;

// Re-export all public types for convenient access
pub use error::*;
pub use message::*;
pub use types::*;
