//! Planning step — plan data structures and management.
//!
//! Plan mode uses tool calls from the model as plan steps. Each iteration
//! the model may request one or more tool calls; the planner tracks them
//! as a sequence of steps.

use serde::{Deserialize, Serialize};

use protocol_interface::core::ConvergenceDecision;
use runtime_model::ToolCall;

/// A plan composed of executable steps.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub steps: Vec<PlanStep>,
    pub goal: String,
    pub status: PlanStatus,
}

/// A single step within a plan.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanStep {
    pub index: usize,
    pub description: String,
    pub tool_name: Option<String>,
    pub tool_args: Option<serde_json::Value>,
    pub status: PlanStepStatus,
    pub result: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlanStatus {
    Proposed,
    Approved,
    Executing,
    Completed,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlanStepStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl Plan {
    /// Create a new plan from a user goal.
    pub fn new(goal: String) -> Self {
        Self {
            steps: Vec::new(),
            goal,
            status: PlanStatus::Proposed,
        }
    }

    /// Add steps from model-requested tool calls.
    pub fn add_tool_steps(&mut self, tool_calls: &[ToolCall]) {
        for call in tool_calls {
            self.steps.push(PlanStep {
                index: self.steps.len(),
                description: format!("Execute tool: {}", call.name),
                tool_name: Some(call.name.clone()),
                tool_args: Some(call.args.clone()),
                status: PlanStepStatus::Pending,
                result: None,
            });
        }
    }

    /// Mark a step as completed with its result.
    pub fn complete_step(&mut self, index: usize, result: String) {
        if let Some(step) = self.steps.get_mut(index) {
            step.status = PlanStepStatus::Completed;
            step.result = Some(result);
        }
    }

    /// Mark a step as failed.
    pub fn fail_step(&mut self, index: usize, error: String) {
        if let Some(step) = self.steps.get_mut(index) {
            step.status = PlanStepStatus::Failed;
            step.result = Some(error);
        }
    }

    /// Get the next pending step.
    pub fn next_pending_step(&self) -> Option<&PlanStep> {
        self.steps
            .iter()
            .find(|s| s.status == PlanStepStatus::Pending)
    }

    /// Check if all steps are done (completed or failed).
    pub fn is_complete(&self) -> bool {
        self.steps
            .iter()
            .all(|s| s.status == PlanStepStatus::Completed || s.status == PlanStepStatus::Failed)
    }

    /// Check if any step failed.
    pub fn has_failures(&self) -> bool {
        self.steps
            .iter()
            .any(|s| s.status == PlanStepStatus::Failed)
    }

    /// Check plan convergence decision.
    pub fn convergence_decision(&self) -> ConvergenceDecision {
        if self.is_complete() {
            if self.has_failures() {
                ConvergenceDecision::Failed
            } else {
                ConvergenceDecision::Completed
            }
        } else {
            ConvergenceDecision::Continue
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_tool_call(name: &str, args: serde_json::Value) -> ToolCall {
        ToolCall::new("call-1".to_string(), name.to_string(), args)
    }

    #[test]
    fn plan_from_tool_calls() {
        let mut plan = Plan::new("implement feature".to_string());
        plan.add_tool_steps(&[
            sample_tool_call("read_file", json!({"path": "src/main.rs"})),
            sample_tool_call(
                "write_file",
                json!({"path": "src/new.rs", "content": "hello"}),
            ),
        ]);

        assert_eq!(plan.steps.len(), 2);
        assert_eq!(plan.steps[0].tool_name, Some("read_file".to_string()));
        assert_eq!(plan.steps[1].status, PlanStepStatus::Pending);
    }

    #[test]
    fn complete_and_fail_steps() {
        let mut plan = Plan::new("test".to_string());
        plan.add_tool_steps(&[sample_tool_call("read_file", json!({}))]);
        plan.add_tool_steps(&[sample_tool_call("write_file", json!({}))]);

        plan.complete_step(0, "file contents".to_string());
        assert_eq!(plan.steps[0].status, PlanStepStatus::Completed);

        plan.fail_step(1, "permission denied".to_string());
        assert_eq!(plan.steps[1].status, PlanStepStatus::Failed);

        assert!(plan.is_complete());
        assert!(plan.has_failures());
    }

    #[test]
    fn convergence_decision_completed() {
        let mut plan = Plan::new("test".to_string());
        plan.add_tool_steps(&[sample_tool_call("read_file", json!({}))]);
        plan.complete_step(0, "ok".to_string());

        assert_eq!(plan.convergence_decision(), ConvergenceDecision::Completed);
    }

    #[test]
    fn convergence_decision_continue() {
        let mut plan = Plan::new("test".to_string());
        plan.add_tool_steps(&[sample_tool_call("read_file", json!({}))]);

        assert_eq!(plan.convergence_decision(), ConvergenceDecision::Continue);
    }

    #[test]
    fn next_pending_step() {
        let mut plan = Plan::new("test".to_string());
        plan.add_tool_steps(&[
            sample_tool_call("read_file", json!({})),
            sample_tool_call("write_file", json!({})),
        ]);

        plan.complete_step(0, "ok".to_string());
        let next = plan.next_pending_step().unwrap();
        assert_eq!(next.index, 1);
    }

    #[test]
    fn plan_serialization_roundtrip() {
        let mut plan = Plan::new("implement feature".to_string());
        plan.add_tool_steps(&[sample_tool_call("read_file", json!({"path": "a.rs"}))]);
        plan.status = PlanStatus::Executing;

        let json = serde_json::to_string(&plan).unwrap();
        let decoded: Plan = serde_json::from_str(&json).unwrap();

        assert_eq!(decoded.goal, "implement feature");
        assert_eq!(decoded.status, PlanStatus::Executing);
        assert_eq!(decoded.steps.len(), 1);
    }
}
