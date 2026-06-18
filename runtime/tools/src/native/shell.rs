//! Native `shell` tool — cross-platform command execution with Shell Gate.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::process::Command;

use protocol_interface::{AliusError, RuntimeMode};

use crate::permission::PermissionLevel;
use crate::shell_gate::authorizer::{authorize, ShellGateConfig, ShellGateDecision};
use crate::shell_gate::inspector::command_args;
use crate::shell_gate::{ShellCommandRequest, ShellOrigin};
use crate::traits::{AliusTool, ToolContext, ToolResult};

const DEFAULT_TIMEOUT_SECS: u64 = 120;
const MAX_OUTPUT_CHARS: usize = 20_000;

pub struct Shell;

#[async_trait]
impl AliusTool for Shell {
    fn name(&self) -> &'static str {
        "shell"
    }

    fn description(&self) -> &'static str {
        "Run a shell command (sh -c on Unix, cmd /C on Windows). AcceptEdits is workspace-scoped; BypassPermissions skips Alius gates."
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn preview_confirmation(&self, args: &Value, _mode: RuntimeMode) -> bool {
        // Callers only honor this preview when their permission strategy is AcceptEdits.
        let command = args.get("command").and_then(|v| v.as_str()).unwrap_or("");
        if command.is_empty() {
            return false;
        }
        let req = ShellCommandRequest {
            command: command.to_string(),
            args: command_args(command),
            cwd: PathBuf::new(),
            origin: ShellOrigin::LocalCli,
            workspace_root: PathBuf::new(),
        };
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        matches!(decision, ShellGateDecision::ApprovalRequired { .. })
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "The shell command to run" },
                "cwd": { "type": "string", "description": "Working directory relative to workspace. Default: workspace root" },
                "timeout": { "type": "integer", "description": format!("Timeout in seconds. Default: {DEFAULT_TIMEOUT_SECS}") }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let command = args
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if command.is_empty() {
            return Ok(ToolResult::error("command is required".to_string()));
        }
        let timeout_secs = args
            .get("timeout")
            .and_then(|v| v.as_u64())
            .unwrap_or(DEFAULT_TIMEOUT_SECS)
            .max(1);
        let cwd = if ctx.bypass_permissions() {
            match resolve_bypass_cwd(args.get("cwd").and_then(|v| v.as_str()), &ctx) {
                Ok(path) => path,
                Err(e) => return Ok(ToolResult::error(format!("invalid cwd: {e}"))),
            }
        } else {
            match resolve_cwd(args.get("cwd").and_then(|v| v.as_str()), &ctx.workspace) {
                Ok(path) => path,
                Err(e) => return Ok(ToolResult::error(format!("invalid cwd: {e}"))),
            }
        };

        if !ctx.bypass_permissions() {
            let req = ShellCommandRequest {
                command: command.clone(),
                args: command_args(&command),
                cwd: cwd.clone(),
                origin: ShellOrigin::LocalCli,
                workspace_root: ctx.workspace.clone(),
            };
            let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
            if let ShellGateDecision::Deny { reason } = decision {
                return Ok(ToolResult::error(format!("denied by Shell Gate: {reason}")));
            }
        }
        // Allow + ApprovalRequired both proceed. Plan-mode confirmation is
        // gated by preview_confirmation() at the tool_step level (Stage B).

        let mut cmd = build_command(&command);
        cmd.current_dir(&cwd);
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::piped());
        cmd.stderr(Stdio::piped());
        // env fully inherited from parent process (default).

        let child = match cmd.spawn() {
            Ok(c) => c,
            Err(e) => return Ok(ToolResult::error(format!("failed to spawn: {e}"))),
        };
        let output =
            match tokio::time::timeout(Duration::from_secs(timeout_secs), child.wait_with_output())
                .await
            {
                Ok(Ok(o)) => o,
                Ok(Err(e)) => return Ok(ToolResult::error(format!("wait failed: {e}"))),
                Err(_) => {
                    return Ok(ToolResult::error(format!(
                        "timed out after {timeout_secs}s"
                    )))
                }
            };

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit = output.status.code().unwrap_or(-1);
        let combined = format!(
            "[exit:{}]\n{}\n{}",
            exit,
            truncate_output(&stdout),
            truncate_output(&stderr)
        );
        Ok(ToolResult {
            output: combined,
            success: output.status.success(),
            metadata: Some(json!({ "exit_code": exit })),
        })
    }
}

