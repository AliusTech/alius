//! Alius Protocol Interface crate.
//!
//! This crate defines the stable protocol boundary used across Alius:
//! - Core Runtime protocol contract (`ProtocolEnvelope`, `CoreRequest`, `CoreEvent`)
//! - Direct Rust Protocol Interface gateway (`ProtocolInterface`)
//! - Error types (`AliusError`)
//! - Message types (`Message`, `MessageRole`)
//! - Shared types (`SessionId`, `ProviderType`, `SoulRole`, `SessionMetadata`)

/// Core Runtime protocol contract types.
pub mod core;

/// Error types for the Alius application.
pub mod error;

/// Direct Rust Protocol Interface gateway.
pub mod interface;

/// Message types for conversations (user, assistant, system, summary).
pub mod message;

/// Shared types (session IDs, provider types, soul roles, metadata).
pub mod types;

/// Test utilities (gated behind `testing` feature or `#[cfg(test)]`).
#[cfg(any(test, feature = "testing"))]
pub mod testing;

// Re-export all public types for convenient access
pub use core::*;
pub use error::*;
pub use interface::{ProtocolContext, ProtocolInterface, ProtocolRunContext};
pub use message::*;
pub use types::*;
