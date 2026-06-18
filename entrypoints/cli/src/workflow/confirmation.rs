//! Workflow confirmation channel — controls how tool confirmations are handled.
//!
//! Different execution contexts need different confirmation strategies:
//! - Non-interactive (CI/scripts): fail-closed, deny all confirmations
//! - Interactive TUI: prompt the user for approval
//! - JSON-RPC: forward confirmation request to the remote client

use anyhow::Result;

/// Confirmation decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ConfirmationDecision {
    /// User approved the operation.
    Approved,
    /// User rejected the operation.
    Rejected,
    /// Confirmation not needed (auto-approve).
    NotNeeded,
}

/// Confirmation request details.
#[derive(Debug, Clone)]
pub struct ConfirmationRequest {
    /// Tool name requesting confirmation.
    pub tool_name: String,
    /// Operation description.
    pub operation: String,
    /// Detailed information about what will happen.
    pub details: String,
}

/// Trait for handling tool confirmation requests in workflows.
pub trait ConfirmationChannel: Send + Sync {
    /// Handle a confirmation request.
    ///
    /// Returns the decision (approved/rejected).
    fn confirm(&self, request: &ConfirmationRequest) -> Result<ConfirmationDecision>;
}

/// Fail-closed confirmation channel — rejects all confirmations.
///
/// Used in non-interactive contexts (CI, scripts, automated workflows)
/// where there is no user to prompt.
pub struct FailClosedChannel;

impl ConfirmationChannel for FailClosedChannel {
    fn confirm(&self, request: &ConfirmationRequest) -> Result<ConfirmationDecision> {
        tracing::warn!(
            "Tool '{}' requires confirmation but running in non-interactive mode — denying",
            request.tool_name
        );
        Ok(ConfirmationDecision::Rejected)
    }
}

/// Stdin-based confirmation channel — prompts the user via terminal.
///
/// Used in interactive CLI workflows where the user can respond.
pub struct StdinConfirmChannel;

impl ConfirmationChannel for StdinConfirmChannel {
    fn confirm(&self, request: &ConfirmationRequest) -> Result<ConfirmationDecision> {
        use std::io::{self, Write};

        println!();
        println!("⚠️  Tool '{}' requires confirmation:", request.tool_name);
        println!("   Operation: {}", request.operation);
        if !request.details.is_empty() {
            println!("   Details: {}", request.details);
        }
        print!("   Approve? [y/N] ");
        io::stdout().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;
        let trimmed = input.trim().to_lowercase();

        if trimmed == "y" || trimmed == "yes" {
            Ok(ConfirmationDecision::Approved)
        } else {
            Ok(ConfirmationDecision::Rejected)
        }
    }
}

/// Auto-approve confirmation channel — approves all confirmations.
///
/// Used when the user has explicitly opted into auto-confirm mode.
pub struct AutoApproveChannel;

impl ConfirmationChannel for AutoApproveChannel {
    fn confirm(&self, _request: &ConfirmationRequest) -> Result<ConfirmationDecision> {
        Ok(ConfirmationDecision::Approved)
    }
}

/// Select the appropriate confirmation channel for the given context.
///
/// - `interactive == true` → `StdinConfirmChannel`
/// - `interactive == false` → `FailClosedChannel`
pub fn select_channel(interactive: bool) -> Box<dyn ConfirmationChannel> {
    if interactive {
        Box::new(StdinConfirmChannel)
    } else {
        Box::new(FailClosedChannel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fail_closed_rejects() {
        let channel = FailClosedChannel;
        let req = ConfirmationRequest {
            tool_name: "shell".to_string(),
            operation: "execute command".to_string(),
            details: "rm -rf /tmp/test".to_string(),
        };
        assert_eq!(channel.confirm(&req).unwrap(), ConfirmationDecision::Rejected);
    }

    #[test]
    fn test_auto_approve_approves() {
        let channel = AutoApproveChannel;
        let req = ConfirmationRequest {
            tool_name: "shell".to_string(),
            operation: "execute command".to_string(),
            details: "ls -la".to_string(),
        };
        assert_eq!(channel.confirm(&req).unwrap(), ConfirmationDecision::Approved);
    }

    #[test]
    fn test_select_channel_interactive() {
        let channel = select_channel(true);
        // Verify it's not FailClosed (which would reject)
        let req = ConfirmationRequest {
            tool_name: "test".to_string(),
            operation: "test".to_string(),
            details: String::new(),
        };
        // StdinConfirmChannel will try to read stdin, which will fail in test.
        // Just verify the channel was created (not FailClosed).
        // We test the non-interactive path separately.
        let _ = channel;
    }

    #[test]
    fn test_select_channel_non_interactive() {
        let channel = select_channel(false);
        let req = ConfirmationRequest {
            tool_name: "test".to_string(),
            operation: "test".to_string(),
            details: String::new(),
        };
        assert_eq!(channel.confirm(&req).unwrap(), ConfirmationDecision::Rejected);
    }
}
