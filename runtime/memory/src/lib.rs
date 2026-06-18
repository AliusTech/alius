pub mod bridge;
pub mod conversation;
pub mod episodic;
pub mod memory;
pub mod paths;
pub mod procedural;
pub mod retrieval;
pub mod semantic;
pub mod session;

pub use bridge::MemoryBridge;
pub use conversation::ConversationStore;
pub use episodic::EpisodicStore;
pub use memory::MemoryStore;
pub use procedural::ProceduralStore;
pub use protocol_interface::SessionId;
pub use retrieval::RetrievalEngine;
pub use semantic::SemanticStore;
pub use session::SessionStore;

#[cfg(test)]
pub(crate) mod test_util {
    pub static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
}
