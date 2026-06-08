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
pub fn authorize(request: &ShellCommandRequest, config: &ShellGateConfig) -> ShellGateDecision {
    let inspection = parse_command(request);
    let scope = analyze_scope(request);

    // RemoteA2A and Embedded origins: restrict to low-risk only.
    if config.restrict_remote_to_readonly
        && matches!(
            request.origin,
            ShellOrigin::RemoteA2A | ShellOrigin::Embedded
        )
        && inspection.risk_level > RiskLevel::Low
    {
        return ShellGateDecision::Deny {
            reason: format!(
                "origin {:?} is restricted to read-only commands; '{}' is {:?} risk",
                request.origin, inspection.base_command, inspection.risk_level
            ),
        };
    }

    // Symlink escape — always deny.
    if scope.symlink_escape {
        return ShellGateDecision::Deny {
            reason: "symlink escape detected — cwd resolves outside workspace".into(),
        };
    }

    // Critical risk level.
    match inspection.risk_level {
        RiskLevel::Critical => {
            if config.deny_critical {
                return ShellGateDecision::Deny {
                    reason: format!(
                        "command '{}' classified as Critical risk",
                        inspection.raw_command
                    ),
                };
            }
            return ShellGateDecision::ApprovalRequired {
                reason: format!(
                    "command '{}' classified as Critical risk — requires approval",
                    inspection.raw_command
                ),
            };
        }
        RiskLevel::High => {
            return ShellGateDecision::ApprovalRequired {
                reason: format!(
                    "command '{}' classified as High risk — requires approval",
                    inspection.raw_command
                ),
            };
        }
        _ => {}
    }

    // Outside workspace.
    if !scope.cwd_inside_workspace && config.deny_outside_workspace {
        return ShellGateDecision::Deny {
            reason: "command targets paths outside workspace and deny_outside_workspace is enabled"
                .into(),
        };
    }
    if !scope.external_paths.is_empty() {
        return ShellGateDecision::ApprovalRequired {
            reason: format!(
                "command references paths outside workspace: {:?}",
                scope.external_paths
            ),
        };
    }

    ShellGateDecision::Allow
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
        let decision = authorize(&req, &ShellGateConfig::default());
        assert_eq!(decision, ShellGateDecision::Allow);
    }

    #[test]
    fn test_authorize_rm_rf_root_deny() {
        let req = make_request("rm -rf /");
        let decision = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_authorize_sudo_deny() {
        let req = make_request("sudo rm -rf /");
        let decision = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_authorize_rm_rf_specific_approval() {
        let req = make_request("rm -rf /workspace/build");
        let decision = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(
            decision,
            ShellGateDecision::ApprovalRequired { .. }
        ));
    }

    #[test]
    fn test_remote_restricted_to_readonly() {
        let req = make_request_with_origin("cp a.txt b.txt", ShellOrigin::RemoteA2A);
        let decision = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }

    #[test]
    fn test_remote_low_risk_allowed() {
        let req = make_request_with_origin("ls -la", ShellOrigin::RemoteA2A);
        let decision = authorize(&req, &ShellGateConfig::default());
        assert_eq!(decision, ShellGateDecision::Allow);
    }

    #[test]
    fn test_embedded_restricted() {
        let req = make_request_with_origin("find . -name '*.rs'", ShellOrigin::Embedded);
        let decision = authorize(&req, &ShellGateConfig::default());
        // find is low risk — should be allowed
        assert_eq!(decision, ShellGateDecision::Allow);
    }

    #[test]
    fn test_embedded_write_denied() {
        let req = make_request_with_origin("rm file.txt", ShellOrigin::Embedded);
        let decision = authorize(&req, &ShellGateConfig::default());
        assert!(matches!(decision, ShellGateDecision::Deny { .. }));
    }
}
