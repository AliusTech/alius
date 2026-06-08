//! Built-in tools implementation

mod code_stats;
mod create_dir;
mod delete_file;
mod edit_file;
mod find_files;
mod git_diff;
mod git_status;
mod http_request;
mod json;
mod list_dir;
mod move_file;
mod read_file;
mod search;
mod shell;
mod todo;
mod write_file;

pub use code_stats::CodeStatsTool;
pub use create_dir::CreateDirTool;
pub use delete_file::DeleteFileTool;
pub use edit_file::EditFileTool;
pub use find_files::FindFilesTool;
pub use git_diff::GitDiffTool;
pub use git_status::GitStatusTool;
pub use http_request::HttpRequestTool;
pub use json::JsonTool;
pub use list_dir::ListDirTool;
pub use move_file::MoveFileTool;
pub use read_file::ReadFileTool;
pub use search::SearchTool;
pub use shell::ShellTool;
pub use todo::TodoTool;
pub use write_file::WriteFileTool;

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
    registry.register(CreateDirTool);
    registry.register(GitStatusTool);
    registry.register(GitDiffTool);
    registry.register(HttpRequestTool);
    registry.register(CodeStatsTool);
    registry.register(TodoTool);
    registry.register(JsonTool);
}
