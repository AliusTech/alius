//! Loop engine execution context.

use std::path::PathBuf;
use std::sync::Arc;

use runtime_config::LlmSettings;
use runtime_model::{Conversation, LlmClient};
use runtime_tools::ToolRegistry;

/// Context needed by the loop engine for a single run.
pub struct LoopContext {
    pub client: Arc<LlmClient>,
    pub conversation: Conversation,
    pub settings: LlmSettings,
    pub workspace: PathBuf,
    /// Tool registry for Plan mode tool execution.
    pub tool_registry: Option<Arc<ToolRegistry>>,
    /// Maximum context window tokens for truncation decisions.
    pub max_context_tokens: usize,
}

impl LoopContext {
    pub fn with_tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn with_max_context_tokens(mut self, tokens: usize) -> Self {
        self.max_context_tokens = tokens;
        self
    }
}
