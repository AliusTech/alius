//! MCP (Model Context Protocol) implementation for Alius.
//!
//! This crate provides a complete implementation of the MCP protocol,
//! allowing Alius to connect to MCP servers and use their tools, resources, and prompts.

pub mod client;
pub mod protocol;
pub mod registry;
pub mod transport;

#[cfg(test)]
mod protocol_tests;

pub use client::McpClient;
pub use protocol::*;
pub use registry::McpRegistry;
pub use transport::{StdioTransport, Transport};
