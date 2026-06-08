//! Unified Loop Engine.
//!
//! Chat and Plan modes both enter this engine. The mode-specific behavior is
//! controlled by `LoopPolicy` rather than by separate DirectChat/AgentLoop
//! execution paths.

pub mod context;
pub mod context_manager;
pub mod convergence;
pub mod engine;
pub mod iteration;
pub mod model_step;
pub mod planner;
pub mod policy;
pub mod result;
pub mod tool_step;

pub use context::LoopContext;
pub use context_manager::ContextManager;
pub use convergence::check_convergence;
pub use engine::LoopEngine;
pub use iteration::LoopIteration;
pub use result::LoopExecutionResult;
