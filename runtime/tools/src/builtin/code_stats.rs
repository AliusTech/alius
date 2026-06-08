//! Code analysis tool - count lines of code

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, PermissionLevel, ToolContext, ToolResult};
use protocol_interface::AliusError;

pub struct CodeStatsTool;

#[async_trait]
impl AliusTool for CodeStatsTool {
    fn name(&self) -> &'static str {
        "code_stats"
    }

    fn description(&self) -> &'static str {
        "Count lines of code, files, and get code statistics for a directory."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Directory to analyze (default: workspace root)"
                },
                "extensions": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "File extensions to include (e.g. ['rs', 'toml'])"
                }
            }
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let search_path = args["path"].as_str().unwrap_or(".");
        let extensions: Vec<&str> = args["extensions"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|v| v.as_str()).collect())
            .unwrap_or_else(|| vec!["rs", "toml", "md", "json", "yaml", "yml"]);

        // Resolve path
        let full_path = if search_path.starts_with('/') {
            PathBuf::from(search_path)
        } else {
            ctx.workspace.join(search_path)
        };

        // Validate path
        let canonical_path = full_path.canonicalize().map_err(AliusError::Io)?;
        let canonical_workspace = ctx.workspace.canonicalize().map_err(AliusError::Io)?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent("Path outside workspace".to_string()));
        }

        // Use tokei if available, otherwise use find + wc
        let ext_filter = extensions.join(",");
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "find '{}' -type f \\( {} \\) -exec wc -l {} + 2>/dev/null | tail -1",
                full_path.display(),
                extensions
                    .iter()
                    .map(|e| format!("-name '*.{e}'"))
                    .collect::<Vec<_>>()
                    .join(" -o "),
                ""
            ))
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to count: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // Count files
        let file_count_output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(format!(
                "find '{}' -type f \\( {} \\) | wc -l",
                full_path.display(),
                extensions
                    .iter()
                    .map(|e| format!("-name '*.{e}'"))
                    .collect::<Vec<_>>()
                    .join(" -o ")
            ))
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to count files: {}", e)))?;

        let file_count = String::from_utf8_lossy(&file_count_output.stdout)
            .trim()
            .to_string();

        // Parse total lines from wc output (last column)
        let total_lines = stdout.split_whitespace().last().unwrap_or("0");

        Ok(ToolResult::success(format!(
            "Files: {}\nLines: {}\nExtensions: {}",
            file_count, total_lines, ext_filter
        )))
    }
}
