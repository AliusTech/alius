//! Loop convergence checks.

use protocol_interface::core::{ConvergenceDecision, ConvergenceReport, RuntimeMode};

use crate::loop_engine::LoopIteration;

/// Check whether the loop should continue iterating.
///
/// - `has_pending_tools`: the model requested more tool calls this iteration
/// - `plan_completed`: a plan has finished all steps
pub fn check_convergence(
    iteration: &LoopIteration,
    has_pending_tools: bool,
    plan_completed: bool,
) -> ConvergenceReport {
    let (decision, reason, next_action) = if plan_completed {
        (
            ConvergenceDecision::Completed,
            match iteration.mode {
                RuntimeMode::Chat => "chat mode completed",
                RuntimeMode::Plan => "plan completed all steps",
            }
            .to_string(),
            None,
        )
    } else if !has_pending_tools {
        (
            ConvergenceDecision::Completed,
            match iteration.mode {
                RuntimeMode::Chat => "chat mode completed after one model step",
                RuntimeMode::Plan => "model produced final response without tool calls",
            }
            .to_string(),
            None,
        )
    } else if iteration.index >= iteration.policy.max_iterations {
        (
            ConvergenceDecision::MaxIterationsReached,
            "loop policy max_iterations reached".to_string(),
            Some("emit final error or ask user for direction".to_string()),
        )
    } else {
        (
            ConvergenceDecision::Continue,
            format!(
                "iteration {} of {}, continuing",
                iteration.index, iteration.policy.max_iterations
            ),
            Some("continue loop".to_string()),
        )
    };

    ConvergenceReport {
        iteration: iteration.index,
        decision,
        reason,
        remaining_steps: Vec::new(),
        next_action,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::core::LoopPolicy;

    #[test]
    fn chat_mode_completes_without_tools() {
        let iter = LoopIteration::first(RuntimeMode::Chat, LoopPolicy::chat());
        let report = check_convergence(&iter, false, false);
        assert_eq!(report.decision, ConvergenceDecision::Completed);
    }

    #[test]
    fn plan_mode_completes_when_plan_done() {
        let iter = LoopIteration::first(RuntimeMode::Plan, LoopPolicy::plan());
        let report = check_convergence(&iter, false, true);
        assert_eq!(report.decision, ConvergenceDecision::Completed);
    }

    #[test]
    fn plan_mode_continues_with_pending_tools() {
        let iter = LoopIteration::first(RuntimeMode::Plan, LoopPolicy::plan());
        let report = check_convergence(&iter, true, false);
        assert_eq!(report.decision, ConvergenceDecision::Continue);
    }

    #[test]
    fn max_iterations_reached() {
        let mut iter = LoopIteration::first(RuntimeMode::Plan, LoopPolicy::plan());
        iter.index = 20;
        let report = check_convergence(&iter, true, false);
        assert_eq!(report.decision, ConvergenceDecision::MaxIterationsReached);
    }

    #[test]
    fn plan_mode_completes_without_tool_calls() {
        let iter = LoopIteration::first(RuntimeMode::Plan, LoopPolicy::plan());
        let report = check_convergence(&iter, false, false);
        assert_eq!(report.decision, ConvergenceDecision::Completed);
        assert!(report.reason.contains("final response"));
    }
}
