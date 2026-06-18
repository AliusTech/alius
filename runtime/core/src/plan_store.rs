//! Plan Store — persistent plan node storage.
//!
//! Provides a trait-based abstraction for storing and retrieving plan nodes.
//! The file-backed implementation persists plans as JSON in the workspace.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

/// Plan node status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanNodeStatus {
    Pending,
    Running,
    Completed,
    Review,
    Approved,
    Revising,
    Failed,
    Blocked,
    Cancelled,
}

/// A single plan node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: String,
    pub title: String,
    pub status: PlanNodeStatus,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub acceptance_criteria: Vec<String>,
    #[serde(default)]
    pub evidence: Vec<String>,
    /// Reserved for AgentNet / Agent Team coordination.
    #[serde(default)]
    pub owner: Option<String>,
}

/// Plan store trait — abstracts plan persistence.
pub trait PlanStore: Send + Sync {
    /// Load all plan nodes.
    fn load(&self) -> Result<Vec<PlanNode>>;

    /// Save a single plan node (upsert by id).
    fn save(&self, node: &PlanNode) -> Result<()>;

    /// Delete a plan node by id.
    fn delete(&self, id: &str) -> Result<()>;

    /// Update the status of a plan node.
    fn update_status(&self, id: &str, status: PlanNodeStatus) -> Result<()>;

    /// Add evidence to a plan node.
    fn add_evidence(&self, id: &str, evidence: &str) -> Result<()>;
}

/// File-backed plan store.
///
/// Persists plan nodes as a JSON array in `<workspace>/.alius/plans.json`.
pub struct FilePlanStore {
    path: PathBuf,
    cache: RwLock<Vec<PlanNode>>,
}

impl FilePlanStore {
    /// Create a new file-backed plan store.
    pub fn new(workspace_root: &Path) -> Self {
        let path = workspace_root.join(".alius/plans.json");
        let cache = RwLock::new(Vec::new());
        let store = Self { path, cache };
        // Load existing plans on creation
        if let Ok(nodes) = store.load_from_disk() {
            *store.cache.write().unwrap() = nodes;
        }
        store
    }

    /// Load plans from disk.
    fn load_from_disk(&self) -> Result<Vec<PlanNode>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.path)?;
        let nodes: Vec<PlanNode> = serde_json::from_str(&content)?;
        Ok(nodes)
    }

    /// Persist plans to disk.
    fn persist(&self, nodes: &[PlanNode]) -> Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content = serde_json::to_string_pretty(nodes)?;
        std::fs::write(&self.path, content)?;
        Ok(())
    }
}

impl PlanStore for FilePlanStore {
    fn load(&self) -> Result<Vec<PlanNode>> {
        let nodes = self.cache.read().unwrap().clone();
        Ok(nodes)
    }

    fn save(&self, node: &PlanNode) -> Result<()> {
        let mut nodes = self.cache.write().unwrap();
        if let Some(existing) = nodes.iter_mut().find(|n| n.id == node.id) {
            *existing = node.clone();
        } else {
            nodes.push(node.clone());
        }
        self.persist(&nodes)
    }

    fn delete(&self, id: &str) -> Result<()> {
        let mut nodes = self.cache.write().unwrap();
        nodes.retain(|n| n.id != id);
        self.persist(&nodes)
    }

    fn update_status(&self, id: &str, status: PlanNodeStatus) -> Result<()> {
        let mut nodes = self.cache.write().unwrap();
        if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
            node.status = status;
            self.persist(&nodes)?;
        }
        Ok(())
    }

    fn add_evidence(&self, id: &str, evidence: &str) -> Result<()> {
        let mut nodes = self.cache.write().unwrap();
        if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
            node.evidence.push(evidence.to_string());
            self.persist(&nodes)?;
        }
        Ok(())
    }
}

/// In-memory plan store for testing.
pub struct InMemoryPlanStore {
    nodes: RwLock<Vec<PlanNode>>,
}

