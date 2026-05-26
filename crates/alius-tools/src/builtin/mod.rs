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
mod create_dir;
mod git_status;
mod git_diff;
mod http_request;
mod code_stats;
mod todo;
mod json;

pub use read_file::ReadFileTool;
pub use list_dir::ListDirTool;
pub use shell::ShellTool;
pub use write_file::WriteFileTool;
pub use edit_file::EditFileTool;
pub use search::SearchTool;
pub use find_files::FindFilesTool;
pub use move_file::MoveFileTool;
pub use delete_file::DeleteFileTool;
pub use create_dir::CreateDirTool;
pub use git_status::GitStatusTool;
pub use git_diff::GitDiffTool;
pub use http_request::HttpRequestTool;
pub use code_stats::CodeStatsTool;
pub use todo::TodoTool;
pub use json::JsonTool;

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