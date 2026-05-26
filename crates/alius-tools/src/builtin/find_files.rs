//! Find files tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel};
use alius_protocol::AliusError;

pub struct FindFilesTool;

#[async_trait]
impl AliusTool for FindFilesTool {
    fn name(&self) -> &'static str {
        "find_files"
    }

    fn description(&self) -> &'static str {
        "Find files matching a pattern in the workspace. Returns list of matching file paths."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "pattern": {
                    "type": "string",
                    "description": "Glob pattern to match files (e.g. '*.rs', '**/*.toml')"
                },
                "path": {
                    "type": "string",
                    "description": "Directory to search in (default: workspace root)"
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

        let search_path = args["path"].as_str().unwrap_or("");

        // Resolve path relative to workspace
        let full_path = if search_path.is_empty() || search_path.starts_with('/') {
            ctx.workspace.clone()
        } else {
            ctx.workspace.join(search_path)
        };

        // Validate path is within workspace
        let canonical_path = full_path.canonicalize()
            .map_err(AliusError::Io)?;

        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(AliusError::Io)?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Path is outside workspace - access denied".to_string()
            ));
        }

        // Use find command for glob matching
        let output = tokio::process::Command::new("find")
            .arg(&full_path)
            .arg("-name")
            .arg(pattern)
            .arg("-type")
            .arg("f")
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to run find: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if stdout.is_empty() {
            Ok(ToolResult::success("No files found".to_string()))
        } else {
            // Convert absolute paths to relative paths
            let relative_paths: Vec<String> = stdout
                .lines()
                .filter_map(|line| {
                    let path = PathBuf::from(line);
                    path.strip_prefix(&ctx.workspace)
                        .map(|p| p.to_string_lossy().to_string())
                        .ok()
                })
                .collect();

            Ok(ToolResult::success(relative_paths.join("\n")))
        }
    }
}