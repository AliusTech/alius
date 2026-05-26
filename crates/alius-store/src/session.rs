//! Session storage

use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

use alius_protocol::{SessionId, SessionMetadata};

/// Session store for managing session persistence
pub struct SessionStore {
    base_path: PathBuf,
}

impl SessionStore {
    /// Create a new session store
    pub fn new() -> Result<Self> {
        let base_path = Self::get_base_path();
        std::fs::create_dir_all(&base_path)?;
        Ok(Self { base_path })
    }

    /// Get the base path for sessions
    fn get_base_path() -> PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(format!("{}/.alius/sessions", home))
    }

    /// Create a new session
    pub fn create(&self, model: String) -> SessionMetadata {
        SessionMetadata::new(model)
    }

    /// Save a session
    pub fn save(&self, session: &SessionMetadata) -> Result<()> {
        let session_dir = self.base_path.join(session.id.as_str());
        std::fs::create_dir_all(&session_dir)?;

        let session_file = session_dir.join("session.json");
        let content = serde_json::to_string_pretty(session)?;
        std::fs::write(session_file, content)?;

        Ok(())
    }

    /// Load a session by ID
    pub fn load(&self, id: &SessionId) -> Result<SessionMetadata> {
        let session_file = self.base_path.join(id.as_str()).join("session.json");
        let content = std::fs::read_to_string(session_file)?;
        let session: SessionMetadata = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// List all sessions
    pub fn list(&self) -> Result<Vec<SessionMetadata>> {
        let mut sessions = Vec::new();
        for entry in std::fs::read_dir(&self.base_path)? {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                let session_file = path.join("session.json");
                if session_file.exists() {
                    let content = std::fs::read_to_string(session_file)?;
                    let session: SessionMetadata = serde_json::from_str(&content)?;
                    sessions.push(session);
                }
            }
        }
        sessions.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(sessions)
    }

    /// Delete a session
    pub fn delete(&self, id: &SessionId) -> Result<()> {
        let session_dir = self.base_path.join(id.as_str());
        if session_dir.exists() {
            std::fs::remove_dir_all(session_dir)?;
        }
        Ok(())
    }

    /// Update session metadata
    pub fn update(&self, session: &mut SessionMetadata) -> Result<()> {
        session.updated_at = Utc::now();
        self.save(session)?;
        Ok(())
    }
}

impl Default for SessionStore {
    fn default() -> Self {
        Self::new().expect("Failed to create session store")
    }
}