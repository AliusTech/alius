//! SQLite-backed episodic memory store.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use super::types::CoreEvent;

/// SQLite-backed episodic memory store.
pub struct EpisodicStore {
    conn: Connection,
}

impl EpisodicStore {
    /// Open (or create) an episodic store at the given database path.
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory store (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        self.conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS sessions (
                id TEXT PRIMARY KEY,
                workspace TEXT NOT NULL,
                started_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS turns (
                id TEXT PRIMARY KEY,
                session_id TEXT NOT NULL,
                role TEXT NOT NULL,
                idx INTEGER NOT NULL,
                FOREIGN KEY (session_id) REFERENCES sessions(id)
            );
            CREATE TABLE IF NOT EXISTS messages (
                id TEXT PRIMARY KEY,
                turn_id TEXT NOT NULL,
                content TEXT NOT NULL,
                summary TEXT,
                created_at TEXT NOT NULL,
                FOREIGN KEY (turn_id) REFERENCES turns(id)
            );
            CREATE TABLE IF NOT EXISTS core_events (
                id TEXT PRIMARY KEY,
                trace_id TEXT NOT NULL,
                session_id TEXT NOT NULL,
                run_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                data TEXT NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS tool_calls (
                id TEXT PRIMARY KEY,
                trace_id TEXT NOT NULL,
                tool TEXT NOT NULL,
                input TEXT NOT NULL,
                output TEXT,
                success INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL
            );
            CREATE INDEX IF NOT EXISTS idx_events_trace ON core_events(trace_id);
            CREATE INDEX IF NOT EXISTS idx_events_session ON core_events(session_id);",
        )?;
        Ok(())
    }

    /// Record a core event.
    pub fn append_event(&self, event: &CoreEvent) -> Result<String> {
        self.conn.execute(
            "INSERT INTO core_events (id, trace_id, session_id, run_id, event_type, data, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                &event.id,
                &event.trace_id,
                &event.session_id,
                &event.run_id,
                &event.event_type,
                &serde_json::to_string(&event.data)?,
                &event.created_at,
            ),
        )?;
        Ok(event.id.clone())
    }

    /// Append a message to a session.
    pub fn append_message(&self, session_id: &str, role: &str, content: &str) -> Result<String> {
        // Ensure session exists.
        let session_count: i64 = self.conn.query_row(
            "SELECT COUNT(*) FROM sessions WHERE id = ?1",
            [session_id],
            |row| row.get(0),
        )?;
        if session_count == 0 {
            self.conn.execute(
                "INSERT OR IGNORE INTO sessions (id, workspace, started_at) VALUES (?1, '', ?2)",
                (session_id, &chrono::Utc::now().to_rfc3339()),
            )?;
        }

        let turn_id = uuid::Uuid::new_v4().to_string();
        let msg_id = uuid::Uuid::new_v4().to_string();

        // Get next turn index.
        let idx: i64 = self.conn.query_row(
            "SELECT COALESCE(MAX(idx), -1) + 1 FROM turns WHERE session_id = ?1",
            [session_id],
            |row| row.get(0),
        )?;

        self.conn.execute(
            "INSERT INTO turns (id, session_id, role, idx) VALUES (?1, ?2, ?3, ?4)",
            (&turn_id, session_id, role, idx),
        )?;

        self.conn.execute(
            "INSERT INTO messages (id, turn_id, content, created_at) VALUES (?1, ?2, ?3, ?4)",
            (&msg_id, &turn_id, content, &chrono::Utc::now().to_rfc3339()),
        )?;

        Ok(msg_id)
    }

