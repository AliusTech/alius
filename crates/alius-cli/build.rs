//! Build script to embed the release version from CI or the current git tag.
//!
//! This script runs during `cargo build` and sets the `ALIUS_VERSION` compile-time
//! environment variable. The version is resolved from multiple sources in order:
//!
//! 1. `ALIUS_VERSION` env var (set by CI or manually)
//! 2. `GITHUB_REF_NAME` env var (set by GitHub Actions, e.g., "v0.0.3")
//! 3. `GITHUB_REF` env var (set by GitHub Actions, e.g., "refs/tags/v0.0.3")
//! 4. Git tag from `git describe --tags --exact-match`
//! 5. `CARGO_PKG_VERSION` from Cargo.toml
//! 6. "0.0.0" (ultimate fallback)
//!
//! The version is normalized by stripping "refs/tags/" and "v" prefixes,
//! and validated to be strict semver (major.minor.patch).

use std::env;
use std::process::Command;

fn main() {
    // Resolve version from the first available source
    let version = env_version("ALIUS_VERSION")
        .or_else(|| env_version("GITHUB_REF_NAME"))
        .or_else(|| env_version("GITHUB_REF"))
        .or_else(version_from_git_tag)
        .or_else(|| env::var("CARGO_PKG_VERSION").ok())
        .unwrap_or_else(|| "0.0.0".to_string());

    // Embed the version as a compile-time environment variable
    println!("cargo:rustc-env=ALIUS_VERSION={}", version);

    // Re-run the build script when these environment variables change
    println!("cargo:rerun-if-env-changed=ALIUS_VERSION");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-env-changed=GITHUB_REF");

    // Re-run when git state changes (new tags, HEAD movement)
    if let Some(git_dir) = git_dir() {
        println!("cargo:rerun-if-changed={}/HEAD", git_dir);
        println!("cargo:rerun-if-changed={}/refs/tags", git_dir);
    }
}

/// Read an environment variable and normalize it to a semver version.
///
/// Returns `None` if the variable is not set or the value is not valid semver.
fn env_version(name: &str) -> Option<String> {
    env::var(name).ok().and_then(normalize_version)
}

/// Extract a version from the current git tag using `git describe`.
///
/// Looks for an exact tag match on the current commit that matches the
/// pattern `v[0-9]*` (e.g., "v0.0.3"). Returns `None` if no matching tag exists.
fn version_from_git_tag() -> Option<String> {
    let output = Command::new("git")
        .args(["describe", "--tags", "--exact-match", "--match", "v[0-9]*"])
        .output()
        .ok()?;

    if !output.status.success() {
        return None;
    }

    normalize_version(String::from_utf8(output.stdout).ok()?)
}

/// Get the path to the `.git` directory for change detection.
///
/// Used to set up `cargo:rerun-if-changed` directives so the build script
/// re-runs when new tags are added or HEAD moves.
fn git_dir() -> Option<String> {
    let output = Command::new("git")
        .args(["rev-parse", "--git-dir"])
        .output()
        .ok()?;

    if output.status.success() {
        Some(String::from_utf8(output.stdout).ok()?.trim().to_string())
    } else {
        None
    }
}

/// Normalize a raw version string to strict semver format.
///
/// Strips:
/// - "refs/tags/" prefix (from git refs like "refs/tags/v0.0.3")
/// - "v" prefix (from tag names like "v0.0.3")
/// - Leading/trailing whitespace
///
/// Returns `None` if the result is not valid semver (major.minor.patch).
fn normalize_version(raw: String) -> Option<String> {
    let trimmed = raw.trim();
    let without_ref = trimmed.strip_prefix("refs/tags/").unwrap_or(trimmed);
    let version = without_ref.strip_prefix('v').unwrap_or(without_ref).trim();

    if is_semver(version) {
        Some(version.to_string())
    } else {
        None
    }
}

/// Validate that a string is strict semver format (major.minor.patch).
///
/// Accepts versions with optional pre-release or build metadata suffixes
/// (e.g., "1.0.0-beta.1+build.123"), but only validates the core version part.
fn is_semver(version: &str) -> bool {
    let core = version.split(['-', '+']).next().unwrap_or(version);
    let mut parts = core.split('.');

    matches!(
        (parts.next(), parts.next(), parts.next(), parts.next()),
        (Some(major), Some(minor), Some(patch), None)
            if is_numeric_identifier(major)
                && is_numeric_identifier(minor)
                && is_numeric_identifier(patch)
    )
}

/// Check if a string is a valid numeric identifier (non-empty, digits only).
///
/// Used to validate each component of a semver version (major, minor, patch).
fn is_numeric_identifier(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}
