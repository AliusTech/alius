//! Alius Core Runtime implementation.
//!
//! This crate implements the `CoreRuntimeApi` trait defined in `protocol-interface`,
//! providing the unified execution layer for all product entrypoints.

pub mod config;
pub mod event_adapter;
pub mod logging;
pub mod loop_engine;
pub mod manager;
pub mod patch;
pub mod runtime;
pub mod session;

#[cfg(feature = "mcp")]
pub mod mcp_manager;

pub use event_adapter::EventAdapter;
pub use loop_engine::{LoopContext, LoopEngine};
pub use manager::{CoreRuntimeManager, RuntimeManagerContext};
pub use runtime::{CoreRuntime, CoreRuntimeBuilder};
pub use session::SessionManager;
