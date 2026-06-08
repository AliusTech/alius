//! Session storage

use anyhow::Result;
use chrono::Utc;
use std::path::PathBuf;

use protocol_interface::{SessionId, SessionMetadata};

use super::paths;

/// Session store for managing session persistence
pub struct SessionStore {
    base_path: PathBuf,
    legacy_base_paths: Vec<PathBuf>,
}

impl SessionStore {
    /// Create a new session store
    pub fn new() -> Result<Self> {
        let base_path = Self::get_base_path();
        std::fs::create_dir_all(&base_path)?;
        Ok(Self {
            base_path,
            legacy_base_paths: Self::get_legacy_base_paths(),
        })
    }

    /// Get the base path for sessions
    fn get_base_path() -> PathBuf {
        paths::project_communication_sessions_dir()
    }

    fn get_legacy_base_paths() -> Vec<PathBuf> {
        vec![
            paths::project_alius_dir().join("sessions"),
            paths::global_alius_dir().join("sessions"),
        ]
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
        let session_file = self.session_file(id);
        let content = std::fs::read_to_string(session_file)?;
        let session: SessionMetadata = serde_json::from_str(&content)?;
        Ok(session)
    }

    /// List all sessions
    pub fn list(&self) -> Result<Vec<SessionMetadata>> {
        let mut sessions = Vec::new();
        for base_path in self.read_paths() {
            if !base_path.exists() {
                continue;
            }

            for entry in std::fs::read_dir(&base_path)? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    let session_file = path.join("session.json");
                    if session_file.exists() {
                        let content = std::fs::read_to_string(&session_file)?;
                        let session: SessionMetadata = serde_json::from_str(&content)?;
                        if !sessions
                            .iter()
                            .any(|s: &SessionMetadata| s.id == session.id)
                        {
                            sessions.push(session);
                        }
                    }
                }
            }
        }
        sessions.sort_by_key(|b| std::cmp::Reverse(b.updated_at));
        Ok(sessions)
    }

    fn session_file(&self, id: &SessionId) -> PathBuf {
        let primary = self.base_path.join(id.as_str()).join("session.json");
        if primary.exists() {
            return primary;
        }

        for base_path in &self.legacy_base_paths {
            let legacy = base_path.join(id.as_str()).join("session.json");
            if legacy.exists() {
                return legacy;
            }
        }

        primary
    }

    fn read_paths(&self) -> Vec<PathBuf> {
        let mut paths = vec![self.base_path.clone()];
        paths.extend(self.legacy_base_paths.clone());
        paths
    }

    /// Delete a session
    pub fn delete(&self, id: &SessionId) -> Result<()> {
        for base_path in self.read_paths() {
            let session_dir = base_path.join(id.as_str());
            if session_dir.exists() {
                std::fs::remove_dir_all(session_dir)?;
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sessions_use_memory_communications_directory() {
        let _lock = crate::test_util::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let orig_cwd = std::env::current_dir().unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = tmp.path().join("project");
        std::fs::create_dir_all(&project_dir).unwrap();
        std::env::set_current_dir(&project_dir).unwrap();

        let store = SessionStore::new().unwrap();
        let session = store.create("test-model".to_string());
        let session_id = session.id.clone();
        store.save(&session).unwrap();
        std::env::set_current_dir(orig_cwd).unwrap();

        assert!(project_dir
            .join(".alius")
            .join("memory")
            .join("communications")
            .join("sessions")
            .join(session_id.as_str())
            .join("session.json")
            .exists());
    }
}