fn resolve_bypass_cwd(cwd: Option<&str>, ctx: &ToolContext) -> Result<PathBuf, AliusError> {
    let candidate = match cwd {
        Some(c) if !c.is_empty() => {
            let path = Path::new(c);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                ctx.working_directory.join(path)
            }
        }
        _ => ctx.working_directory.clone(),
    };

    candidate
        .canonicalize()
        .map_err(|e| AliusError::Agent(format!("invalid cwd: {e}")))
}

fn resolve_cwd(cwd: Option<&str>, workspace: &Path) -> Result<PathBuf, AliusError> {
    let candidate = match cwd {
        Some(c) if !c.is_empty() => {
            let path = Path::new(c);
            // Reject absolute paths
            if path.is_absolute() {
                return Err(AliusError::Agent(
                    "cwd must be relative to workspace".to_string(),
                ));
            }
            workspace.join(c)
        }
        _ => workspace.to_path_buf(),
    };

    // Canonicalize to resolve .. and symlinks
    let canonical = candidate
        .canonicalize()
        .map_err(|e| AliusError::Agent(format!("invalid cwd: {e}")))?;

    let canonical_workspace = workspace
        .canonicalize()
        .map_err(|e| AliusError::Agent(format!("failed to canonicalize workspace: {e}")))?;

    // Verify the resolved cwd is inside workspace
    if !canonical.starts_with(&canonical_workspace) {
        return Err(AliusError::Agent(
            "cwd must be inside workspace".to_string(),
        ));
    }

    Ok(canonical)
}

#[cfg(unix)]
fn build_command(command: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(command);
    c
}

#[cfg(windows)]
fn build_command(command: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").arg(command);
    c
}

fn truncate_output(s: &str) -> String {
    if s.chars().count() <= MAX_OUTPUT_CHARS {
        return s.to_string();
    }
    let truncated: String = s.chars().take(MAX_OUTPUT_CHARS).collect();
    format!("{truncated}\n... [output truncated]")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_cwd_relative_path_ok() {
        let workspace = std::env::current_dir().unwrap();
        let result = resolve_cwd(Some("src"), &workspace);
        assert!(result.is_ok());
    }

    #[test]
    fn test_resolve_cwd_absolute_path_rejected() {
        let workspace = std::env::current_dir().unwrap();
        let result = resolve_cwd(Some("/tmp"), &workspace);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("must be relative"));
    }

    #[test]
    fn test_resolve_cwd_parent_escape_rejected() {
        let workspace = std::env::current_dir().unwrap();
        // Try to escape with ../../../
        let result = resolve_cwd(Some("../../../../../../../tmp"), &workspace);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        // The error could be either "must be inside workspace" or "invalid cwd"
        // depending on whether the path exists
        assert!(
            err_msg.contains("must be inside workspace") || err_msg.contains("invalid cwd"),
            "unexpected error: {}",
            err_msg
        );
    }

    #[test]
    fn test_resolve_cwd_empty_defaults_to_workspace() {
        let workspace = std::env::current_dir().unwrap();
        let result = resolve_cwd(None, &workspace);
        assert!(result.is_ok());
        let canonical_workspace = workspace.canonicalize().unwrap();
        assert_eq!(result.unwrap(), canonical_workspace);
    }

    #[test]
    fn test_resolve_cwd_nonexistent_path_rejected() {
        let workspace = std::env::current_dir().unwrap();
        let result = resolve_cwd(Some("nonexistent_dir_12345"), &workspace);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("invalid cwd"));
    }
}
