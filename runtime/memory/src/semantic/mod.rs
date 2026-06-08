//! Semantic memory — stores facts, documents, and chunks for retrieval.
//!
//! Uses SQLite for persistence with keyword search. Embedding-based semantic
//! search is a placeholder that degrades to keyword search when unavailable.

pub mod store;
pub mod types;

pub use store::SemanticStore;
pub use types::{MemoryHit, SemanticFact};
