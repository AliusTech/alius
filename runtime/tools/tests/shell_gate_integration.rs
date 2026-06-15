//! Integration tests for Shell Gate workspace boundary enforcement.
//!
//! These tests verify the P3-1 requirements:
//! - args are properly extracted and used in scope analysis
//! - external paths are detected: /etc/passwd, ../outside, --output=/tmp/out, > /tmp/out, 2>/tmp/err
//! - cwd is validated to be inside workspace
//! - Chat/Bypass modes respect Shell Gate (not bypassed)
//! - authorize() returns Deny for external paths
//! - Shell::execute() rejects external paths in all modes

use std::path::PathBuf;

use protocol_interface::RuntimeMode;
use runtime_tools::native::shell::Shell;
use runtime_tools::shell_gate::authorizer::{authorize, ShellGateConfig, ShellGateDecision};
use runtime_tools::shell_gate::scope::analyze_scope;
use runtime_tools::shell_gate::{ShellCommandRequest, ShellOrigin};
use runtime_tools::traits::{AliusTool, ToolContext};
use serde_json::json;

fn make_request(cwd: &str, workspace: &str, command: &str) -> ShellCommandRequest {
    ShellCommandRequest {
        command: command.to_string(),
        args: vec![],
        cwd: PathBuf::from(cwd),
        origin: ShellOrigin::LocalCli,
        workspace_root: PathBuf::from(workspace),
    }
}

fn make_tool_context(workspace: &str, mode: RuntimeMode) -> ToolContext {
    ToolContext {
        workspace: PathBuf::from(workspace),
        session_id: "test-session".to_string(),
        working_directory: PathBuf::from(workspace),
        mode,
    }
}

// ============================================================================
// Scope Analysis Tests (detection layer)
// ============================================================================

#[test]
fn test_etc_passwd_detected_as_external() {
    let mut req = make_request("/workspace", "/workspace", "cat /etc/passwd");
    req.args = vec!["/etc/passwd".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis
            .external_paths
            .contains(&PathBuf::from("/etc/passwd")),
        "Expected /etc/passwd to be detected as external path"
    );
}

#[test]
fn test_parent_escape_detected_as_external() {
    let mut req = make_request("/workspace", "/workspace", "cat ../outside/file.txt");
    req.args = vec!["../outside/file.txt".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        !analysis.external_paths.is_empty(),
        "Expected ../outside to be detected as external when escaping workspace"
    );
}

#[test]
fn test_output_flag_external_path_detected() {
    let mut req = make_request("/workspace", "/workspace", "gcc main.c --output=/tmp/out");
    req.args = vec!["main.c".to_string(), "--output=/tmp/out".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis.external_paths.contains(&PathBuf::from("/tmp/out")),
        "Expected --output=/tmp/out to be detected as external path"
    );
}

#[test]
fn test_stdout_redirection_detected() {
    let req = make_request("/workspace", "/workspace", "echo test > /tmp/out");
    let analysis = analyze_scope(&req);
    assert!(
        analysis.external_paths.contains(&PathBuf::from("/tmp/out")),
        "Expected > /tmp/out to be detected as external path"
    );
}

#[test]
fn test_stderr_redirection_detected() {
    let req = make_request("/workspace", "/workspace", "command 2>/tmp/err");
    let analysis = analyze_scope(&req);
    assert!(
        analysis.external_paths.contains(&PathBuf::from("/tmp/err")),
        "Expected 2>/tmp/err to be detected as external path"
    );
}

#[test]
fn test_cwd_outside_workspace_detected() {
    let req = make_request("/tmp", "/workspace", "ls");
    let analysis = analyze_scope(&req);
    assert!(
        !analysis.cwd_inside_workspace,
        "Expected cwd /tmp to be outside workspace"
    );
}

#[test]
fn test_args_used_when_provided() {
    let mut req = make_request("/workspace", "/workspace", "cat /etc/passwd");
    req.args = vec!["/etc/passwd".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis
            .external_paths
            .contains(&PathBuf::from("/etc/passwd")),
        "Expected explicit args to be used for scope analysis"
    );
}

#[test]
fn test_args_fallback_when_empty() {
    let mut req = make_request("/workspace", "/workspace", "cat /etc/passwd");
    req.args = vec![]; // Empty args should fallback to parsing command
    let analysis = analyze_scope(&req);
    assert!(
        analysis
            .external_paths
            .contains(&PathBuf::from("/etc/passwd")),
        "Expected fallback to command parsing when args is empty"
    );
}

