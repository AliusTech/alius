//! Conflict detection for patches.

use super::diff::FileChange;
use std::path::Path;

/// Result of conflict detection.
#[derive(Debug, Clone)]
pub struct ConflictReport {
    /// Whether conflicts were detected.
    pub has_conflicts: bool,
    /// Conflicting file paths.
    pub conflicting_paths: Vec<String>,
}

/// Detect conflicts between a patch and the current state.
///
/// A conflict occurs when the patch tries to modify a file that has
/// been changed since the patch was created.
pub fn detect_conflicts(
    changes: &[FileChange],
    project_root: &Path,
    current_hashes: &std::collections::HashMap<String, String>,
) -> ConflictReport {
    let mut conflicting = Vec::new();

    for change in changes {
        if change.change_type == "modify" || change.change_type == "delete" {
            let path = project_root.join(&change.path);
            if let Some(expected_hash) = current_hashes.get(&change.path) {
                if path.exists() {
                    let actual = simple_file_hash(&path);
                    if let Some(actual) = actual {
                        if actual != *expected_hash {
                            conflicting.push(change.path.clone());
                        }
                    }
                }
            }
        }
    }

    ConflictReport {
        has_conflicts: !conflicting.is_empty(),
        conflicting_paths: conflicting,
    }
}

fn simple_file_hash(path: &Path) -> Option<String> {
    use std::hash::{Hash, Hasher};
    let content = std::fs::read_to_string(path).ok()?;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    content.hash(&mut hasher);
    Some(format!("{:016x}", hasher.finish()))
}
