//! Read file tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult};
use alius_protocol::AliusError;

pub struct ReadFileTool;

#[async_trait]
impl AliusTool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }

    fn description(&self) -> &'static str {
        "Read the contents of a file from the filesystem. Returns the file content as a string."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to read (relative to workspace or absolute)"
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
            .map_err(|e| AliusError::Io(e))?;

        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(|e| AliusError::Io(e))?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Path is outside workspace - access denied".to_string()
            ));
        }

        // Read file content
        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| AliusError::Io(e))?;

        Ok(ToolResult::success(content))
    }
}