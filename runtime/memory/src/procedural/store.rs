//! SQLite-backed procedural memory store.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;

use super::types::{FailurePattern, Procedure, ProcedureHit};

/// SQLite-backed procedural memory store.
pub struct ProceduralStore {
    conn: Connection,
}

impl ProceduralStore {
    /// Open (or create) a procedural store at the given database path.
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
            "CREATE TABLE IF NOT EXISTS procedures (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                steps TEXT NOT NULL,
                scope TEXT NOT NULL DEFAULT 'workspace',
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS failure_patterns (
                id TEXT PRIMARY KEY,
                symptoms TEXT NOT NULL,
                resolution TEXT NOT NULL,
                created_at TEXT NOT NULL
            );",
        )?;
        Ok(())
    }

    /// Insert or update a procedure.
    pub fn upsert_procedure(
        &self,
        name: &str,
        steps: serde_json::Value,
        scope: &str,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.conn.execute(
            "INSERT INTO procedures (id, name, steps, scope, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            (&id, name, &serde_json::to_string(&steps)?, scope, &now),
        )?;
        Ok(id)
    }

    /// Match procedures by context keyword.
    pub fn match_procedure(&self, context: &str, top_k: usize) -> Result<Vec<ProcedureHit>> {
        let pattern = format!("%{}%", context.to_lowercase());
        let mut stmt = self.conn.prepare(
            "SELECT id, name, steps, scope, created_at FROM procedures WHERE LOWER(name) LIKE ?1",
        )?;
        let rows = stmt.query_map([&pattern], |row| {
            let steps_str: String = row.get(2)?;
            Ok(Procedure {
                id: row.get(0)?,
                name: row.get(1)?,
                steps: serde_json::from_str(&steps_str).unwrap_or(serde_json::Value::Null),
                scope: row.get(3)?,
                created_at: row.get(4)?,
            })
        })?;

        let mut hits: Vec<ProcedureHit> = rows
            .filter_map(|r| r.ok())
            .map(|p| ProcedureHit {
                score: 0.8,
                procedure: p,
            })
            .collect();

        hits.truncate(top_k);
        Ok(hits)
    }

    /// Record a failure pattern.
    pub fn record_failure_pattern(
        &self,
        symptoms: serde_json::Value,
        resolution: &str,
    ) -> Result<String> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        self.conn.execute(
            "INSERT INTO failure_patterns (id, symptoms, resolution, created_at) VALUES (?1, ?2, ?3, ?4)",
            (&id, &serde_json::to_string(&symptoms)?, resolution, &now),
        )?;
        Ok(id)
    }

    /// List failure patterns.
    pub fn list_failure_patterns(&self) -> Result<Vec<FailurePattern>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, symptoms, resolution, created_at FROM failure_patterns ORDER BY created_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let symptoms_str: String = row.get(1)?;
            Ok(FailurePattern {
                id: row.get(0)?,
                symptoms: serde_json::from_str(&symptoms_str).unwrap_or(serde_json::Value::Null),
                resolution: row.get(2)?,
                created_at: row.get(3)?,
            })
        })?;
        rows.collect::<std::result::Result<Vec<_>, _>>()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_procedure_stores() {
        let store = ProceduralStore::open_in_memory().unwrap();
        let id = store
            .upsert_procedure(
                "deploy_to_staging",
                serde_json::json!({"steps": ["build", "test", "deploy"]}),
                "workspace",
            )
            .unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn test_match_procedure_by_context() {
        let store = ProceduralStore::open_in_memory().unwrap();
        store
            .upsert_procedure(
                "deploy_to_staging",
                serde_json::json!({"steps": ["build", "test"]}),
                "workspace",
            )
            .unwrap();
        store
            .upsert_procedure(
                "run_tests",
                serde_json::json!({"steps": ["cargo test"]}),
                "workspace",
            )
            .unwrap();

        let hits = store.match_procedure("deploy", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].procedure.name, "deploy_to_staging");
    }

    #[test]
    fn test_record_failure_pattern() {
        let store = ProceduralStore::open_in_memory().unwrap();
        let id = store
            .record_failure_pattern(
                serde_json::json!({"error": "connection refused", "port": 5432}),
                "Start postgres service before running tests",
            )
            .unwrap();
        assert!(!id.is_empty());

        let patterns = store.list_failure_patterns().unwrap();
        assert_eq!(patterns.len(), 1);
        assert!(patterns[0].resolution.contains("postgres"));
    }

    #[test]
    fn test_procedure_has_applicable_scope() {
        let store = ProceduralStore::open_in_memory().unwrap();
        store
            .upsert_procedure("global_setup", serde_json::json!({"steps": []}), "global")
            .unwrap();

        let hits = store.match_procedure("setup", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].procedure.scope, "global");
    }
}