#[test]
fn test_workspace_internal_paths_allowed() {
    let mut req = make_request("/workspace", "/workspace", "cat ./src/main.rs");
    req.args = vec!["./src/main.rs".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis.external_paths.is_empty(),
        "Expected workspace-internal paths to be allowed"
    );
    assert!(
        analysis.cwd_inside_workspace,
        "Expected cwd to be inside workspace"
    );
}

#[test]
fn test_multiple_external_paths_detected() {
    let mut req = make_request(
        "/workspace",
        "/workspace",
        "cp /etc/passwd /tmp/backup > /var/log/output.log",
    );
    req.args = vec!["/etc/passwd".to_string(), "/tmp/backup".to_string()];
    let analysis = analyze_scope(&req);

    // Should detect /etc/passwd, /tmp/backup from args, and /var/log/output.log from redirection
    assert!(
        analysis.external_paths.len() >= 2,
        "Expected multiple external paths to be detected"
    );
    assert!(
        analysis
            .external_paths
            .contains(&PathBuf::from("/etc/passwd")),
        "Expected /etc/passwd in external paths"
    );
    assert!(
        analysis
            .external_paths
            .contains(&PathBuf::from("/tmp/backup")),
        "Expected /tmp/backup in external paths"
    );
}

// ============================================================================
// Authorization Tests (decision layer)
// ============================================================================

#[test]
fn test_authorize_etc_passwd_denied() {
    let mut req = make_request("/workspace", "/workspace", "cat /etc/passwd");
    req.args = vec!["/etc/passwd".to_string()];
    let decision = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for /etc/passwd, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_parent_escape_denied() {
    let mut req = make_request("/workspace", "/workspace", "cat ../outside/file.txt");
    req.args = vec!["../outside/file.txt".to_string()];
    let decision = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for ../outside escape, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_output_flag_denied() {
    let mut req = make_request("/workspace", "/workspace", "gcc main.c --output=/tmp/out");
    req.args = vec!["main.c".to_string(), "--output=/tmp/out".to_string()];
    let decision = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for --output=/tmp/out, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_stdout_redirect_denied() {
    let req = make_request("/workspace", "/workspace", "echo test > /tmp/out");
    let decision = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for > /tmp/out, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_stderr_redirect_denied() {
    let req = make_request("/workspace", "/workspace", "command 2>/tmp/err");
    let decision = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for 2>/tmp/err, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_workspace_internal_allowed() {
    let mut req = make_request("/workspace", "/workspace", "cat ./src/main.rs");
    req.args = vec!["./src/main.rs".to_string()];
    let decision = authorize(&req, &ShellGateConfig::default());
    assert_eq!(
        decision,
        ShellGateDecision::Allow,
        "Expected Allow for workspace-internal path"
    );
}

#[test]
fn test_authorize_high_risk_internal_requires_approval() {
    let mut req = make_request("/workspace", "/workspace", "rm -rf ./build");
    req.args = vec!["-rf".to_string(), "./build".to_string()];
    let decision = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::ApprovalRequired { .. }),
        "Expected ApprovalRequired for high-risk workspace-internal command, got {:?}",
        decision
    );
}

// ============================================================================
// Execution Tests (Shell::execute layer)
// ============================================================================

#[tokio::test]
async fn test_execute_etc_passwd_rejected_in_chat() {
    let shell = Shell;
    // Use current dir as workspace since it exists
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
    };
    let args = json!({"command": "cat /etc/passwd"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(!tool_result.success, "Expected execution to fail");
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Expected denial message, got: {}",
        tool_result.output
    );
}

#[tokio::test]
async fn test_execute_stdout_redirect_rejected_in_chat() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
    };
    let args = json!({"command": "echo test > /tmp/out"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(!tool_result.success, "Expected execution to fail");
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Expected denial message, got: {}",
        tool_result.output
    );
}

#[tokio::test]
async fn test_execute_stderr_redirect_rejected_in_plan() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Plan,
    };
    let args = json!({"command": "command 2>/tmp/err"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(!tool_result.success, "Expected execution to fail");
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Expected denial message, got: {}",
        tool_result.output
    );
}

#[tokio::test]
async fn test_execute_output_flag_rejected_in_chat() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
    };
    let args = json!({"command": "gcc main.c --output=/tmp/out"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(!tool_result.success, "Expected execution to fail");
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Expected denial message, got: {}",
        tool_result.output
    );
}
