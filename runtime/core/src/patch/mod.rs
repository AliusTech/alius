//! Patch management — diff generation, conflict detection, backup and rollback.

pub mod backup;
pub mod conflict;
pub mod diff;

pub use backup::{create_backup, restore_backup, BackupRef};
pub use conflict::detect_conflicts;
pub use diff::{create_patch, FileChange, Patch};
