//! List directory tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult};
use alius_protocol::AliusError;

pub struct ListDirTool;

#[async_trait]
impl AliusTool for ListDirTool {
    fn name(&self) -> &'static str {
        "list_dir"
    }

    fn description(&self) -> &'static str {
        "List the contents of a directory. Returns a list of files and directories with their names and types."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the directory to list (relative to workspace or absolute)"
                }
            },
            "required": ["path"]
        })
    }

    async fn execute(
        &self,
        args: JsonValue,
        ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let path = args["path"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'path' argument".to_string()))?;

        // Resolve path relative to workspace
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            ctx.workspace.join(path)
        };

        // Validate path is within workspace (security check)
        let canonical_path = full_path.canonicalize()
            .map_err(AliusError::Io)?;

        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(AliusError::Io)?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Path is outside workspace - access denied".to_string()
            ));
        }

        // Read directory entries
        let mut entries = Vec::new();
        let mut dir = tokio::fs::read_dir(&full_path)
            .await
            .map_err(AliusError::Io)?;

        while let Some(entry) = dir.next_entry().await.map_err(AliusError::Io)? {
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().await.map_err(AliusError::Io)?.is_dir();
            entries.push(serde_json::json!({
                "name": name,
                "type": if is_dir { "directory" } else { "file" }
            }));
        }

        // Sort entries: directories first, then files
        entries.sort_by(|a, b| {
            let a_type = a["type"].as_str().unwrap_or("");
            let b_type = b["type"].as_str().unwrap_or("");
            let a_name = a["name"].as_str().unwrap_or("");
            let b_name = b["name"].as_str().unwrap_or("");
            match (a_type, b_type) {
                ("directory", "file") => std::cmp::Ordering::Less,
                ("file", "directory") => std::cmp::Ordering::Greater,
                _ => a_name.cmp(b_name),
            }
        });

        let result = serde_json::json!({
            "path": path,
            "entries": entries
        });

        Ok(ToolResult::success(result.to_string()))
    }
}