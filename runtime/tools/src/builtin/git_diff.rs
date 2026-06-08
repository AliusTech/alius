//! Git diff tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::{AliusTool, PermissionLevel, ToolContext, ToolResult};
use protocol_interface::AliusError;

pub struct GitDiffTool;

#[async_trait]
impl AliusTool for GitDiffTool {
    fn name(&self) -> &'static str {
        "git_diff"
    }

    fn description(&self) -> &'static str {
        "Show git diff for changed files. Shows unstaged changes by default."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "staged": {
                    "type": "boolean",
                    "description": "Show staged changes instead of unstaged",
                    "default": false
                },
                "file": {
                    "type": "string",
                    "description": "Specific file to show diff for"
                }
            }
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let staged = args["staged"].as_bool().unwrap_or(false);
        let file = args["file"].as_str();

        let mut cmd = tokio::process::Command::new("git");
        cmd.arg("diff");

        if staged {
            cmd.arg("--staged");
        }

        if let Some(f) = file {
            cmd.arg(f);
        }

        cmd.current_dir(&ctx.workspace);

        let output = cmd
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to run git diff: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        if stdout.is_empty() {
            Ok(ToolResult::success("No changes".to_string()))
        } else {
            // Truncate if too long
            if stdout.len() > 5000 {
                Ok(ToolResult::success(format!(
                    "{}... (truncated)",
                    &stdout[..5000]
                )))
            } else {
                Ok(ToolResult::success(stdout))
            }
        }
    }
}
