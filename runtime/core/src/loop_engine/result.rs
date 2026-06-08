//! Loop execution result.

use protocol_interface::core::CoreEvent;

#[derive(Debug, Clone)]
pub struct LoopExecutionResult {
    pub events: Vec<CoreEvent>,
    pub final_content: String,
}
