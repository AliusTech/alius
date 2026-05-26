//! Create directory tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel, ConfirmationRequest};
use alius_protocol::AliusError;

pub struct CreateDirTool;

#[async_trait]
impl AliusTool for CreateDirTool {
    fn name(&self) -> &'static str {
        "create_dir"
    }

    fn description(&self) -> &'static str {
        "Create a new directory. Creates parent directories if needed."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to create"
                }
            },
            "required": ["path"]
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
            operation: "create directory".to_string(),
            details: format!("Path: {}", args["path"].as_str().unwrap_or("?")),
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

        // Validate path is within workspace (for parent if exists)
        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(AliusError::Io)?;

        if let Some(parent) = full_path.parent() {
            if parent.exists() {
                let canonical_parent = parent.canonicalize()
                    .map_err(AliusError::Io)?;

                if !canonical_parent.starts_with(&canonical_workspace) {
                    return Err(AliusError::Agent(
                        "Path is outside workspace - access denied".to_string()
                    ));
                }
            }
        }

        // Create directory
        tokio::fs::create_dir_all(&full_path)
            .await
            .map_err(AliusError::Io)?;

        Ok(ToolResult::success(format!("Created directory: {}", path)))
    }
}