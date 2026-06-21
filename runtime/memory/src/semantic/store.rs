//! SQLite-backed semantic memory store.

use anyhow::Result;
use rusqlite::Connection;
use std::path::Path;
use std::sync::Mutex;

use super::types::MemoryHit;

/// SQLite-backed semantic memory store.
///
/// Uses `Mutex<Connection>` to ensure `Send + Sync` for use with `Arc`.
pub struct SemanticStore {
    conn: Mutex<Connection>,
}

impl SemanticStore {
    /// Open (or create) a semantic store at the given database path.
    pub fn open(db_path: &Path) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    /// Open an in-memory store (for testing).
    pub fn open_in_memory() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS facts (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                scope TEXT NOT NULL DEFAULT 'workspace',
                confidence REAL NOT NULL DEFAULT 0.5,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS documents (
                id TEXT PRIMARY KEY,
                path TEXT NOT NULL,
                indexed_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS chunks (
                id TEXT PRIMARY KEY,
                document_id TEXT NOT NULL,
                content TEXT NOT NULL,
                chunk_idx INTEGER NOT NULL,
                FOREIGN KEY (document_id) REFERENCES documents(id)
            );",
        )?;
        Ok(())
    }

    /// Insert or update a fact.
    pub fn upsert_fact(&self, content: &str, scope: &str) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        conn.execute(
            "INSERT INTO facts (id, content, scope, confidence, created_at) VALUES (?1, ?2, ?3, 0.5, ?4)",
            (&id, content, scope, &now),
        )?;
        Ok(id)
    }

    /// Index a document by splitting it into chunks.
    pub fn index_document(&self, path: &Path) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let content = std::fs::read_to_string(path)?;
        let doc_id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);

        conn.execute(
            "INSERT INTO documents (id, path, indexed_at) VALUES (?1, ?2, ?3)",
            (&doc_id, path.to_string_lossy().to_string(), &now),
        )?;

        // Simple chunking: split by paragraphs, ~500 chars per chunk.
        let chunks: Vec<&str> = content.split("\n\n").collect();
        let mut current_chunk = String::new();
        let mut chunk_idx = 0;

        for paragraph in chunks {
            if current_chunk.len() + paragraph.len() > 500 && !current_chunk.is_empty() {
                let chunk_id = uuid::Uuid::new_v4().to_string();
                conn.execute(
                    "INSERT INTO chunks (id, document_id, content, chunk_idx) VALUES (?1, ?2, ?3, ?4)",
                    (&chunk_id, &doc_id, &current_chunk, chunk_idx),
                )?;
                chunk_idx += 1;
                current_chunk = paragraph.to_string();
            } else {
                if !current_chunk.is_empty() {
                    current_chunk.push_str("\n\n");
                }
                current_chunk.push_str(paragraph);
            }
        }

        if !current_chunk.is_empty() {
            let chunk_id = uuid::Uuid::new_v4().to_string();
            conn.execute(
                "INSERT INTO chunks (id, document_id, content, chunk_idx) VALUES (?1, ?2, ?3, ?4)",
                (&chunk_id, &doc_id, &current_chunk, chunk_idx),
            )?;
        }

        Ok(doc_id)
    }

    /// Keyword search across facts and chunks.
    pub fn keyword_search(&self, query: &str, top_k: usize) -> Result<Vec<MemoryHit>> {
        let conn = self.conn.lock().unwrap();
        let pattern = format!("%{}%", query.to_lowercase());
        let mut hits = Vec::new();

        // Search facts.
        let mut stmt =
            conn.prepare("SELECT content, scope FROM facts WHERE LOWER(content) LIKE ?1")?;
        let fact_rows = stmt.query_map([&pattern], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in fact_rows {
            let (content, _scope) = row?;
            hits.push(MemoryHit {
                content,
                score: 0.7,
                memory_type: "semantic".to_string(),
                source: None,
            });
        }

        // Search chunks.
        let mut stmt2 = conn.prepare("SELECT content FROM chunks WHERE LOWER(content) LIKE ?1")?;
        let chunk_rows = stmt2.query_map([&pattern], |row| row.get::<_, String>(0))?;
        for row in chunk_rows {
            let content = row?;
            hits.push(MemoryHit {
                content,
                score: 0.5,
                memory_type: "semantic".to_string(),
                source: None,
            });
        }

        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        Ok(hits)
    }

    /// Semantic search — degrades to keyword search when embeddings are unavailable.
    pub fn semantic_search(&self, query: &str, top_k: usize) -> Result<Vec<MemoryHit>> {
        // Placeholder: embedding model not yet integrated.
        // Degrade gracefully to keyword search.
        self.keyword_search(query, top_k)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_upsert_fact_stores_in_sqlite() {
        let store = SemanticStore::open_in_memory().unwrap();
        let id = store
            .upsert_fact("Rust uses ownership for memory management", "workspace")
            .unwrap();
        assert!(!id.is_empty());

        let hits = store.keyword_search("ownership", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("ownership"));
    }

    #[test]
    fn test_keyword_search_returns_relevant() {
        let store = SemanticStore::open_in_memory().unwrap();
        store
            .upsert_fact("The project uses React for frontend", "workspace")
            .unwrap();
        store
            .upsert_fact("Rust backend with Axum framework", "workspace")
            .unwrap();

        let hits = store.keyword_search("rust", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert!(hits[0].content.contains("Rust"));
    }

    #[test]
    fn test_index_document_creates_chunks() {
        let dir = tempfile::tempdir().unwrap();
        let doc_path = dir.path().join("test.md");
        std::fs::write(
            &doc_path,
            "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.",
        )
        .unwrap();

        let store = SemanticStore::open_in_memory().unwrap();
        let id = store.index_document(&doc_path).unwrap();
        assert!(!id.is_empty());

        let hits = store.keyword_search("first", 5).unwrap();
        assert!(!hits.is_empty());
    }

    #[test]
    fn test_semantic_search_degrades_to_keyword() {
        let store = SemanticStore::open_in_memory().unwrap();
        store
            .upsert_fact("Test fact about databases", "workspace")
            .unwrap();

        let hits = store.semantic_search("database", 5).unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory_type, "semantic");
    }

    #[test]
    fn test_fact_scope_filtering() {
        let store = SemanticStore::open_in_memory().unwrap();
        store.upsert_fact("Global knowledge", "global").unwrap();
        store
            .upsert_fact("Workspace knowledge", "workspace")
            .unwrap();

        // Both should match in keyword search regardless of scope.
        let hits = store.keyword_search("knowledge", 5).unwrap();
        assert_eq!(hits.len(), 2);
    }
}
