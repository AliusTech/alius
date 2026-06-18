//! Integration tests for Shell Gate workspace boundary enforcement.
//!
//! These tests verify the P3-1 requirements:
//! - args are properly extracted and used in scope analysis
//! - external paths are detected: /etc/passwd, ../outside, --output=/tmp/out, > /tmp/out, 2>/tmp/err
//! - cwd is validated to be inside workspace
//! - Chat mode respects Shell Gate
//! - Bypass permission strategy skips Alius Shell Gate and cwd boundary checks
//! - authorize() returns Deny for external paths
//! - Shell::execute() rejects external paths unless BypassPermissions is explicitly set

use std::path::PathBuf;

use protocol_interface::{PermissionStrategy, RuntimeMode};
use runtime_tools::native::shell::Shell;
use runtime_tools::shell_gate::authorizer::{authorize, ShellGateConfig, ShellGateDecision};
use runtime_tools::shell_gate::scope::analyze_scope;
use runtime_tools::shell_gate::{ShellCommandRequest, ShellOrigin};
use runtime_tools::traits::{AliusTool, ToolContext};
use serde_json::json;
use tempfile::TempDir;

fn make_request(cwd: &str, workspace: &str, command: &str) -> ShellCommandRequest {
    ShellCommandRequest {
        command: command.to_string(),
        args: vec![],
        cwd: PathBuf::from(cwd),
        origin: ShellOrigin::LocalCli,
        workspace_root: PathBuf::from(workspace),
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
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
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
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
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
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for --output=/tmp/out, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_stdout_redirect_denied() {
    let req = make_request("/workspace", "/workspace", "echo test > /tmp/out");
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for > /tmp/out, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_stderr_redirect_denied() {
    let req = make_request("/workspace", "/workspace", "command 2>/tmp/err");
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
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
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
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
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::ApprovalRequired { .. }),
        "Expected ApprovalRequired for high-risk workspace-internal command, got {:?}",
        decision
    );
}

#[test]
fn test_authorize_high_risk_external_denied() {
    let mut req = make_request("/workspace", "/workspace", "rm -rf /tmp/foo");
    req.args = vec!["-rf".to_string(), "/tmp/foo".to_string()];
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for high-risk external path (rm -rf /tmp/foo), got {:?}",
        decision
    );
}

