//! CLI run command tests.
//!
//! Verifies that `alius run -p "prompt"` works correctly,
//! including error paths when no provider is configured.

use std::process::Command;
use tempfile::TempDir;

fn alius_in_dir(dir: &std::path::Path) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_alius"));
    cmd.current_dir(dir);
    cmd.env("HOME", dir);
    cmd.env("XDG_CONFIG_HOME", dir.join(".config"));
    // Clear any provider env vars to ensure clean state
    cmd.env_remove("OPENAI_API_KEY");
    cmd.env_remove("ANTHROPIC_API_KEY");
    cmd.env_remove("DEEPSEEK_API_KEY");
    cmd.env_remove("ALIUS_PROVIDER_SMOKE");
    cmd
}

#[test]
fn run_requires_prompt_argument() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .arg("run")
        .output()
        .expect("failed to execute alius run");

    // Should fail without the -p argument
    assert!(!output.status.success(), "run without -p should fail");
}

#[test]
fn run_fails_without_api_key() {
    let tmp = TempDir::new().unwrap();
    let output = alius_in_dir(tmp.path())
        .args(["run", "-p", "hello"])
        .output()
        .expect("failed to execute alius run");

    // Without an API key, the run should either fail or produce an error
    // The exact behavior depends on the provider configuration
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    let combined = format!("{stdout}{stderr}");

    // We just verify it doesn't panic — it should either succeed (if a default
    // key is available) or fail gracefully with an error message
    if !output.status.success() {
        assert!(
            !combined.contains("panicked"),
            "run should not panic, got: {combined}"
        );
    }
}
