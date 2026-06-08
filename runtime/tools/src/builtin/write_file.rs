//! Write file tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, PermissionLevel, ToolContext, ToolResult};
use protocol_interface::AliusError;

pub struct WriteFileTool;

#[async_trait]
impl AliusTool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }

    fn description(&self) -> &'static str {
        "Write content to a file. Creates the file if it doesn't exist, overwrites if it does."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to write (relative to workspace or absolute)"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["path", "content"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn requires_confirmation(&self, _args: &JsonValue) -> bool {
        true // Writing files always requires confirmation
    }

    fn confirmation_request(&self, args: &JsonValue) -> Option<crate::ConfirmationRequest> {
        let path = args["path"].as_str().unwrap_or("unknown");
        Some(crate::ConfirmationRequest {
            tool_name: self.name().to_string(),
            operation: "write file".to_string(),
            details: format!(
                "Path: {}\nContent length: {} bytes",
                path,
                args["content"].as_str().map(|s| s.len()).unwrap_or(0)
            ),
        })
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let path = args["path"]
            .as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'path' argument".to_string()))?;

        let content = args["content"]
            .as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'content' argument".to_string()))?;

        // Resolve path relative to workspace
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            ctx.workspace.join(path)
        };

        // Validate path is within workspace
        let canonical_workspace = ctx.workspace.canonicalize().map_err(AliusError::Io)?;

        // For new files, check parent directory
        let parent = full_path
            .parent()
            .ok_or_else(|| AliusError::Agent("Invalid path - no parent directory".to_string()))?;

        if parent.exists() {
            let canonical_parent = parent.canonicalize().map_err(AliusError::Io)?;

            if !canonical_parent.starts_with(&canonical_workspace) {
                return Err(AliusError::Agent(
                    "Path is outside workspace - access denied".to_string(),
                ));
            }
        } else {
            // Parent doesn't exist, check if it would be within workspace
            let relative_parent = path.rfind('/').map(|i| &path[..i]).unwrap_or(".");
            let check_path = ctx.workspace.join(relative_parent);
            if !check_path.starts_with(&canonical_workspace) {
                return Err(AliusError::Agent(
                    "Path is outside workspace - access denied".to_string(),
                ));
            }
        }

        // Write file content
        tokio::fs::write(&full_path, content)
            .await
            .map_err(AliusError::Io)?;

        Ok(ToolResult::success(format!(
            "File written: {} ({})",
            path,
            content.len()
        )))
    }
}
