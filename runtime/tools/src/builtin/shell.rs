//! Shell command tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use tokio::process::Command;

use crate::{AliusTool, ToolContext, ToolResult};
use protocol_interface::AliusError;

pub struct ShellTool;

#[async_trait]
impl AliusTool for ShellTool {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Execute a shell command in the workspace directory. Returns the command output (stdout and stderr)."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The shell command to execute"
                },
                "timeout": {
                    "type": "integer",
                    "description": "Optional timeout in seconds (default: 30)",
                    "default": 30
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let command = args["command"]
            .as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'command' argument".to_string()))?;

        let timeout_secs = args["timeout"].as_u64().unwrap_or(30);

        // Security: validate command is not obviously dangerous
        let dangerous_patterns = ["rm -rf /", "sudo", "mkfs", ":(){ :|:& };:"];
        for pattern in dangerous_patterns {
            if command.contains(pattern) {
                return Err(AliusError::Agent(format!(
                    "Command contains dangerous pattern: {}",
                    pattern
                )));
            }
        }

        // Execute command in workspace directory
        let output = tokio::time::timeout(
            std::time::Duration::from_secs(timeout_secs),
            Command::new("sh")
                .arg("-c")
                .arg(command)
                .current_dir(&ctx.workspace)
                .output(),
        )
        .await
        .map_err(|_| {
            AliusError::Agent(format!("Command timed out after {} seconds", timeout_secs))
        })?
        .map_err(|e| AliusError::Agent(format!("Failed to execute command: {}", e)))?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code().unwrap_or(-1);

        let result = if output.status.success() {
            ToolResult::success(stdout)
        } else {
            ToolResult::error(format!(
                "Command failed with exit code {}\nstdout: {}\nstderr: {}",
                exit_code, stdout, stderr
            ))
        };

        Ok(result)
    }
}
