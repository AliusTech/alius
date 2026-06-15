//! Native filesystem tools — read/write/list/edit, workspace-bound.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::{json, Value};

use protocol_interface::{AliusError, RuntimeMode};

use crate::permission::PermissionLevel;
use crate::traits::{AliusTool, ToolContext, ToolResult};

/// Resolve `path` (relative to workspace) and verify it stays inside the
/// workspace after canonicalization. Defeats `../` traversal, symlink escape,
/// and absolute-path injection. `must_exist` controls whether the target itself
/// must canonicalize (read) or only its parent (write).
fn resolve_within_workspace(
    path: &str,
    workspace: &Path,
    must_exist: bool,
) -> Result<PathBuf, String> {
    let p = Path::new(path);
    if p.is_absolute() {
        return Err("absolute paths are not allowed; use a path relative to workspace".into());
    }
    let abs = workspace.join(path);
    let ws_canon = workspace
        .canonicalize()
        .map_err(|e| format!("workspace not accessible: {e}"))?;
    let canon = if must_exist {
        abs.canonicalize()
            .map_err(|e| format!("cannot access '{path}': {e}"))?
    } else {
        let parent = abs.parent().unwrap_or(workspace);
        let canon_parent = parent
            .canonicalize()
            .map_err(|e| format!("cannot access parent of '{path}': {e}"))?;
        let name = abs.file_name().ok_or_else(|| "invalid path".to_string())?;
        canon_parent.join(name)
    };
    if !canon.starts_with(&ws_canon) {
        return Err(format!("path '{path}' resolves outside workspace"));
    }
    Ok(canon)
}

pub struct ReadFile;
pub struct WriteFile;
pub struct ListDir;
pub struct EditFile;

#[async_trait]
impl AliusTool for ReadFile {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read a text file from the workspace."
    }
    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Path relative to workspace" } },
            "required": ["path"]
        })
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("path is required".into())),
        };
        let resolved = match resolve_within_workspace(path, &ctx.workspace, true) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };
        match tokio::fs::read_to_string(&resolved).await {
            Ok(content) => Ok(ToolResult::success(content)),
            Err(e) => Ok(ToolResult::error(format!("read failed: {e}"))),
        }
    }
}

#[async_trait]
impl AliusTool for WriteFile {
    fn name(&self) -> &'static str {
        "write_file"
    }
    fn description(&self) -> &'static str {
        "Write text content to a workspace file (overwrites). Requires approval in Plan mode."
    }
    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn preview_confirmation(&self, _args: &Value, mode: RuntimeMode) -> bool {
        // File write/edit requires confirmation in Plan mode (Stage B pauses here).
        mode == RuntimeMode::Plan
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "content": { "type": "string" }
            },
            "required": ["path", "content"]
        })
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("path is required".into())),
        };
        let content = match args.get("content").and_then(|v| v.as_str()) {
            Some(c) => c,
            None => return Ok(ToolResult::error("content is required".into())),
        };
        let resolved = match resolve_within_workspace(path, &ctx.workspace, false) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };
        match tokio::fs::write(&resolved, content).await {
            Ok(_) => Ok(ToolResult::success(format!("wrote {path}"))),
            Err(e) => Ok(ToolResult::error(format!("write failed: {e}"))),
        }
    }
}

#[async_trait]
impl AliusTool for ListDir {
    fn name(&self) -> &'static str {
        "list_dir"
    }
    fn description(&self) -> &'static str {
        "List entries in a workspace directory."
    }
    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": { "path": { "type": "string", "description": "Directory relative to workspace. Default: workspace root" } }
        })
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
        let resolved = match resolve_within_workspace(path, &ctx.workspace, true) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };
        let mut entries = match tokio::fs::read_dir(&resolved).await {
            Ok(d) => d,
            Err(e) => return Ok(ToolResult::error(format!("list failed: {e}"))),
        };
        let mut names = Vec::new();
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            let kind = if entry.path().is_dir() { "dir" } else { "file" };
            names.push(format!("{kind}\t{name}"));
        }
        names.sort();
        Ok(ToolResult::success(names.join("\n")))
    }
}

#[async_trait]
impl AliusTool for EditFile {
    fn name(&self) -> &'static str {
        "edit_file"
    }
    fn description(&self) -> &'static str {
        "Replace all occurrences of `find` with `replace` in a workspace file. Requires approval in Plan mode."
    }
    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn preview_confirmation(&self, _args: &Value, mode: RuntimeMode) -> bool {
        // File write/edit requires confirmation in Plan mode (Stage B pauses here).
        mode == RuntimeMode::Plan
    }
    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": { "type": "string" },
                "find": { "type": "string" },
                "replace": { "type": "string" }
            },
            "required": ["path", "find", "replace"]
        })
    }
    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let path = match args.get("path").and_then(|v| v.as_str()) {
            Some(p) => p,
            None => return Ok(ToolResult::error("path is required".into())),
        };
        let find = args.get("find").and_then(|v| v.as_str()).unwrap_or("");
        let replace = args.get("replace").and_then(|v| v.as_str()).unwrap_or("");
        let resolved = match resolve_within_workspace(path, &ctx.workspace, true) {
            Ok(p) => p,
            Err(e) => return Ok(ToolResult::error(e)),
        };
        let content = match tokio::fs::read_to_string(&resolved).await {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(format!("read failed: {e}"))),
        };
        let count = content.matches(find).count();
        if find.is_empty() {
            return Ok(ToolResult::error("find is empty".into()));
        }
        let new_content = content.replace(find, replace);
        match tokio::fs::write(&resolved, &new_content).await {
            Ok(_) => Ok(ToolResult::success(format!(
                "replaced {count} occurrence(s) in {path}"
            ))),
            Err(e) => Ok(ToolResult::error(format!("write failed: {e}"))),
        }
    }
}
