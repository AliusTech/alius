//! Alius Model - LLM client and agent crate.
//!
//! This crate provides the LLM client implementation and agent loop for
//! communicating with AI providers. It supports:
//! - Streaming and non-streaming chat completions
//! - Tool calling (function calling) support
//! - Conversation management
//! - Retry logic for API failures

/// LLM client for OpenAI-compatible API endpoints.
pub mod client;

/// Conversation history management.
pub mod conversation;

/// Chat event types for streaming responses.
pub mod events;

/// Agent event types for tool calling and agent loop.
pub mod agent_events;

/// Retry logic for failed API calls.
pub mod retry;

/// Agent loop implementation with tool calling support.
pub mod agent;

// Re-export all public types for convenient access
pub use client::*;
pub use conversation::*;
pub use events::*;
pub use agent_events::*;
pub use retry::*;
pub use agent::*;
