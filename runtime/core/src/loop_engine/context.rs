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
    /// Session manager — used by tool_step to register/await confirmations (Stage B).
    pub session: Option<Arc<crate::SessionManager>>,
    /// Maximum context window tokens for truncation decisions.
    pub max_context_tokens: usize,
    /// Cancellation token for stopping the run mid-execution.
    pub cancel_token: Option<tokio_util::sync::CancellationToken>,
}

impl LoopContext {
    pub fn with_tool_registry(mut self, registry: Arc<ToolRegistry>) -> Self {
        self.tool_registry = Some(registry);
        self
    }

    pub fn with_session(mut self, session: Arc<crate::SessionManager>) -> Self {
        self.session = Some(session);
        self
    }

    pub fn with_max_context_tokens(mut self, tokens: usize) -> Self {
        self.max_context_tokens = tokens;
        self
    }
}
