//! Alius Store - Session and conversation storage crate.
//!
//! This crate provides persistence for:
//! - Session metadata (session ID, model, soul role, timestamps)
//! - Conversation history (messages, system prompts)
//!
//! Uses JSON files for storage in the ~/.alius/ directory.

/// Session metadata storage (session ID, model, timestamps).
pub mod session;

/// Conversation history storage (messages, system prompts).
pub mod conversation;

/// Memory store for persistent notes and context.
pub mod memory;

// Re-export all public types for convenient access
pub use session::*;
pub use conversation::*;
