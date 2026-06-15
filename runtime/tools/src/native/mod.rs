//! Native (non-WASM) tools — direct OS access via Rust std/tokio.
//!
//! These complement WASM plugin tools: shell/filesystem operations need OS
//! syscalls that the WASM sandbox cannot provide. All native tools reuse the
//! shared security primitives (Shell Gate, workspace boundary).

mod fs;
pub mod shell;

use crate::registry::ToolRegistry;

/// Register the built-in native tools (shell + filesystem group).
pub fn register_native_tools(registry: &mut ToolRegistry) {
    registry.register(shell::Shell);
    registry.register(fs::ReadFile);
    registry.register(fs::WriteFile);
    registry.register(fs::ListDir);
    registry.register(fs::EditFile);
}
