//! Test utilities for the Alius CLI.
//!
//! Provides shared helpers for CLI integration tests and TUI state-machine tests.

#![allow(dead_code)]

use std::path::PathBuf;
use tempfile::TempDir;

/// RAII guard that restores the original working directory on drop.
///
/// Created by [`enter_temp_cwd`]. Ensures tests that change the process
/// working directory don't interfere with each other.
pub struct CwdGuard {
    original: PathBuf,
}

impl Drop for CwdGuard {
    fn drop(&mut self) {
        let _ = std::env::set_current_dir(&self.original);
    }
}

/// Create a temporary directory and change the process working directory into it.
///
/// Returns the `TempDir` (keep it alive!) and a `CwdGuard` that restores
/// the original directory on drop.
///
/// # Examples
///
/// ```ignore
/// let (_temp, _guard) = enter_temp_cwd();
/// // Current directory is now a fresh temp dir.
/// // Both _temp and _guard are dropped at end of scope.
/// ```
pub fn enter_temp_cwd() -> (TempDir, CwdGuard) {
    let original = std::env::current_dir().expect("failed to get current dir");
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    std::env::set_current_dir(temp_dir.path()).expect("failed to change to temp dir");
    let guard = CwdGuard { original };
    (temp_dir, guard)
}

/// Create a temporary directory with a `.alius/` project structure.
///
/// Useful for tests that need an initialized workspace.
pub fn temp_workspace() -> TempDir {
    let temp_dir = TempDir::new().expect("failed to create temp dir");
    let alius_dir = temp_dir.path().join(".alius");
    std::fs::create_dir_all(alius_dir.join("config")).expect("failed to create .alius/config");
    std::fs::create_dir_all(alius_dir.join("runtime")).expect("failed to create .alius/runtime");
    temp_dir
}

/// Set the i18n locale to "en" for test determinism.
///
/// Call this at the start of any test that asserts on translated strings.
pub fn set_test_locale() {
    rust_i18n::set_locale("en");
}
