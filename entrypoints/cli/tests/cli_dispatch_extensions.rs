//! CLI extension command dispatch tests.
//!
//! Verifies that core, soul, plugin, mcp, and workflow commands
//! work correctly with isolated HOME/workspace.

use std::process::Command;
use tempfile::TempDir;

fn alius_in_dir(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_alius"));
    cmd.current_dir(dir);
    cmd.env("HOME", dir);
    cmd.env("XDG_CONFIG_HOME", dir.join(".config"));
    cmd
}

// --- Soul commands ---

#[test]
fn soul_list_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["soul", "list"])
        .output()
        .expect("failed to execute alius soul list");

    assert!(
        output.status.success(),
        "soul list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn soul_current_reports_status() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["soul", "current"])
        .output()
        .expect("failed to execute alius soul current");

    // Should succeed (reports current soul or "none")
    assert!(
        output.status.success(),
        "soul current failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn soul_install_requires_id() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["soul", "install"])
        .output()
        .expect("failed to execute alius soul install");

    // Should fail without the soul ID argument
    assert!(
        !output.status.success(),
        "soul install without ID should fail"
    );
}

// --- Core commands ---

#[test]
fn core_list_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["core", "list"])
        .output()
        .expect("failed to execute alius core list");

    assert!(
        output.status.success(),
        "core list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

// --- Plugin commands ---

#[test]
fn plugin_list_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["plugin", "list"])
        .output()
        .expect("failed to execute alius plugin list");

    assert!(
        output.status.success(),
        "plugin list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn plugin_install_requires_path() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["plugin", "install"])
        .output()
        .expect("failed to execute alius plugin install");

    // Should fail without the path argument
    assert!(
        !output.status.success(),
        "plugin install without path should fail"
    );
}

#[test]
fn plugin_install_rejects_nonexistent_path() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["plugin", "install", "/nonexistent/path"])
        .output()
        .expect("failed to execute alius plugin install");

    assert!(
        !output.status.success(),
        "plugin install with nonexistent path should fail"
    );
}

// --- MCP commands ---

#[test]
fn mcp_list_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["mcp", "list"])
        .output()
        .expect("failed to execute alius mcp list");

    assert!(
        output.status.success(),
        "mcp list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn mcp_start_requires_name() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["mcp", "start"])
        .output()
        .expect("failed to execute alius mcp start");

    // Should fail without the server name argument
    assert!(
        !output.status.success(),
        "mcp start without name should fail"
    );
}

#[test]
fn mcp_tools_requires_name() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["mcp", "tools"])
        .output()
        .expect("failed to execute alius mcp tools");

    // Should fail without the server name argument
    assert!(
        !output.status.success(),
        "mcp tools without name should fail"
    );
}

// --- Workflow commands ---

#[test]
fn workflow_list_succeeds() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["workflow", "list"])
        .output()
        .expect("failed to execute alius workflow list");

    assert!(
        output.status.success(),
        "workflow list failed: stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn workflow_validate_requires_path() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["workflow", "validate"])
        .output()
        .expect("failed to execute alius workflow validate");

    // Should fail without the path argument
    assert!(
        !output.status.success(),
        "workflow validate without path should fail"
    );
}

#[test]
fn workflow_validate_rejects_nonexistent_file() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["workflow", "validate", "/nonexistent/workflow.json"])
        .output()
        .expect("failed to execute alius workflow validate");

    // Handler prints "Invalid workflow" but returns Ok(())
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Invalid") || stdout.contains("invalid") || !output.status.success(),
        "workflow validate should report invalid for nonexistent file, got: {stdout}"
    );
}

#[test]
fn workflow_run_requires_name() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["workflow", "run"])
        .output()
        .expect("failed to execute alius workflow run");

    // Should fail without the workflow name argument
    assert!(
        !output.status.success(),
        "workflow run without name should fail"
    );
}
