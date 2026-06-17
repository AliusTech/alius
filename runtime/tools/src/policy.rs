//! Tool permission policy matrix.
//!
//! Defines the unified confirmation/denial policy across tool sources
//! (Native, WASM, MCP) and execution modes (Chat, Plan, Bypass).

/// Tool source type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolSource {
    Native,
    Wasm,
    Mcp,
}

/// Execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunMode {
    Chat,
    Plan,
    Bypass,
}

/// Risk level classification (from Shell Gate).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Policy decision for a tool invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Execute without confirmation.
    Allow,
    /// Require user confirmation before execution.
    Confirm,
    /// Deny execution entirely.
    Deny,
}

/// Determine the policy decision for a tool invocation.
///
/// Returns `Allow`, `Confirm`, or `Deny` based on the tool source,
/// execution mode, and risk level.
pub fn evaluate_policy(source: ToolSource, mode: RunMode, risk: RiskLevel) -> PolicyDecision {
    use PolicyDecision::*;
    use RiskLevel::*;
    use RunMode::*;
    use ToolSource::*;

    match (source, mode, risk) {
        // Native tools
        (Native, Chat, Low) => Allow,
        (Native, Chat, Medium) => Allow,
        (Native, Chat, High) => Confirm,
        (Native, Chat, Critical) => Deny,
        (Native, Plan, Low) => Allow,
        (Native, Plan, Medium) => Allow,
        (Native, Plan, High) => Confirm,
        (Native, Plan, Critical) => Deny,

        // WASM plugins — more restrictive (untrusted code)
        (Wasm, Chat, Low) => Allow,
        (Wasm, Chat, Medium) => Deny,
        (Wasm, Chat, High) => Deny,
        (Wasm, Chat, Critical) => Deny,
        (Wasm, Plan, Low) => Allow,
        (Wasm, Plan, Medium) => Deny,
        (Wasm, Plan, High) => Deny,
        (Wasm, Plan, Critical) => Deny,

        // MCP tools — middle ground
        (Mcp, Chat, Low) => Allow,
        (Mcp, Chat, Medium) => Allow,
        (Mcp, Chat, High) => Confirm,
        (Mcp, Chat, Critical) => Deny,
        (Mcp, Plan, Low) => Allow,
        (Mcp, Plan, Medium) => Confirm,
        (Mcp, Plan, High) => Confirm,
        (Mcp, Plan, Critical) => Deny,

        // Bypass mode: always allow (admin/test context).
        (_, Bypass, _) => Allow,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_native_low_all_modes_allow() {
        assert_eq!(
            evaluate_policy(ToolSource::Native, RunMode::Chat, RiskLevel::Low),
            PolicyDecision::Allow
        );
        assert_eq!(
            evaluate_policy(ToolSource::Native, RunMode::Plan, RiskLevel::Low),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn test_native_high_chat_confirms() {
        assert_eq!(
            evaluate_policy(ToolSource::Native, RunMode::Chat, RiskLevel::High),
            PolicyDecision::Confirm
        );
    }

    #[test]
    fn test_native_high_plan_confirms() {
        assert_eq!(
            evaluate_policy(ToolSource::Native, RunMode::Plan, RiskLevel::High),
            PolicyDecision::Confirm
        );
    }

    #[test]
    fn test_native_critical_always_denies() {
        assert_eq!(
            evaluate_policy(ToolSource::Native, RunMode::Chat, RiskLevel::Critical),
            PolicyDecision::Deny
        );
        assert_eq!(
            evaluate_policy(ToolSource::Native, RunMode::Plan, RiskLevel::Critical),
            PolicyDecision::Deny
        );
    }

    #[test]
    fn test_wasm_medium_chat_denied() {
        assert_eq!(
            evaluate_policy(ToolSource::Wasm, RunMode::Chat, RiskLevel::Medium),
            PolicyDecision::Deny
        );
    }

    #[test]
    fn test_wasm_low_chat_allow() {
        assert_eq!(
            evaluate_policy(ToolSource::Wasm, RunMode::Chat, RiskLevel::Low),
            PolicyDecision::Allow
        );
    }

    #[test]
    fn test_wasm_all_non_low_denied() {
        for risk in [RiskLevel::Medium, RiskLevel::High, RiskLevel::Critical] {
            assert_eq!(
                evaluate_policy(ToolSource::Wasm, RunMode::Chat, risk),
                PolicyDecision::Deny
            );
            assert_eq!(
                evaluate_policy(ToolSource::Wasm, RunMode::Plan, risk),
                PolicyDecision::Deny
            );
        }
    }

    #[test]
    fn test_mcp_high_plan_confirms() {
        assert_eq!(
            evaluate_policy(ToolSource::Mcp, RunMode::Plan, RiskLevel::High),
            PolicyDecision::Confirm
        );
    }

    #[test]
    fn test_mcp_medium_plan_confirms() {
        assert_eq!(
            evaluate_policy(ToolSource::Mcp, RunMode::Plan, RiskLevel::Medium),
            PolicyDecision::Confirm
        );
    }

    #[test]
    fn test_mcp_critical_always_denies() {
        assert_eq!(
            evaluate_policy(ToolSource::Mcp, RunMode::Chat, RiskLevel::Critical),
            PolicyDecision::Deny
        );
        assert_eq!(
            evaluate_policy(ToolSource::Mcp, RunMode::Plan, RiskLevel::Critical),
            PolicyDecision::Deny
        );
    }

    #[test]
    fn test_bypass_always_allow() {
        for source in [ToolSource::Native, ToolSource::Wasm, ToolSource::Mcp] {
            for risk in [
                RiskLevel::Low,
                RiskLevel::Medium,
                RiskLevel::High,
                RiskLevel::Critical,
            ] {
                assert_eq!(
                    evaluate_policy(source, RunMode::Bypass, risk),
                    PolicyDecision::Allow
                );
            }
        }
    }
}
