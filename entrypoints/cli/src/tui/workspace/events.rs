#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    Plan,
    Bypass,
}

#[derive(Debug, Clone)]
pub enum WorkspaceAction {
    None,
    Submit(String),
    ApprovePlan,
    #[allow(dead_code)]
    ExecuteSelectedNodes,
    RevisePlan(String),
    CancelDecision,
    ApproveReview,
    RequestRevision(String),
    ViewEvidence,
    RerunNode,
    InitReconfigure,
    InterruptExecution,
    ContinueExecution,
    ContinueConfig,
    ClosePlan,
    Quit,
}

#[derive(Debug, Clone)]
pub struct CommandOutcome {
    pub output: String,
    pub quit: bool,
    pub clear_blocks: bool,
    pub show_init_menu: bool,
}

impl CommandOutcome {
    pub fn output(output: impl Into<String>) -> Self {
        Self {
            output: output.into(),
            quit: false,
            clear_blocks: false,
            show_init_menu: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    PlanApproval,
    #[allow(dead_code)]
    NodeReview,
    InitCommand,
    ExecutionInterrupt,
    ConfigExit,
    PlanCompletion,
}
