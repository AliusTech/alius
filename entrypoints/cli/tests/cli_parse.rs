//! CLI parsing and help output tests.
//!
//! Verifies that all commands are correctly parsed and help text is displayed.

use std::process::Command;

fn alius_bin() -> Command {
    Command::new(env!("CARGO_BIN_EXE_alius"))
}

#[test]
fn version_outputs_version_string() {
    let output = alius_bin()
        .arg("version")
        .output()
        .expect("failed to execute alius version");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.starts_with("alius "),
        "expected version string starting with 'alius ', got: {stdout}"
    );
}

#[test]
fn root_help_shows_usage() {
    let output = alius_bin()
        .arg("--help")
        .output()
        .expect("failed to execute alius --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage:") || stdout.contains("usage:"));
    assert!(stdout.contains("Commands:") || stdout.contains("commands:"));
}

#[test]
fn subcommand_help_shows_usage() {
    for subcmd in &[
        "config", "core", "soul", "plugin", "mcp", "workflow", "update",
    ] {
        let output = alius_bin()
            .args([subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius {subcmd} --help exited with non-zero status"
        );
        let stdout = String::from_utf8_lossy(&output.stdout);
        assert!(
            !stdout.trim().is_empty(),
            "alius {subcmd} --help produced empty output"
        );
    }
}

#[test]
fn invalid_subcommand_fails() {
    let output = alius_bin()
        .arg("nonexistent-command")
        .output()
        .expect("failed to execute alius with invalid subcommand");

    assert!(!output.status.success());
}

#[test]
fn config_subcommands_exist() {
    for subcmd in &["show", "validate"] {
        let output = alius_bin()
            .args(["config", subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius config {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius config {subcmd} --help exited with non-zero status"
        );
    }
}

#[test]
fn plugin_subcommands_exist() {
    for subcmd in &["list", "install", "info", "remove"] {
        let output = alius_bin()
            .args(["plugin", subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius plugin {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius plugin {subcmd} --help exited with non-zero status"
        );
    }
}

#[test]
fn mcp_subcommands_exist() {
    for subcmd in &["list", "start", "tools"] {
        let output = alius_bin()
            .args(["mcp", subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius mcp {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius mcp {subcmd} --help exited with non-zero status"
        );
    }
}

#[test]
fn workflow_subcommands_exist() {
    for subcmd in &["list", "run", "validate"] {
        let output = alius_bin()
            .args(["workflow", subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius workflow {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius workflow {subcmd} --help exited with non-zero status"
        );
    }
}

#[test]
fn soul_subcommands_exist() {
    for subcmd in &["update", "list", "install", "current", "remove"] {
        let output = alius_bin()
            .args(["soul", subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius soul {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius soul {subcmd} --help exited with non-zero status"
        );
    }
}

#[test]
fn core_subcommands_exist() {
    for subcmd in &["update", "list", "info"] {
        let output = alius_bin()
            .args(["core", subcmd, "--help"])
            .output()
            .unwrap_or_else(|e| panic!("failed to execute alius core {subcmd} --help: {e}"));

        assert!(
            output.status.success(),
            "alius core {subcmd} --help exited with non-zero status"
        );
    }
}

// ============================================================================
// Root flags tests
// ============================================================================

#[test]
fn root_help_shows_model_flag() {
    let output = alius_bin()
        .arg("--help")
        .output()
        .expect("failed to execute alius --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--model") || stdout.contains("-m"),
        "help should mention --model flag"
    );
}

#[test]
fn root_help_shows_provider_flag() {
    let output = alius_bin()
        .arg("--help")
        .output()
        .expect("failed to execute alius --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--provider") || stdout.contains("-p"),
        "help should mention --provider flag"
    );
}

#[test]
fn root_help_shows_workspace_flag() {
    let output = alius_bin()
        .arg("--help")
        .output()
        .expect("failed to execute alius --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--workspace"),
        "help should mention --workspace flag"
    );
}

#[test]
fn root_help_shows_config_flag() {
    let output = alius_bin()
        .arg("--help")
        .output()
        .expect("failed to execute alius --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--config") || stdout.contains("-c"),
        "help should mention --config flag"
    );
}

#[test]
fn root_help_shows_verbose_flag() {
    let output = alius_bin()
        .arg("--help")
        .output()
        .expect("failed to execute alius --help");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("--verbose") || stdout.contains("-v"),
        "help should mention --verbose flag"
    );
}
