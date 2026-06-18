//! Alius Core Runtime implementation.
//!
//! This crate implements the `CoreRuntimeApi` trait defined in `protocol-interface`,
//! providing the unified execution layer for all product entrypoints.

pub mod a2a;
pub mod config;
pub mod event_adapter;
pub mod logging;
pub mod loop_engine;
pub mod manager;
pub mod patch;
pub mod plan_store;
pub mod runtime;
pub mod session;

#[cfg(feature = "mcp")]
pub mod mcp_manager;

pub use a2a::{
    A2AMessage, A2AMessageStatus, A2AMessageType, A2ATransport, AgentEndpoint, LocalA2ATransport,
};
pub use event_adapter::EventAdapter;
pub use loop_engine::{LoopContext, LoopEngine};
pub use manager::{CoreRuntimeManager, RuntimeManagerContext};
pub use plan_store::{FilePlanStore, InMemoryPlanStore, PlanNode, PlanNodeStatus, PlanStore};
pub use runtime::{CoreRuntime, CoreRuntimeBuilder};
pub use runtime_model::LlmClient;
pub use session::SessionManager;

/// Test utilities (gated behind `testing` feature or `#[cfg(test)]`).
#[cfg(any(test, feature = "testing"))]
pub mod testing;
