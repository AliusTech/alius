//! Delete file tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ConfirmationRequest, PermissionLevel, ToolContext, ToolResult};
use protocol_interface::AliusError;

pub struct DeleteFileTool;

#[async_trait]
impl AliusTool for DeleteFileTool {
    fn name(&self) -> &'static str {
        "delete_file"
    }

    fn description(&self) -> &'static str {
        "Delete a file or empty directory. Requires confirmation before deletion."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file or directory to delete"
                }
            },
            "required": ["path"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn requires_confirmation(&self, _args: &JsonValue) -> bool {
        true // Deletion always requires confirmation
    }

    fn confirmation_request(&self, args: &JsonValue) -> Option<ConfirmationRequest> {
        Some(ConfirmationRequest {
            tool_name: self.name().to_string(),
            operation: "delete file".to_string(),
            details: format!("Path: {}", args["path"].as_str().unwrap_or("?")),
        })
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'path' argument".to_string()))?;

        // Resolve path relative to workspace
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            ctx.workspace.join(path)
        };

        // Validate path exists and is within workspace
        let canonical_path = full_path
            .canonicalize()
            .map_err(|e| AliusError::Agent(format!("Path not found: {}", e)))?;

        let canonical_workspace = ctx.workspace.canonicalize().map_err(AliusError::Io)?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Path is outside workspace - access denied".to_string(),
            ));
        }

        // Check if it's a file or directory
        let is_dir = full_path.is_dir();

        if is_dir {
            // Check if directory is empty
            let mut entries = tokio::fs::read_dir(&full_path)
                .await
                .map_err(AliusError::Io)?;

            if entries
                .next_entry()
                .await
                .map_err(AliusError::Io)?
                .is_some()
            {
                return Err(AliusError::Agent(
                    "Directory is not empty - cannot delete".to_string(),
                ));
            }

            tokio::fs::remove_dir(&full_path)
                .await
                .map_err(AliusError::Io)?;

            Ok(ToolResult::success(format!(
                "Deleted empty directory: {}",
                path
            )))
        } else {
            tokio::fs::remove_file(&full_path)
                .await
                .map_err(AliusError::Io)?;

            Ok(ToolResult::success(format!("Deleted file: {}", path)))
        }
    }
}
