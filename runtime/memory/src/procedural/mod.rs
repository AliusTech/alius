//! Procedural memory — stores procedures, workflows, and failure patterns.

pub mod store;
pub mod types;

pub use store::ProceduralStore;
pub use types::{FailurePattern, Procedure, ProcedureHit};
