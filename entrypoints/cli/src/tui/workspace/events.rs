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
    ExecuteSelectedNodes,
    RevisePlan(String),
    CancelDecision,
    ApproveReview,
    RequestRevision(String),
    ViewEvidence,
    RerunNode,
    InitReconfigure,
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

#[derive(Debug, Clone, Copy)]
pub enum DecisionKind {
    PlanApproval,
    NodeReview,
    InitCommand,
}
