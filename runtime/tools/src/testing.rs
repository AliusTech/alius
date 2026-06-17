//! Test utilities for runtime-tools.
//!
//! Provides reusable fake tool implementations for testing tool registry,
//! shell gate, WASM host, and loop engine behavior.

use async_trait::async_trait;
use protocol_interface::core::ToolSource;
use protocol_interface::{AliusError, RuntimeMode};
use serde_json::Value as JsonValue;

use crate::traits::{AliusTool, ToolContext, ToolResult};

/// A configurable fake tool for testing.
///
/// # Examples
///
/// ```ignore
/// let tool = FakeTool::new("my_tool");
/// registry.register(Box::new(tool));
/// ```
///
/// ```ignore
/// let tool = FakeTool::new("echo")
///     .with_response("custom output");
/// ```
pub struct FakeTool {
    name: &'static str,
    description: &'static str,
    source: ToolSource,
    response: Option<String>,
    requires_confirm: bool,
    confirm_all_modes: bool,
}

impl FakeTool {
    /// Create a new fake tool with the given name.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            description: "fake test tool",
            source: ToolSource::RustWasm,
            response: None,
            requires_confirm: false,
            confirm_all_modes: false,
        }
    }

    /// Set the description shown to the LLM.
    pub fn with_description(mut self, desc: &'static str) -> Self {
        self.description = desc;
        self
    }

    /// Set a fixed response for `execute()`.
    pub fn with_response(mut self, response: impl Into<String>) -> Self {
        self.response = Some(response.into());
        self
    }

    /// Mark this tool as MCP-sourced.
    pub fn as_mcp(mut self) -> Self {
        self.source = ToolSource::Mcp;
        self
    }

    /// Make `preview_confirmation()` return `true` in Plan mode only.
    pub fn with_plan_confirm(mut self) -> Self {
        self.requires_confirm = true;
        self.confirm_all_modes = false;
        self
    }

    /// Make `preview_confirmation()` return `true` in all modes.
    pub fn with_always_confirm(mut self) -> Self {
        self.requires_confirm = true;
        self.confirm_all_modes = true;
        self
    }
}

#[async_trait]
impl AliusTool for FakeTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        self.description
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({"type": "object", "properties": {}})
    }

    fn source(&self) -> ToolSource {
        self.source.clone()
    }

    fn preview_confirmation(&self, _args: &JsonValue, mode: RuntimeMode) -> bool {
        if self.confirm_all_modes {
            self.requires_confirm
        } else {
            self.requires_confirm && mode == RuntimeMode::Plan
        }
    }

    async fn execute(&self, _args: JsonValue, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        match &self.response {
            Some(text) => Ok(ToolResult::success(text.clone())),
            None => Ok(ToolResult::success(format!("fake:{}:ok", self.name))),
        }
    }
}

/// A fake MCP-sourced tool that echoes its input back.
///
/// Supports configurable confirmation behavior.
pub struct EchoTool {
    name: &'static str,
    require_confirm: bool,
}

impl EchoTool {
    /// Create a new echo tool with the given name.
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            require_confirm: false,
        }
    }

    /// Make `preview_confirmation()` return `true` in Plan mode.
    pub fn with_confirm(mut self) -> Self {
        self.require_confirm = true;
        self
    }
}

#[async_trait]
impl AliusTool for EchoTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        "fake MCP echo tool"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({"type": "object", "properties": {"message": {"type": "string"}}})
    }

    fn source(&self) -> ToolSource {
        ToolSource::Mcp
    }

    fn preview_confirmation(&self, _args: &JsonValue, mode: RuntimeMode) -> bool {
        self.require_confirm && mode == RuntimeMode::Plan
    }

    async fn execute(&self, args: JsonValue, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let msg = args
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("no message");
        Ok(ToolResult {
            output: format!("mcp_echo: {msg}"),
            success: true,
            metadata: None,
        })
    }
}

/// A tool that always requires confirmation.
///
/// Useful for testing fail-closed behavior — this tool should never
/// actually execute when confirmation is not provided.
pub struct ConfirmationRequiredTool {
    name: &'static str,
}

impl ConfirmationRequiredTool {
    /// Create a new confirmation-required tool.
    pub fn new(name: &'static str) -> Self {
        Self { name }
    }
}

impl Default for ConfirmationRequiredTool {
    fn default() -> Self {
        Self {
            name: "dangerous_tool",
        }
    }
}

#[async_trait]
impl AliusTool for ConfirmationRequiredTool {
    fn name(&self) -> &'static str {
        self.name
    }

    fn description(&self) -> &'static str {
        "tool that always requires confirmation"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({"type": "object", "properties": {}})
    }

    fn preview_confirmation(&self, _args: &JsonValue, _mode: RuntimeMode) -> bool {
        true
    }

    async fn execute(&self, _args: JsonValue, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        Ok(ToolResult::success("should-not-reach".to_string()))
    }
}
