//! Search tool (grep)

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel};
use alius_protocol::AliusError;

pub struct SearchTool;

#[async_trait]
impl AliusTool for SearchTool {
    fn name(&self) -> &'static str {
        "search"
    }

    fn description(&self) -> &'static str {
        "Search for a pattern in files using grep-like functionality. Returns matching lines."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "The pattern to search for"
                },
                "path": {
                    "type": "string",
                    "description": "The directory to search in (default: workspace root)"
                },
                "file_pattern": {
                    "type": "string",
                    "description": "Glob pattern for files to search (default: *)"
                }
            },
            "required": ["pattern"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    async fn execute(
        &self,
        args: JsonValue,
        ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let pattern = args["pattern"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'pattern' argument".to_string()))?;

        let search_path = args["path"].as_str().unwrap_or(".");
        let file_pattern = args["file_pattern"].as_str().unwrap_or("*");

        // Resolve path relative to workspace
        let full_path = if search_path.starts_with('/') {
            PathBuf::from(search_path)
        } else {
            ctx.workspace.join(search_path)
        };

        // Validate path is within workspace
        let canonical_path = full_path.canonicalize()
            .map_err(|e| AliusError::Io(e))?;

        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(|e| AliusError::Io(e))?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Path is outside workspace - access denied".to_string()
            ));
        }

        // Use grep command for now (simple implementation)
        let output = tokio::process::Command::new("grep")
            .arg("-rn")
            .arg("--include")
            .arg(file_pattern)
            .arg(pattern)
            .current_dir(&full_path)
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to run grep: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        if output.status.success() || !stdout.is_empty() {
            let lines: Vec<&str> = stdout.lines().take(50).collect();
            let result = lines.join("\n");
            Ok(ToolResult::success(result))
        } else if !stderr.is_empty() {
            Ok(ToolResult::error(format!("Search error: {}", stderr)))
        } else {
            Ok(ToolResult::success("No matches found".to_string()))
        }
    }
}