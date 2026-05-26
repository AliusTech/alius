//! Permission system for tool execution

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Permission levels for tools
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum PermissionLevel {
    /// Read-only operations (read_file, list_dir, search)
    Read,
    /// Write operations (write_file, edit_file)
    Write,
    /// Execute operations (shell commands)
    Execute,
    /// Admin - all permissions
    Admin,
}

impl PermissionLevel {
    pub fn all() -> Vec<PermissionLevel> {
        vec![PermissionLevel::Read, PermissionLevel::Write, PermissionLevel::Execute, PermissionLevel::Admin]
    }
}

/// Permission manager for controlling tool access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionManager {
    /// Enabled permission levels
    enabled_levels: HashSet<PermissionLevel>,
    /// Tools explicitly allowed (overrides level check)
    allowed_tools: HashSet<String>,
    /// Tools explicitly denied
    denied_tools: HashSet<String>,
    /// Require confirmation for destructive operations
    require_confirmation: bool,
}

impl PermissionManager {
    /// Create a new permission manager with default settings
    pub fn new() -> Self {
        let mut enabled_levels = HashSet::new();
        enabled_levels.insert(PermissionLevel::Read);
        enabled_levels.insert(PermissionLevel::Execute);

        Self {
            enabled_levels,
            allowed_tools: HashSet::new(),
            denied_tools: HashSet::new(),
            require_confirmation: true,
        }
    }

    /// Create with all permissions enabled
    pub fn full_access() -> Self {
        let mut enabled_levels = HashSet::new();
        enabled_levels.extend(PermissionLevel::all());

        Self {
            enabled_levels,
            allowed_tools: HashSet::new(),
            denied_tools: HashSet::new(),
            require_confirmation: false,
        }
    }

    /// Enable a permission level
    pub fn enable(&mut self, level: PermissionLevel) {
        self.enabled_levels.insert(level);
    }

    /// Disable a permission level
    pub fn disable(&mut self, level: PermissionLevel) {
        self.enabled_levels.remove(&level);
    }

    /// Check if a permission level is enabled
    pub fn has_level(&self, level: PermissionLevel) -> bool {
        self.enabled_levels.contains(&level) || self.enabled_levels.contains(&PermissionLevel::Admin)
    }

    /// Allow a specific tool
    pub fn allow_tool(&mut self, tool_name: &str) {
        self.allowed_tools.insert(tool_name.to_string());
    }

    /// Deny a specific tool
    pub fn deny_tool(&mut self, tool_name: &str) {
        self.denied_tools.insert(tool_name.to_string());
    }

    /// Check if a tool is allowed
    pub fn is_tool_allowed(&self, tool_name: &str, required_level: PermissionLevel) -> bool {
        // Check explicit deny first
        if self.denied_tools.contains(tool_name) {
            return false;
        }

        // Check explicit allow
        if self.allowed_tools.contains(tool_name) {
            return true;
        }

        // Check permission level
        self.has_level(required_level)
    }

    /// Check if confirmation is required
    pub fn requires_confirmation(&self) -> bool {
        self.require_confirmation
    }

    /// Set confirmation requirement
    pub fn set_require_confirmation(&mut self, value: bool) {
        self.require_confirmation = value;
    }

    /// Get required permission level for a tool
    pub fn level_for_tool(tool_name: &str) -> PermissionLevel {
        match tool_name {
            "read_file" | "list_dir" | "search" => PermissionLevel::Read,
            "write_file" | "edit_file" => PermissionLevel::Write,
            "shell" => PermissionLevel::Execute,
            _ => PermissionLevel::Admin,
        }
    }
}

impl Default for PermissionManager {
    fn default() -> Self {
        Self::new()
    }
}