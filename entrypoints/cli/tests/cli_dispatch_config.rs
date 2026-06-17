//! CLI config command dispatch tests.
//!
//! Verifies that config show/validate work correctly with isolated HOME/workspace.

use std::process::Command;
use tempfile::TempDir;

fn alius_in_dir(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_alius"));
    cmd.current_dir(dir);
    // Isolate from user's real HOME
    cmd.env("HOME", dir);
    cmd.env("XDG_CONFIG_HOME", dir.join(".config"));
    cmd
}

#[test]
fn config_show_succeeds_with_default_settings() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["config", "show"])
        .output()
        .expect("failed to execute alius config show");

    // config show should succeed even without a project config
    assert!(
        output.status.success(),
        "config show failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn config_validate_reports_status() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["config", "validate"])
        .output()
        .expect("failed to execute alius config validate");

    // validate should produce output (either success or validation errors)
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.trim().is_empty(),
        "config validate produced no output"
    );
}

#[test]
fn config_soul_requires_role_argument() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["config", "soul"])
        .output()
        .expect("failed to execute alius config soul");

    // Should fail without the --role argument
    assert!(
        !output.status.success(),
        "config soul without --role should fail"
    );
}

#[test]
fn config_credential_check_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["config", "credential", "check"])
        .output()
        .expect("failed to execute alius config credential check");

    // credential check should succeed (reports keyring availability)
    assert!(
        output.status.success(),
        "config credential check failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
