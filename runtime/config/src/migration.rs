//! Legacy configuration migration.
//!
//! This module handles migrating old configuration paths to the new
//! `.alius/config/` split structure.

use crate::error::{ConfigError, ConfigResult};
use std::path::{Path, PathBuf};

/// Migration report.
#[derive(Debug, Clone)]
pub struct MigrationReport {
    /// Successfully migrated files.
    pub migrated_files: Vec<(PathBuf, PathBuf)>,
    /// Files that were skipped (not found or already exists).
    pub skipped_files: Vec<PathBuf>,
    /// Warnings during migration.
    pub warnings: Vec<String>,
    /// Errors during migration.
    pub errors: Vec<String>,
}

impl MigrationReport {
    /// Create a new empty migration report.
    pub fn new() -> Self {
        Self {
            migrated_files: Vec::new(),
            skipped_files: Vec::new(),
            warnings: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Check if migration was successful (no errors).
    pub fn is_success(&self) -> bool {
        self.errors.is_empty()
    }

    /// Check if any files were migrated.
    pub fn has_migrations(&self) -> bool {
        !self.migrated_files.is_empty()
    }
}

impl Default for MigrationReport {
    fn default() -> Self {
        Self::new()
    }
}

/// Migrate legacy configuration to new structure.
///
/// Handles:
/// - `.alius/config.toml` → `.alius/config/config.toml`
/// - `.alius/mcp.json` → `.alius/config/mcp.json`
/// - `.alius/soul/.active` → `.alius/config/soul.toml` (marker only)
pub fn migrate_legacy_config(root: &Path) -> ConfigResult<MigrationReport> {
    let mut report = MigrationReport::new();

    let alius_dir = root.join(".alius");
    let config_dir = alius_dir.join("config");

    // Ensure config directory exists
    if !config_dir.exists() {
        std::fs::create_dir_all(&config_dir).map_err(|e| {
            ConfigError::migration(alius_dir.clone(), config_dir.clone(), e.to_string())
        })?;
    }

    // Migrate legacy flat config.toml
    migrate_config_toml(&alius_dir, &config_dir, &mut report)?;

    // Migrate legacy mcp.json
    migrate_mcp_json(&alius_dir, &config_dir, &mut report)?;

    // Check for legacy soul active marker
    check_soul_active_marker(&alius_dir, &config_dir, &mut report)?;

    Ok(report)
}

/// Migrate legacy `.alius/config.toml` to `.alius/config/config.toml`.
fn migrate_config_toml(
    alius_dir: &Path,
    config_dir: &Path,
    report: &mut MigrationReport,
) -> ConfigResult<()> {
    let legacy_config = alius_dir.join("config.toml");
    let new_config = config_dir.join("config.toml");

    if !legacy_config.exists() {
        report.skipped_files.push(legacy_config.clone());
        return Ok(());
    }

    if new_config.exists() {
        // New config already exists - warn and skip
        report.warnings.push(format!(
            "Skipping migration of {}: {} already exists",
            legacy_config.display(),
            new_config.display()
        ));
        report.skipped_files.push(legacy_config.clone());
        return Ok(());
    }

    // Read legacy config content
    let content =
        std::fs::read_to_string(&legacy_config).map_err(|e| ConfigError::io(&legacy_config, e))?;

    // Write to new location
    std::fs::write(&new_config, &content).map_err(|e| ConfigError::io(&new_config, e))?;

    // Keep legacy file (don't delete automatically - let user decide)
    report
        .migrated_files
        .push((legacy_config.clone(), new_config.clone()));
    report.warnings.push(format!(
        "Migrated {}. Original file kept at {} - you may delete it after verification",
        legacy_config.display(),
        legacy_config.display()
    ));

    Ok(())
}

/// Migrate legacy `.alius/mcp.json` to `.alius/config/mcp.json`.
fn migrate_mcp_json(
    alius_dir: &Path,
    config_dir: &Path,
    report: &mut MigrationReport,
) -> ConfigResult<()> {
    let legacy_mcp = alius_dir.join("mcp.json");
    let new_mcp = config_dir.join("mcp.json");

    if !legacy_mcp.exists() {
        report.skipped_files.push(legacy_mcp.clone());
        return Ok(());
    }

    if new_mcp.exists() {
        report.warnings.push(format!(
            "Skipping migration of {}: {} already exists",
            legacy_mcp.display(),
            new_mcp.display()
        ));
        report.skipped_files.push(legacy_mcp.clone());
        return Ok(());
    }

    // Read legacy MCP config
    let content =
        std::fs::read_to_string(&legacy_mcp).map_err(|e| ConfigError::io(&legacy_mcp, e))?;

    // Write to new location
    std::fs::write(&new_mcp, &content).map_err(|e| ConfigError::io(&new_mcp, e))?;

    report
        .migrated_files
        .push((legacy_mcp.clone(), new_mcp.clone()));
    report.warnings.push(format!(
        "Migrated {}. Original file kept - you may delete it after verification",
        legacy_mcp.display()
    ));

    Ok(())
}

/// Check for legacy soul active marker.
fn check_soul_active_marker(
    alius_dir: &Path,
    config_dir: &Path,
    report: &mut MigrationReport,
) -> ConfigResult<()> {
    // Legacy soul directory structure
    let soul_dir = alius_dir.join("soul");
    let active_marker = soul_dir.join(".active");

    if !active_marker.exists() {
        report.skipped_files.push(active_marker.clone());
        return Ok(());
    }

    // Read the active marker content (contains soul name)
    let soul_name = std::fs::read_to_string(&active_marker)
        .map_err(|e| ConfigError::io(&active_marker, e))?
        .trim()
        .to_string();

    // Check if soul.toml already exists
    let soul_toml = config_dir.join("soul.toml");
    if soul_toml.exists() {
        report.warnings.push(format!(
            "Legacy soul active marker found ({}) but {} already exists - skipping",
            active_marker.display(),
            soul_toml.display()
        ));
        report.skipped_files.push(active_marker.clone());
        return Ok(());
    }

    // Create a minimal soul.toml with the soul name
    let soul_content = generate_soul_toml_from_marker(&soul_name);

    std::fs::write(&soul_toml, &soul_content).map_err(|e| ConfigError::io(&soul_toml, e))?;

    report
        .migrated_files
        .push((active_marker.clone(), soul_toml.clone()));
    report.warnings.push(format!(
        "Created {} from legacy soul active marker (soul: {}). Original marker kept at {}",
        soul_toml.display(),
        soul_name,
        active_marker.display()
    ));

    Ok(())
}

/// Generate minimal soul.toml content from legacy marker.
fn generate_soul_toml_from_marker(soul_name: &str) -> String {
    format!(
        r#"# Alius project soul
#
# Migrated from legacy soul active marker: {}

[agent]
name = "{}"
description = ""
version = "0.1.0"

[agent_card]
documentation_url = ""
icon_url = ""
export_path = ".well-known/agent-card.json"

[[supported_interfaces]]
url = ""
protocol_binding = "HTTP+JSON"
protocol_version = "1.0"

[provider]
organization = ""
url = ""

[capabilities]
streaming = true
push_notifications = false
extended_agent_card = false

[interaction]
default_input_modes = [
  "text/plain",
  "application/json",
]
default_output_modes = [
  "text/plain",
  "application/json",
]

[[skills]]
id = ""
name = ""
description = ""
tags = []
examples = []
input_modes = [
  "text/plain",
  "application/json",
]
output_modes = [
  "text/plain",
  "application/json",
]
"#,
        soul_name, soul_name
    )
}

/// Clean up legacy files after successful migration.
///
/// This removes the original legacy files after the user has verified
/// the migration was successful.
pub fn cleanup_legacy_files(root: &Path, report: &MigrationReport) -> ConfigResult<()> {
    for (from, _to) in &report.migrated_files {
        if from.exists() {
            std::fs::remove_file(from).map_err(|e| ConfigError::io(from, e))?;
        }
    }

    // Remove legacy soul directory if empty after migration
    let soul_dir = root.join(".alius/soul");
    if soul_dir.exists() && is_dir_empty(&soul_dir) {
        std::fs::remove_dir(&soul_dir).map_err(|e| ConfigError::io(&soul_dir, e))?;
    }

    Ok(())
}

/// Check if a directory is empty.
fn is_dir_empty(dir: &Path) -> bool {
    if !dir.is_dir() {
        return false;
    }

    match std::fs::read_dir(dir) {
        Ok(mut entries) => entries.next().is_none(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_migrate_config_toml() {
        let tmp = TempDir::new().unwrap();
        let alius_dir = tmp.path().join(".alius");
        let config_dir = alius_dir.join("config");

        std::fs::create_dir_all(&alius_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        // Create legacy config
        let legacy_config = alius_dir.join("config.toml");
        std::fs::write(&legacy_config, "[project]\nname = \"test\"").unwrap();

        let mut report = MigrationReport::new();
        migrate_config_toml(&alius_dir, &config_dir, &mut report).unwrap();

        assert!(report.has_migrations());
        assert_eq!(report.migrated_files.len(), 1);

        let new_config = config_dir.join("config.toml");
        assert!(new_config.exists());
        assert!(legacy_config.exists()); // Original kept
    }

    #[test]
    fn test_migrate_config_toml_already_exists() {
        let tmp = TempDir::new().unwrap();
        let alius_dir = tmp.path().join(".alius");
        let config_dir = alius_dir.join("config");

        std::fs::create_dir_all(&config_dir).unwrap();

        // Create both legacy and new config
        let legacy_config = alius_dir.join("config.toml");
        std::fs::write(&legacy_config, "[project]\nname = \"legacy\"").unwrap();

        let new_config = config_dir.join("config.toml");
        std::fs::write(&new_config, "[project]\nname = \"new\"").unwrap();

        let mut report = MigrationReport::new();
        migrate_config_toml(&alius_dir, &config_dir, &mut report).unwrap();

        assert!(!report.has_migrations());
        assert!(!report.warnings.is_empty());

        // New config should not be overwritten
        let content = std::fs::read_to_string(&new_config).unwrap();
        assert!(content.contains("name = \"new\""));
    }

    #[test]
    fn test_migrate_mcp_json() {
        let tmp = TempDir::new().unwrap();
        let alius_dir = tmp.path().join(".alius");
        let config_dir = alius_dir.join("config");

        std::fs::create_dir_all(&alius_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        // Create legacy MCP config
        let legacy_mcp = alius_dir.join("mcp.json");
        std::fs::write(&legacy_mcp, "{\"servers\": {}}").unwrap();

        let mut report = MigrationReport::new();
        migrate_mcp_json(&alius_dir, &config_dir, &mut report).unwrap();

        assert!(report.has_migrations());

        let new_mcp = config_dir.join("mcp.json");
        assert!(new_mcp.exists());
    }

    #[test]
    fn test_check_soul_active_marker() {
        let tmp = TempDir::new().unwrap();
        let alius_dir = tmp.path().join(".alius");
        let soul_dir = alius_dir.join("soul");
        let config_dir = alius_dir.join("config");

        std::fs::create_dir_all(&soul_dir).unwrap();
        std::fs::create_dir_all(&config_dir).unwrap();

        // Create active marker
        let active_marker = soul_dir.join(".active");
        std::fs::write(&active_marker, "Backend Developer").unwrap();

        let mut report = MigrationReport::new();
        check_soul_active_marker(&alius_dir, &config_dir, &mut report).unwrap();

        assert!(report.has_migrations());

        let soul_toml = config_dir.join("soul.toml");
        assert!(soul_toml.exists());

        let content = std::fs::read_to_string(&soul_toml).unwrap();
        assert!(content.contains("Backend Developer"));
    }

    #[test]
    fn test_migrate_legacy_config_full() {
        let tmp = TempDir::new().unwrap();
        let project_dir = tmp.path();
        let alius_dir = project_dir.join(".alius");

        std::fs::create_dir_all(&alius_dir).unwrap();

        // Create legacy files
        std::fs::write(alius_dir.join("config.toml"), "[project]\nname = \"test\"").unwrap();
        std::fs::write(alius_dir.join("mcp.json"), "{\"servers\": {}}").unwrap();

        let report = migrate_legacy_config(project_dir).unwrap();

        assert!(report.is_success());
        assert!(report.has_migrations());
        assert_eq!(report.migrated_files.len(), 2);

        // New files should exist
        assert!(alius_dir.join("config/config.toml").exists());
        assert!(alius_dir.join("config/mcp.json").exists());
    }

    #[test]
    fn test_cleanup_legacy_files() {
        let tmp = TempDir::new().unwrap();
        let alius_dir = tmp.path().join(".alius");
        let config_dir = alius_dir.join("config");

        std::fs::create_dir_all(&config_dir).unwrap();

        // Create legacy and new files
        let legacy_config = alius_dir.join("config.toml");
        let new_config = config_dir.join("config.toml");

        std::fs::write(&legacy_config, "content").unwrap();
        std::fs::write(&new_config, "content").unwrap();

        let mut report = MigrationReport::new();
        report
            .migrated_files
            .push((legacy_config.clone(), new_config.clone()));

        cleanup_legacy_files(tmp.path(), &report).unwrap();

        assert!(!legacy_config.exists());
        assert!(new_config.exists());
    }

    #[test]
    fn test_generate_soul_toml_from_marker() {
        let content = generate_soul_toml_from_marker("Frontend Engineer");

        assert!(content.contains("Frontend Engineer"));
        assert!(content.contains("[agent]"));
        assert!(content.contains("[capabilities]"));
        assert!(content.contains("[skills]"));
    }

    #[test]
    fn test_is_dir_empty() {
        let tmp = TempDir::new().unwrap();

        let empty_dir = tmp.path().join("empty");
        std::fs::create_dir_all(&empty_dir).unwrap();
        assert!(is_dir_empty(&empty_dir));

        let non_empty_dir = tmp.path().join("non_empty");
        std::fs::create_dir_all(&non_empty_dir).unwrap();
        std::fs::write(non_empty_dir.join("file.txt"), "content").unwrap();
        assert!(!is_dir_empty(&non_empty_dir));
    }

    #[test]
    fn test_migration_report() {
        let report = MigrationReport::new();
        assert!(report.is_success());
        assert!(!report.has_migrations());

        let report = MigrationReport {
            migrated_files: vec![(PathBuf::from("a"), PathBuf::from("b"))],
            skipped_files: vec![],
            warnings: vec!["test".to_string()],
            errors: vec!["error".to_string()],
        };
        assert!(!report.is_success());
        assert!(report.has_migrations());
    }
}
