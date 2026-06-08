//! Episodic memory — stores events, messages, and tool calls per session.
//!
//! Uses SQLite for durable persistence. Events are organized by session, run,
//! and turn with trace_id correlation for timeline reconstruction.

pub mod store;
pub mod types;

pub use store::EpisodicStore;
pub use types::CoreEvent;
