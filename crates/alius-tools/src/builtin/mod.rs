//! Built-in tools implementation

mod read_file;
mod list_dir;
mod shell;

pub use read_file::ReadFileTool;
pub use list_dir::ListDirTool;
pub use shell::ShellTool;

use crate::ToolRegistry;

/// Register all built-in tools
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    registry.register(ReadFileTool);
    registry.register(ListDirTool);
    registry.register(ShellTool);
}