//! Hybrid retrieval engine combining all memory layers.

use anyhow::Result;

use crate::episodic::EpisodicStore;
use crate::procedural::ProceduralStore;
use crate::semantic::types::MemoryHit;
use crate::semantic::SemanticStore;

/// Hybrid retrieval engine that queries across all memory layers.
pub struct RetrievalEngine {
    _episodic: Option<EpisodicStore>,
    semantic: Option<SemanticStore>,
    procedural: Option<ProceduralStore>,
}

impl RetrievalEngine {
    /// Create a new retrieval engine with optional layers.
    pub fn new(
        episodic: Option<EpisodicStore>,
        semantic: Option<SemanticStore>,
        procedural: Option<ProceduralStore>,
    ) -> Self {
        Self {
            _episodic: episodic,
            semantic,
            procedural,
        }
    }

    /// Retrieve from all available layers and merge results by score.
    pub fn hybrid_retrieve(&self, query: &str, top_k: usize) -> Result<Vec<MemoryHit>> {
        let mut hits = Vec::new();

        // Semantic layer — facts and documents.
        if let Some(ref sem) = self.semantic {
            if let Ok(mut h) = sem.keyword_search(query, top_k) {
                hits.append(&mut h);
            }
        }

        // Procedural layer — matching procedures.
        if let Some(ref proc) = self.procedural {
            if let Ok(proc_hits) = proc.match_procedure(query, top_k) {
                for ph in proc_hits {
                    hits.push(MemoryHit {
                        content: format!(
                            "Procedure: {} — {}",
                            ph.procedure.name,
                            serde_json::to_string(&ph.procedure.steps).unwrap_or_default()
                        ),
                        score: ph.score,
                        memory_type: "procedural".to_string(),
                        source: Some(ph.procedure.id),
                    });
                }
            }
        }

        // Episodic layer — recent events matching query (basic keyword match).
        // This is a lightweight scan — full-text search would be more efficient.
        // For now, we skip episodic in hybrid retrieve since it's better suited
        // for timeline reconstruction via trace_id.

        // Sort by score descending.
        hits.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        hits.truncate(top_k);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_retrieve_from_all_layers() {
        let sem = SemanticStore::open_in_memory().unwrap();
        sem.upsert_fact("Rust is a systems programming language", "workspace")
            .unwrap();

        let proc = ProceduralStore::open_in_memory().unwrap();
        proc.upsert_procedure(
            "rust_build",
            serde_json::json!({"steps": ["cargo build"]}),
            "workspace",
        )
        .unwrap();

        let engine = RetrievalEngine::new(None, Some(sem), Some(proc));
        let hits = engine.hybrid_retrieve("rust", 5).unwrap();
        assert!(hits.len() >= 2);
        assert!(hits.iter().any(|h| h.memory_type == "semantic"));
        assert!(hits.iter().any(|h| h.memory_type == "procedural"));
    }

    #[test]
    fn test_retrieve_degrades_when_semantic_unavailable() {
        let proc = ProceduralStore::open_in_memory().unwrap();
        proc.upsert_procedure(
            "test_runner",
            serde_json::json!({"steps": ["cargo test"]}),
            "workspace",
        )
        .unwrap();

        let engine = RetrievalEngine::new(None, None, Some(proc));
        let hits = engine.hybrid_retrieve("test", 5).unwrap();
        assert!(!hits.is_empty());
        assert_eq!(hits[0].memory_type, "procedural");
    }

    #[test]
    fn test_retrieve_returns_score_and_type() {
        let sem = SemanticStore::open_in_memory().unwrap();
        sem.upsert_fact("Test content", "workspace").unwrap();

        let engine = RetrievalEngine::new(None, Some(sem), None);
        let hits = engine.hybrid_retrieve("test", 5).unwrap();
        assert!(!hits.is_empty());
        assert!(hits[0].score > 0.0);
        assert!(!hits[0].memory_type.is_empty());
    }

    #[test]
    fn test_retrieve_empty_when_nothing_available() {
        let engine: RetrievalEngine = RetrievalEngine::new(None, None, None);
        let hits = engine.hybrid_retrieve("anything", 5).unwrap();
        assert!(hits.is_empty());
    }
}
