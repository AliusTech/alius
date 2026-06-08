//! Loop iteration state.

use protocol_interface::core::{LoopPolicy, RuntimeMode};

#[derive(Debug, Clone)]
pub struct LoopIteration {
    pub index: u32,
    pub mode: RuntimeMode,
    pub policy: LoopPolicy,
}

impl LoopIteration {
    pub fn first(mode: RuntimeMode, policy: LoopPolicy) -> Self {
        Self {
            index: 1,
            mode,
            policy,
        }
    }
}
