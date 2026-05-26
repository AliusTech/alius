//! Git status tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel};
use alius_protocol::AliusError;

pub struct GitStatusTool;

#[async_trait]
impl AliusTool for GitStatusTool {
    fn name(&self) -> &'static str {
        "git_status"
    }

    fn description(&self) -> &'static str {
        "Show git status for the workspace. Returns current branch, staged/unstaged changes."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {}
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    async fn execute(
        &self,
        _args: JsonValue,
        ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let output = tokio::process::Command::new("git")
            .arg("status")
            .arg("--short")
            .current_dir(&ctx.workspace)
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to run git: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();

        // Get branch info
        let branch_output = tokio::process::Command::new("git")
            .arg("branch")
            .arg("--show-current")
            .current_dir(&ctx.workspace)
            .output()
            .await
            .map_err(|e| AliusError::Agent(format!("Failed to get branch: {}", e)))?;

        let branch = String::from_utf8_lossy(&branch_output.stdout).trim().to_string();

        if stdout.is_empty() {
            Ok(ToolResult::success(format!("Branch: {} (clean)", branch)))
        } else {
            Ok(ToolResult::success(format!("Branch: {}\n{}", branch, stdout)))
        }
    }
}