#[test]
fn test_authorize_critical_risk_external_denied() {
    let mut req = make_request("/workspace", "/workspace", "sudo rm -rf /etc");
    req.args = vec!["rm".to_string(), "-rf".to_string(), "/etc".to_string()];
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
    assert!(
        matches!(decision, ShellGateDecision::Deny { .. }),
        "Expected Deny for critical-risk external path (sudo rm -rf /etc), got {:?}",
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
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
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
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
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
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
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
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
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

#[tokio::test]
async fn test_execute_high_risk_external_rejected_in_chat() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };
    let args = json!({"command": "rm -rf /tmp/foo"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(!tool_result.success, "Expected execution to fail");
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Expected denial message for high-risk external path, got: {}",
        tool_result.output
    );
}

#[tokio::test]
async fn test_execute_critical_risk_external_rejected_in_plan() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Plan,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };
    let args = json!({"command": "sudo rm -rf /etc"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(!tool_result.success, "Expected execution to fail");
    assert!(
        tool_result.output.contains("denied by Shell Gate"),
        "Expected denial message for critical-risk external path, got: {}",
        tool_result.output
    );
}

// ============================================================================
// Low-risk success tests
// ============================================================================

#[tokio::test]
async fn test_execute_low_risk_ls_succeeds_in_chat() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };
    let args = json!({"command": "ls"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(
        tool_result.success,
        "ls inside workspace should succeed in Chat mode"
    );
}

#[tokio::test]
async fn test_execute_low_risk_echo_succeeds_in_chat() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };
    let args = json!({"command": "echo hello"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(tool_result.success, "echo should succeed in Chat mode");
    assert!(
        tool_result.output.contains("hello"),
        "echo output should contain 'hello'"
    );
}

// ============================================================================
// Plan mode low-risk tests
// ============================================================================

#[tokio::test]
async fn test_execute_low_risk_ls_succeeds_in_plan() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Plan,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };
    let args = json!({"command": "ls"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(
        tool_result.success,
        "ls inside workspace should succeed in Plan mode"
    );
}

// ============================================================================
// Authorization: low-risk workspace-internal
// ============================================================================

#[test]
fn test_authorize_low_risk_internal_allows() {
    let req = make_request("/workspace", "/workspace", "ls");
    let config = ShellGateConfig::default();
    let (decision, _risk) = authorize(&req, &config);
    assert!(
        matches!(decision, ShellGateDecision::Allow),
        "Low-risk workspace-internal command should be allowed, got: {:?}",
        decision
    );
}

#[test]
fn test_authorize_echo_internal_allows() {
    let req = make_request("/workspace", "/workspace", "echo hello");
    let config = ShellGateConfig::default();
    let (decision, _risk) = authorize(&req, &config);
    assert!(
        matches!(decision, ShellGateDecision::Allow),
        "Echo inside workspace should be allowed, got: {:?}",
        decision
    );
}

// ============================================================================
// Medium-risk tests
// ============================================================================

#[test]
fn test_authorize_git_status_is_low_risk() {
    let req = make_request("/workspace", "/workspace", "git status");
    let config = ShellGateConfig::default();
    let (decision, risk) = authorize(&req, &config);
    assert!(
        matches!(risk, runtime_tools::shell_gate::inspector::RiskLevel::Low),
        "git status should be low risk, got: {:?}",
        risk
    );
    assert!(
        matches!(decision, ShellGateDecision::Allow),
        "git status inside workspace should be allowed, got: {:?}",
        decision
    );
}

#[test]
fn test_authorize_git_push_is_medium_risk() {
    let req = make_request("/workspace", "/workspace", "git push");
    let config = ShellGateConfig::default();
    let (decision, risk) = authorize(&req, &config);
    assert!(
        matches!(
            risk,
            runtime_tools::shell_gate::inspector::RiskLevel::Medium
        ),
        "git push should be medium risk, got: {:?}",
        risk
    );
    assert!(
        matches!(decision, ShellGateDecision::Allow),
        "git push inside workspace should be allowed, got: {:?}",
        decision
    );
}

#[test]
fn test_authorize_mkdir_internal_is_medium_risk() {
    let req = make_request("/workspace", "/workspace", "mkdir newdir");
    let config = ShellGateConfig::default();
    let (decision, risk) = authorize(&req, &config);
    assert!(
        matches!(
            risk,
            runtime_tools::shell_gate::inspector::RiskLevel::Medium
        ),
        "mkdir should be medium risk, got: {:?}",
        risk
    );
    assert!(
        matches!(decision, ShellGateDecision::Allow),
        "mkdir inside workspace should be allowed, got: {:?}",
        decision
    );
}

#[tokio::test]
async fn test_execute_git_status_succeeds_in_chat() {
    let shell = Shell;
    let workspace = std::env::current_dir().unwrap();
    let ctx = ToolContext {
        workspace: workspace.clone(),
        session_id: "test-session".to_string(),
        working_directory: workspace,
        mode: RuntimeMode::Chat,
        permission_strategy: protocol_interface::PermissionStrategy::AcceptEdits,
    };
    let args = json!({"command": "git status"});

    let result = shell.execute(args, ctx).await;
    assert!(result.is_ok());
    let tool_result = result.unwrap();
    assert!(
        tool_result.success,
        "git status should succeed in Chat mode"
    );
}

#[tokio::test]
async fn test_execute_allows_external_cwd_with_bypass_permissions() {
    let shell = Shell;
    let workspace = TempDir::new().unwrap();
    let outside = TempDir::new().unwrap();
    let ctx = ToolContext::new_with_permission_strategy(
        workspace.path().to_path_buf(),
        "test-session".to_string(),
        RuntimeMode::Plan,
        PermissionStrategy::BypassPermissions,
    );
    let args = json!({
        "command": "pwd",
        "cwd": outside.path().to_string_lossy(),
    });

    let result = shell
        .execute(args, ctx)
        .await
        .expect("shell should execute");

    assert!(
        result.success,
        "BypassPermissions should skip Alius cwd boundary denial, got: {}",
        result.output
    );
}

// ============================================================================
// Scope analysis: shell metacharacters
// ============================================================================

#[test]
fn test_pipe_to_external_path_detected() {
    let mut req = make_request("/workspace", "/workspace", "cat file | tee /tmp/out");
    req.args = vec!["/tmp/out".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis.external_paths.contains(&PathBuf::from("/tmp/out")),
        "Pipe to external path should be detected"
    );
}

#[test]
fn testsemicolon_with_external_command_detected() {
    let mut req = make_request("/workspace", "/workspace", "ls; cat /etc/passwd");
    req.args = vec!["/etc/passwd".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis
            .external_paths
            .contains(&PathBuf::from("/etc/passwd")),
        "Semicolon with external path should be detected"
    );
}

#[test]
fn test_output_short_flag_external_path_detected() {
    let mut req = make_request("/workspace", "/workspace", "command -o /tmp/out");
    req.args = vec!["-o".to_string(), "/tmp/out".to_string()];
    let analysis = analyze_scope(&req);
    assert!(
        analysis.external_paths.contains(&PathBuf::from("/tmp/out")),
        "Short -o flag with external path should be detected"
    );
}
