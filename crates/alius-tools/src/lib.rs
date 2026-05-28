//! Alius Tools - Built-in tool system crate.
//!
//! This crate provides the tool system for the Alius agent, including:
//! - Tool trait definition (`AliusTool`)
//! - Tool registry for managing available tools
//! - Built-in tools (file operations, shell, git, etc.)
//! - Permission and confirmation handling
//!
//! Tools are functions that the LLM can call to interact with the system.
//! Each tool has a JSON schema for its arguments and can optionally require
//! user confirmation before execution.

/// Tool registry for managing and looking up tools.
pub mod registry;

/// Tool trait and context types (`AliusTool`, `ToolContext`, `ToolResult`).
pub mod traits;

/// Built-in tool implementations (file ops, shell, git, etc.).
pub mod builtin;

/// Permission and confirmation handling for tool execution.
pub mod permission;

// Re-export all public types for convenient access
pub use registry::*;
pub use traits::{AliusTool, ToolContext, ToolResult, ConfirmationRequest};
pub use builtin::*;
pub use permission::*;
