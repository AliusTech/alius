//! Native (non-WASM) tools — direct OS access via Rust std/tokio.
//!
//! These complement WASM plugin tools: shell/filesystem operations need OS
//! syscalls that the WASM sandbox cannot provide. All native tools reuse the
//! shared security primitives (Shell Gate, workspace boundary).

mod fs;
mod local_service;
mod search_code;
pub mod shell;

use crate::registry::ToolRegistry;

/// Register the built-in native tools (shell + filesystem group).
/// Panics if a native tool name is already registered — this should never
/// happen because native tools are registered first.
pub fn register_native_tools(registry: &ToolRegistry) {
    registry
        .register(shell::Shell)
        .expect("native shell tool must not conflict");
    registry
        .register(fs::ReadFile)
        .expect("native read_file tool must not conflict");
    registry
        .register(fs::WriteFile)
        .expect("native write_file tool must not conflict");
    registry
        .register(fs::ListDir)
        .expect("native list_dir tool must not conflict");
    registry
        .register(fs::EditFile)
        .expect("native edit_file tool must not conflict");
    registry
        .register(search_code::SearchCode)
        .expect("native search_code tool must not conflict");
    registry
        .register(local_service::RunLocalService)
        .expect("native run_local_service tool must not conflict");
    registry
        .register(local_service::LocalServiceStatus)
        .expect("native local_service_status tool must not conflict");
    registry
        .register(local_service::StopLocalService)
        .expect("native stop_local_service tool must not conflict");
}
