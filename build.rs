//! Build script to embed the release version from CI or the current git tag.

use std::env;
use std::process::Command;

fn main() {
    let version = env_version("ALIUS_VERSION")
        .or_else(|| env_version("GITHUB_REF_NAME"))
        .or_else(|| env_version("GITHUB_REF"))
        .or_else(version_from_git_tag)
        .or_else(|| env::var("CARGO_PKG_VERSION").ok())
        .unwrap_or_else(|| "0.0.0".to_string());

    println!("cargo:rustc-env=ALIUS_VERSION={}", version);
    println!("cargo:rerun-if-env-changed=ALIUS_VERSION");
    println!("cargo:rerun-if-env-changed=GITHUB_REF_NAME");
    println!("cargo:rerun-if-env-changed=GITHUB_REF");

    if let Some(git_dir) = git_dir() {
        println!("cargo:rerun-if-changed={}/HEAD", git_dir);
        println!("cargo:rerun-if-changed={}/refs/tags", git_dir);
    }
}

fn env_version(name: &str) -> Option<String> {
    env::var(name).ok().and_then(normalize_version)
}

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

fn is_numeric_identifier(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}
