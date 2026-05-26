//! Move/rename file tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel, ConfirmationRequest};
use alius_protocol::AliusError;

pub struct MoveFileTool;

#[async_trait]
impl AliusTool for MoveFileTool {
    fn name(&self) -> &'static str {
        "move_file"
    }

    fn description(&self) -> &'static str {
        "Move or rename a file. Creates the destination directory if needed."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "source": {
                    "type": "string",
                    "description": "The source file path"
                },
                "destination": {
                    "type": "string",
                    "description": "The destination file path"
                }
            },
            "required": ["source", "destination"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn requires_confirmation(&self, _args: &JsonValue) -> bool {
        true
    }

    fn confirmation_request(&self, args: &JsonValue) -> Option<ConfirmationRequest> {
        Some(ConfirmationRequest {
            tool_name: self.name().to_string(),
            operation: "move file".to_string(),
            details: format!("{} -> {}",
                args["source"].as_str().unwrap_or("?"),
                args["destination"].as_str().unwrap_or("?")),
        })
    }

    async fn execute(
        &self,
        args: JsonValue,
        ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let source = args["source"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'source' argument".to_string()))?;

        let destination = args["destination"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'destination' argument".to_string()))?;

        // Resolve paths relative to workspace
        let source_path = if source.starts_with('/') {
            PathBuf::from(source)
        } else {
            ctx.workspace.join(source)
        };

        let dest_path = if destination.starts_with('/') {
            PathBuf::from(destination)
        } else {
            ctx.workspace.join(destination)
        };

        // Validate source exists and is within workspace
        let canonical_source = source_path.canonicalize()
            .map_err(|e| AliusError::Agent(format!("Source file not found: {}", e)))?;

        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(|e| AliusError::Io(e))?;

        if !canonical_source.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Source path is outside workspace - access denied".to_string()
            ));
        }

        // Create destination directory if needed
        if let Some(parent) = dest_path.parent() {
            if !parent.exists() {
                tokio::fs::create_dir_all(parent)
                    .await
                    .map_err(|e| AliusError::Io(e))?;
            }
        }

        // Move file
        tokio::fs::rename(&source_path, &dest_path)
            .await
            .map_err(|e| AliusError::Io(e))?;

        Ok(ToolResult::success(format!("Moved: {} -> {}", source, destination)))
    }
}