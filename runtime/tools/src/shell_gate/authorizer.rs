//! Authorization logic combining inspection and scope analysis.

use super::inspector::{parse_command, RiskLevel};
use super::scope::analyze_scope;
use super::{ShellCommandRequest, ShellOrigin};

/// Configuration for Shell Gate authorization.
#[derive(Debug, Clone)]
pub struct ShellGateConfig {
    /// Whether to deny Critical commands outright (vs. require approval).
    pub deny_critical: bool,
    /// Whether to deny commands outside workspace scope.
    pub deny_outside_workspace: bool,
    /// Whether RemoteA2A origins are restricted to read-only commands.
    pub restrict_remote_to_readonly: bool,
}

impl Default for ShellGateConfig {
    fn default() -> Self {
        Self {
            deny_critical: true,
            deny_outside_workspace: false,
            restrict_remote_to_readonly: true,
        }
    }
}

/// Authorization decision for a shell command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShellGateDecision {
    /// Command is allowed to execute.
    Allow,
    /// Command is denied — must not execute.
    Deny { reason: String },
    /// Command requires user approval before execution.
    ApprovalRequired { reason: String },
}

/// Run the full Shell Gate pipeline on a command request.
/// Returns (decision, risk_level) so callers can apply the unified policy matrix.
pub fn authorize(
    request: &ShellCommandRequest,
    config: &ShellGateConfig,
) -> (ShellGateDecision, RiskLevel) {
    let inspection = parse_command(request);
    let scope = analyze_scope(request);

    // RemoteA2A and WASM plugin origins: restrict to low-risk only.
    if config.restrict_remote_to_readonly
        && matches!(
            request.origin,
            ShellOrigin::RemoteA2A | ShellOrigin::WasmPlugin
        )
        && inspection.risk_level > RiskLevel::Low
    {
        return (
            ShellGateDecision::Deny {
                reason: format!(
                    "origin {:?} is restricted to read-only commands; '{}' is {:?} risk",
                    request.origin, inspection.base_command, inspection.risk_level
                ),
            },
            inspection.risk_level,
        );
    }

    // Symlink escape — always deny.
    if scope.symlink_escape {
        return (
            ShellGateDecision::Deny {
                reason: "symlink escape detected — cwd resolves outside workspace".into(),
            },
            inspection.risk_level,
        );
    }

    // ============================================================================
    // WORKSPACE BOUNDARY CHECKS - MUST COME BEFORE RISK LEVEL CHECKS
    // ============================================================================
    // These are hard boundaries that apply regardless of risk level.
    // A high-risk command targeting external paths (e.g., rm -rf /tmp/foo)
    // must be denied, not approved.

    // Outside workspace - cwd check
    if !scope.cwd_inside_workspace && config.deny_outside_workspace {
        return (
            ShellGateDecision::Deny {
                reason:
                    "command targets paths outside workspace and deny_outside_workspace is enabled"
                        .into(),
            },
            inspection.risk_level,
        );
    }

    // Outside workspace - external paths (hard boundary)
    if !scope.external_paths.is_empty() {
        return (
            ShellGateDecision::Deny {
                reason: format!(
                    "command references paths outside workspace: {:?}",
                    scope.external_paths
                ),
            },
            inspection.risk_level,
        );
    }

    // ============================================================================
    // RISK LEVEL CHECKS - ONLY FOR WORKSPACE-INTERNAL COMMANDS
    // ============================================================================
    // At this point, we know the command operates within workspace boundaries.

    // Critical risk level.
    match inspection.risk_level {
        RiskLevel::Critical => {
            if config.deny_critical {
                return (
                    ShellGateDecision::Deny {
                        reason: format!(
                            "command '{}' classified as Critical risk",
                            inspection.raw_command
                        ),
                    },
                    inspection.risk_level,
                );
            }
            return (
                ShellGateDecision::ApprovalRequired {
                    reason: format!(
                        "command '{}' classified as Critical risk — requires approval",
                        inspection.raw_command
                    ),
                },
                inspection.risk_level,
            );
        }
        RiskLevel::High => {
            return (
                ShellGateDecision::ApprovalRequired {
                    reason: format!(
                        "command '{}' classified as High risk — requires approval",
                        inspection.raw_command
                    ),
                },
                inspection.risk_level,
            );
        }
        _ => {}
    }

    (ShellGateDecision::Allow, inspection.risk_level)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_request(cmd: &str) -> ShellCommandRequest {
        ShellCommandRequest {
            command: cmd.to_string(),
            args: vec![],
            cwd: PathBuf::from("/workspace"),
            origin: ShellOrigin::LocalCli,
            workspace_root: PathBuf::from("/workspace"),
        }
    }

    fn make_request_with_origin(cmd: &str, origin: ShellOrigin) -> ShellCommandRequest {
        ShellCommandRequest {
            command: cmd.to_string(),
            args: vec![],
            cwd: PathBuf::from("/workspace"),
            origin,
            workspace_root: PathBuf::from("/workspace"),
        }
    }

    #[test]
    fn test_authorize_git_status_allow() {
        let req = make_request("git status");
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert_eq!(decision, ShellGateDecision::Allow);
    }

    #[test]
    fn test_authorize_rm_rf_root_deny() {
        let req = make_request("rm -rf /");
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_authorize_sudo_deny() {
        let req = make_request("sudo rm -rf /");
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_authorize_rm_rf_specific_approval() {
        let req = make_request("rm -rf /workspace/build");
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(
            decision,
            ShellGateDecision::ApprovalRequired { .. }
        ));
    }

    #[test]
    fn test_remote_restricted_to_readonly() {
        let req = make_request_with_origin("cp a.txt b.txt", ShellOrigin::RemoteA2A);
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_remote_low_risk_allowed() {
        let req = make_request_with_origin("ls -la", ShellOrigin::RemoteA2A);
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert_eq!(decision, ShellGateDecision::Allow);
    }

    #[test]
    fn test_wasm_plugin_restricted() {
        let req = make_request_with_origin("find . -name '*.rs'", ShellOrigin::WasmPlugin);
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        // find is low risk — should be allowed
        assert_eq!(decision, ShellGateDecision::Allow);
    }

    #[test]
    fn test_wasm_plugin_write_denied() {
        let req = make_request_with_origin("rm file.txt", ShellOrigin::WasmPlugin);
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_external_path_etc_passwd_denied() {
        let mut req = make_request("cat /etc/passwd");
        req.args = vec!["/etc/passwd".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for /etc/passwd, got {:?}",
            decision
        );
    }

    #[test]
    fn test_external_path_parent_escape_denied() {
        let mut req = make_request("cat ../outside/file.txt");
        req.cwd = PathBuf::from("/workspace");
        req.workspace_root = PathBuf::from("/workspace");
        req.args = vec!["../outside/file.txt".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for ../outside escape, got {:?}",
            decision
        );
    }

    #[test]
    fn test_external_path_output_flag_denied() {
        let mut req = make_request("gcc main.c --output=/tmp/out");
        req.args = vec!["main.c".to_string(), "--output=/tmp/out".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for --output=/tmp/out, got {:?}",
            decision
        );
    }

    #[test]
    fn test_external_path_stdout_redirect_denied() {
        let req = make_request("echo test > /tmp/out");
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for > /tmp/out, got {:?}",
            decision
        );
    }

    #[test]
    fn test_external_path_stderr_redirect_denied() {
        let req = make_request("command 2>/tmp/err");
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for 2>/tmp/err, got {:?}",
            decision
        );
    }

    #[test]
    fn test_workspace_internal_paths_allowed() {
        let mut req = make_request("cat ./src/main.rs");
        req.args = vec!["./src/main.rs".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert_eq!(
            decision,
            ShellGateDecision::Allow,
            "Expected Allow for workspace-internal path"
        );
    }

    #[test]
    fn test_high_risk_workspace_internal_requires_approval() {
        let mut req = make_request("rm -rf ./build");
        req.args = vec!["-rf".to_string(), "./build".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::ApprovalRequired { .. }),
            "Expected ApprovalRequired for high-risk workspace-internal command, got {:?}",
            decision
        );
    }

    #[test]
    fn test_high_risk_external_path_denied() {
        let mut req = make_request("rm -rf /tmp/foo");
        req.args = vec!["-rf".to_string(), "/tmp/foo".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for high-risk external path (rm -rf /tmp/foo), got {:?}",
            decision
        );
    }

    #[test]
    fn test_critical_risk_external_path_denied() {
        let mut req = make_request("sudo rm -rf /etc");
        req.args = vec!["rm".to_string(), "-rf".to_string(), "/etc".to_string()];
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        assert!(
            matches!(decision, ShellGateDecision::Deny { .. }),
            "Expected Deny for critical-risk external path (sudo rm -rf /etc), got {:?}",
            decision
        );
    }
}
