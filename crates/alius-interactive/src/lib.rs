//! Alius Interactive - REPL and UI crate.
//!
//! This crate provides the interactive REPL (Read-Eval-Print Loop) interface
//! for the Alius CLI. It handles:
//! - User input processing and command dispatch
//! - LLM client initialization and management
//! - Agent loop integration with tool calling
//! - Session and conversation persistence
//! - Terminal UI rendering

/// Interactive REPL implementation with command handling and chat.
pub mod repl;

/// Terminal UI components (welcome screen, formatting).
pub mod ui;

// Re-export the main REPL entry point
pub use repl::run_repl;