impl InMemoryPlanStore {
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(Vec::new()),
        }
    }
}

impl Default for InMemoryPlanStore {
    fn default() -> Self {
        Self::new()
    }
}

impl PlanStore for InMemoryPlanStore {
    fn load(&self) -> Result<Vec<PlanNode>> {
        Ok(self.nodes.read().unwrap().clone())
    }

    fn save(&self, node: &PlanNode) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        if let Some(existing) = nodes.iter_mut().find(|n| n.id == node.id) {
            *existing = node.clone();
        } else {
            nodes.push(node.clone());
        }
        Ok(())
    }

    fn delete(&self, id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        nodes.retain(|n| n.id != id);
        Ok(())
    }

    fn update_status(&self, id: &str, status: PlanNodeStatus) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
            node.status = status;
        }
        Ok(())
    }

    fn add_evidence(&self, id: &str, evidence: &str) -> Result<()> {
        let mut nodes = self.nodes.write().unwrap();
        if let Some(node) = nodes.iter_mut().find(|n| n.id == id) {
            node.evidence.push(evidence.to_string());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_in_memory_save_and_load() {
        let store = InMemoryPlanStore::new();
        let node = PlanNode {
            id: "p1".to_string(),
            title: "Test Plan".to_string(),
            status: PlanNodeStatus::Pending,
            description: None,
            acceptance_criteria: vec![],
            evidence: vec![],
            owner: None,
        };
        store.save(&node).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "p1");
    }

    #[test]
    fn test_in_memory_update_status() {
        let store = InMemoryPlanStore::new();
        let node = PlanNode {
            id: "p1".to_string(),
            title: "Test".to_string(),
            status: PlanNodeStatus::Pending,
            description: None,
            acceptance_criteria: vec![],
            evidence: vec![],
            owner: None,
        };
        store.save(&node).unwrap();
        store.update_status("p1", PlanNodeStatus::Running).unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].status, PlanNodeStatus::Running);
    }

    #[test]
    fn test_in_memory_delete() {
        let store = InMemoryPlanStore::new();
        let node = PlanNode {
            id: "p1".to_string(),
            title: "Test".to_string(),
            status: PlanNodeStatus::Pending,
            description: None,
            acceptance_criteria: vec![],
            evidence: vec![],
            owner: None,
        };
        store.save(&node).unwrap();
        store.delete("p1").unwrap();
        assert!(store.load().unwrap().is_empty());
    }

    #[test]
    fn test_in_memory_add_evidence() {
        let store = InMemoryPlanStore::new();
        let node = PlanNode {
            id: "p1".to_string(),
            title: "Test".to_string(),
            status: PlanNodeStatus::Pending,
            description: None,
            acceptance_criteria: vec![],
            evidence: vec![],
            owner: None,
        };
        store.save(&node).unwrap();
        store.add_evidence("p1", "step 1 done").unwrap();
        store.add_evidence("p1", "step 2 done").unwrap();
        let loaded = store.load().unwrap();
        assert_eq!(loaded[0].evidence.len(), 2);
    }

    #[test]
    fn test_file_plan_store_persistence() {
        let dir = std::env::temp_dir().join(format!("alius_plan_test_{}", std::process::id()));
        std::fs::create_dir_all(dir.join(".alius")).unwrap();

        let node = PlanNode {
            id: "p1".to_string(),
            title: "Persistent Plan".to_string(),
            status: PlanNodeStatus::Pending,
            description: Some("test".to_string()),
            acceptance_criteria: vec!["criterion 1".to_string()],
            evidence: vec![],
            owner: Some("coder-agent".to_string()),
        };

        {
            let store = FilePlanStore::new(&dir);
            store.save(&node).unwrap();
        }

        // Reload from disk
        let store = FilePlanStore::new(&dir);
        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].id, "p1");
        assert_eq!(loaded[0].title, "Persistent Plan");
        assert_eq!(loaded[0].owner, Some("coder-agent".to_string()));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
