pub mod native;
pub mod package;
pub mod permission;
pub mod policy;
pub mod registry;
pub mod shell_gate;
pub mod traits;
pub mod wasm_host;

/// Test utilities (gated behind `testing` feature or `#[cfg(test)]`).
#[cfg(any(test, feature = "testing"))]
pub mod testing;

#[cfg(feature = "mcp")]
pub mod mcp_bridge;

pub use package::{
    ToolHostCapability, ToolPackage, ToolPackageManifest, ToolPackageResolver, ToolRuntimeHost,
};
pub use permission::PermissionLevel;
pub use registry::ToolRegistry;
pub use traits::{AliusTool, ConfirmationRequest, ToolContext, ToolResult};
pub use wasm_host::WasmPluginTool;

use std::path::Path;

/// Discover and register all installed Rust WASM module tools.
pub fn register_installed_rust_wasm_tools(registry: &mut ToolRegistry, _workspace_root: &Path) {
    let packages = match wasm_host::list_plugins() {
        Ok(packages) => packages,
        Err(err) => {
            eprintln!("[warn] Failed to list Rust WASM tools: {err}");
            return;
        }
    };

    for package in packages {
        let wasm_bytes = match std::fs::read(&package.wasm_path) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!(
                    "[warn] Failed to read Rust WASM tool '{}': {err}",
                    package.manifest.id
                );
                continue;
            }
        };

        match WasmPluginTool::from_wasm_bytes(&wasm_bytes) {
            Ok(tools) => {
                for tool in tools {
                    if let Err(conflict) = registry.register(tool) {
                        eprintln!("[warn] {conflict} — skipping WASM tool");
                    }
                }
            }
            Err(err) => eprintln!(
                "[warn] Failed to load Rust WASM tool '{}': {err}",
                package.manifest.id
            ),
        }
    }
}
