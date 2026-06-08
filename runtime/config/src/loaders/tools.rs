//! Loader for tools.toml.

use crate::error::ConfigResult;
use crate::views::{
    McpToolConfig, PluginToolConfig, ToolConfig, ToolConfirmationConfig, ToolExecutionConfig,
    ToolRegistryConfig, WorkflowToolConfig,
};
use std::path::Path;

/// Load tools.toml from the given path.
pub fn load_tools(path: &Path) -> ConfigResult<ToolConfig> {
    let raw: ToolsToml = super::load_toml(path)?;
    Ok(raw.into())
}

/// tools.toml raw structure.
#[derive(Debug, Clone, serde::Deserialize)]
struct ToolsToml {
    #[serde(default)]
    registry: Option<ToolRegistryToml>,
    #[serde(default)]
    execution: Option<ToolExecutionConfig>,
    #[serde(default)]
    confirmation: Option<ToolConfirmationConfig>,
    #[serde(default, rename = "builtin")]
    _legacy_builtin: Option<toml::Value>,
    #[serde(default)]
    mcp: Option<McpToolConfig>,
    #[serde(default)]
    plugin: Option<PluginToolConfig>,
    #[serde(default)]
    workflow: Option<WorkflowToolConfig>,
}

/// Raw registry config. Legacy fields are accepted as migration input only.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct ToolRegistryToml {
    rust_wasm_modules: Option<bool>,
    mcp_tools: Option<bool>,
    plugin_tools: Option<bool>,
    builtin_tools: Option<bool>,
    workflow_tools: Option<bool>,
}

impl From<ToolsToml> for ToolConfig {
    fn from(raw: ToolsToml) -> Self {
        let default = ToolConfig::default();
        let default_registry = default.registry.clone();
        let registry = raw.registry.map_or(default_registry.clone(), |registry| {
            let rust_wasm_modules = registry
                .rust_wasm_modules
                .or(registry.plugin_tools)
                .or(registry.builtin_tools)
                .unwrap_or(default_registry.rust_wasm_modules);

            ToolRegistryConfig {
                rust_wasm_modules,
                mcp_tools: registry.mcp_tools.unwrap_or(default_registry.mcp_tools),
                workflow_tools: registry
                    .workflow_tools
                    .unwrap_or(default_registry.workflow_tools),
            }
        });

        Self {
            registry,
            execution: raw.execution.unwrap_or(default.execution),
            confirmation: raw.confirmation.unwrap_or(default.confirmation),
            mcp: raw.mcp.unwrap_or(default.mcp),
            plugin: raw.plugin.unwrap_or(default.plugin),
            workflow: raw.workflow.unwrap_or(default.workflow),
        }
    }
}

impl Default for ToolConfig {
    fn default() -> Self {
        Self {
            registry: ToolRegistryConfig {
                rust_wasm_modules: true,
                mcp_tools: false,
                workflow_tools: false,
            },
            execution: ToolExecutionConfig {
                default_timeout_ms: 120000,
                max_parallel_tools: 4,
                persist_tool_trace: true,
            },
            confirmation: ToolConfirmationConfig {
                read_only_tools: "auto".to_string(),
                write_tools: "ask".to_string(),
                shell_tools: "ask".to_string(),
                network_tools: "ask".to_string(),
                remote_a2a_tools: "deny".to_string(),
            },
            mcp: McpToolConfig {
                config: ".alius/config/mcp.json".to_string(),
                load_on_workspace_start: false,
                register_as_tools: false,
            },
            plugin: PluginToolConfig {
                load_on_workspace_start: false,
                register_as_tools: false,
            },
            workflow: WorkflowToolConfig {
                load_project_workflows: false,
                register_as_tools: false,
            },
        }
    }
}