    /// List events for a session, ordered by timestamp.
    pub fn list_session_events(&self, session_id: &str) -> Result<Vec<CoreEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, trace_id, session_id, run_id, event_type, data, created_at
             FROM core_events WHERE session_id = ?1 ORDER BY created_at ASC",
        )?;
        let events = stmt
            .query_map([session_id], |row| {
                let data_str: String = row.get(5)?;
                Ok(CoreEvent {
                    id: row.get(0)?,
                    trace_id: row.get(1)?,
                    session_id: row.get(2)?,
                    run_id: row.get(3)?,
                    event_type: row.get(4)?,
                    data: serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null),
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(events)
    }

    /// Reconstruct a timeline by trace_id.
    pub fn reconstruct_timeline(&self, trace_id: &str) -> Result<Vec<CoreEvent>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, trace_id, session_id, run_id, event_type, data, created_at
             FROM core_events WHERE trace_id = ?1 ORDER BY created_at ASC",
        )?;
        let events = stmt
            .query_map([trace_id], |row| {
                let data_str: String = row.get(5)?;
                Ok(CoreEvent {
                    id: row.get(0)?,
                    trace_id: row.get(1)?,
                    session_id: row.get(2)?,
                    run_id: row.get(3)?,
                    event_type: row.get(4)?,
                    data: serde_json::from_str(&data_str).unwrap_or(serde_json::Value::Null),
                    created_at: row.get(6)?,
                })
            })?
            .collect::<std::result::Result<Vec<_>, _>>()?;
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::episodic::types::CoreEvent;

    #[test]
    fn test_append_event_stores_in_sqlite() {
        let store = EpisodicStore::open_in_memory().unwrap();
        let event = CoreEvent::new(
            "trace-1",
            "sess-1",
            "run-1",
            "test_event",
            serde_json::json!({"key": "value"}),
        );
        let id = store.append_event(&event).unwrap();
        assert!(!id.is_empty());

        let events = store.list_session_events("sess-1").unwrap();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "test_event");
    }

    #[test]
    fn test_append_message_stores_in_sqlite() {
        let store = EpisodicStore::open_in_memory().unwrap();
        let id = store.append_message("sess-1", "user", "Hello").unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_list_session_events_returns_correct_order() {
        let store = EpisodicStore::open_in_memory().unwrap();
        store
            .append_event(&CoreEvent::new(
                "t1",
                "s1",
                "r1",
                "first",
                serde_json::json!({}),
            ))
            .unwrap();
        store
            .append_event(&CoreEvent::new(
                "t1",
                "s1",
                "r1",
                "second",
                serde_json::json!({}),
            ))
            .unwrap();

        let events = store.list_session_events("s1").unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "first");
        assert_eq!(events[1].event_type, "second");
    }

    #[test]
    fn test_reconstruct_timeline_by_trace_id() {
        let store = EpisodicStore::open_in_memory().unwrap();
        store
            .append_event(&CoreEvent::new(
                "trace-abc",
                "s1",
                "r1",
                "event_a",
                serde_json::json!({}),
            ))
            .unwrap();
        store
            .append_event(&CoreEvent::new(
                "trace-abc",
                "s1",
                "r1",
                "event_b",
                serde_json::json!({}),
            ))
            .unwrap();
        store
            .append_event(&CoreEvent::new(
                "trace-other",
                "s2",
                "r2",
                "event_c",
                serde_json::json!({}),
            ))
            .unwrap();

        let timeline = store.reconstruct_timeline("trace-abc").unwrap();
        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline[0].event_type, "event_a");
    }

    #[test]
    fn test_database_created_on_init() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("episodic.db");
        let _store = EpisodicStore::open(&path).unwrap();
        assert!(path.exists());
    }

    #[test]
    fn test_concurrent_appends_safe() {
        use std::sync::{Arc, Mutex};

        let store = Arc::new(Mutex::new(EpisodicStore::open_in_memory().unwrap()));
        let mut handles = vec![];

        for i in 0..10 {
            let s = Arc::clone(&store);
            handles.push(std::thread::spawn(move || {
                let s = s.lock().unwrap();
                s.append_event(&CoreEvent::new(
                    "t1",
                    "s1",
                    "r1",
                    format!("event_{}", i),
                    serde_json::json!({"idx": i}),
                ))
                .unwrap();
            }));
        }

        for h in handles {
            h.join().unwrap();
        }

        let s = store.lock().unwrap();
        let events = s.list_session_events("s1").unwrap();
        assert_eq!(events.len(), 10);
    }
}
