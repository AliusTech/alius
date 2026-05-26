//! Built-in tools implementation

mod read_file;
mod list_dir;
mod shell;
mod write_file;
mod edit_file;
mod search;
mod find_files;
mod move_file;
mod delete_file;

pub use read_file::ReadFileTool;
pub use list_dir::ListDirTool;
pub use shell::ShellTool;
pub use write_file::WriteFileTool;
pub use edit_file::EditFileTool;
pub use search::SearchTool;
pub use find_files::FindFilesTool;
pub use move_file::MoveFileTool;
pub use delete_file::DeleteFileTool;

use crate::ToolRegistry;

/// Register all built-in tools
pub fn register_builtin_tools(registry: &mut ToolRegistry) {
    registry.register(ReadFileTool);
    registry.register(ListDirTool);
    registry.register(ShellTool);
    registry.register(WriteFileTool);
    registry.register(EditFileTool);
    registry.register(SearchTool);
    registry.register(FindFilesTool);
    registry.register(MoveFileTool);
    registry.register(DeleteFileTool);
}