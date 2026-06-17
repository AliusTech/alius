//! Test utilities for core-runtime.
//!
//! Provides [`CoreRuntimeHarness`] for creating isolated runtime instances
//! with fake providers and tool registries, plus MCP-specific test doubles.

use std::sync::Arc;

use anyhow::Result;
use futures::stream;
use protocol_interface::core::ToolSource;
use protocol_interface::*;
use runtime_config::Settings;
use runtime_model::{ChatEvent, ChatStream, LlmClient, LlmProvider, ToolCall, ToolResponse};
use runtime_tools::ToolRegistry;
use tempfile::TempDir;

use crate::manager::CoreRuntimeManager;
use crate::runtime::CoreRuntimeBuilder;

/// An isolated runtime test environment.
///
/// Creates a temporary workspace directory and wires up a `CoreRuntimeManager`
/// with a fake LLM provider and an optional set of fake tools.
///
/// # Examples
///
/// ```ignore
/// let harness = CoreRuntimeHarness::new();
/// let manager = harness.manager();
/// // Use manager for testing...
/// ```
pub struct CoreRuntimeHarness {
    _temp_dir: TempDir,
    workspace_root: std::path::PathBuf,
    manager: CoreRuntimeManager,
}

impl CoreRuntimeHarness {
    /// Create a new harness with a fake LLM provider and empty tool registry.
    pub fn new() -> Self {
        Self::with_provider(Box::new(runtime_model::testing::FakeProvider::new()))
    }

    /// Create a harness with a custom LLM provider.
    pub fn with_provider(provider: Box<dyn LlmProvider>) -> Self {
        let temp_dir = TempDir::new().expect("failed to create temp dir");
        let workspace_root = temp_dir.path().to_path_buf();

        let client =
            LlmClient::new_with_provider_for_test(provider, "test-model", ProviderType::Openai);

        let registry = ToolRegistry::new();
        let runtime = CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new(&workspace_root))
            .settings(Settings::default())
            .client(client)
            .tool_registry_arc(Arc::new(registry))
            .build()
            .expect("failed to build test runtime");

        let manager = CoreRuntimeManager::from_runtime(&workspace_root, runtime);

        Self {
            _temp_dir: temp_dir,
            workspace_root,
            manager,
        }
    }

    /// Get a reference to the runtime manager.
    pub fn manager(&self) -> &CoreRuntimeManager {
        &self.manager
    }

    /// Consume the harness and return the manager.
    pub fn into_manager(self) -> CoreRuntimeManager {
        self.manager
    }

    /// Get the workspace root path.
    pub fn workspace_root(&self) -> &std::path::Path {
        &self.workspace_root
    }
}

impl Default for CoreRuntimeHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// A fake MCP echo tool for loop engine integration tests.
///
/// Echoes the `message` argument back as output. Supports configurable
/// confirmation behavior.
pub struct FakeMcpEchoTool {
    require_confirm: bool,
}

impl FakeMcpEchoTool {
    /// Create a new MCP echo tool (no confirmation required).
    pub fn new() -> Self {
        Self {
            require_confirm: false,
        }
    }

    /// Create an MCP echo tool that requires confirmation in Plan mode.
    pub fn with_confirm() -> Self {
        Self {
            require_confirm: true,
        }
    }
}

impl Default for FakeMcpEchoTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl runtime_tools::AliusTool for FakeMcpEchoTool {
    fn name(&self) -> &'static str {
        "mcp_echo"
    }

    fn description(&self) -> &'static str {
        "fake MCP echo tool"
    }

    fn input_schema(&self) -> serde_json::Value {
        serde_json::json!({"type": "object", "properties": {"message": {"type": "string"}}})
    }

    fn source(&self) -> ToolSource {
        ToolSource::Mcp
    }

    fn preview_confirmation(&self, _args: &serde_json::Value, mode: RuntimeMode) -> bool {
        self.require_confirm && mode == RuntimeMode::Plan
    }

    async fn execute(
        &self,
        args: serde_json::Value,
        _ctx: runtime_tools::ToolContext,
    ) -> std::result::Result<runtime_tools::ToolResult, AliusError> {
        let msg = args
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("no message");
        Ok(runtime_tools::ToolResult {
            output: format!("mcp_echo: {msg}"),
            success: true,
            metadata: None,
        })
    }
}

/// A fake LLM provider that requests an MCP tool call.
///
/// Returns a tool call for "mcp_echo" with a hardcoded message.
/// Used in loop engine integration tests to verify MCP tool execution paths.
pub struct FakeMcpToolCallProvider;

impl LlmProvider for FakeMcpToolCallProvider {
    fn chat_stream<'a>(
        &'a self,
        _conversation: &'a runtime_model::Conversation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatStream>> + Send + 'a>> {
        Box::pin(async {
            let s: ChatStream = Box::pin(stream::iter(vec![Ok(ChatEvent::Done {
                full_response: String::new(),
            })]));
            Ok(s)
        })
    }

    fn chat_once<'a>(
        &'a self,
        _prompt: &'a str,
        _system: Option<&'a str>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async { Ok(String::new()) })
    }

    fn list_models<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        _conversation: &'a runtime_model::Conversation,
        _tools: &'a [protocol_interface::ToolDef],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResponse> + Send + 'a>> {
        Box::pin(async {
            let s: ChatStream = Box::pin(stream::iter(vec![
                Ok(ChatEvent::Delta {
                    text: "calling mcp tool".to_string(),
                }),
                Ok(ChatEvent::Done {
                    full_response: "calling mcp tool".to_string(),
                }),
            ]));
            let tool_calls = vec![ToolCall::new(
                "tc-mcp-1".to_string(),
                "mcp_echo".to_string(),
                serde_json::json!({"message": "hello from mcp"}),
            )];
            Ok((s, Some(tool_calls)))
        })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        _conversation: &'a runtime_model::Conversation,
        tool_results: &'a [(String, String, String)],
        _assistant_tool_calls: &'a [ToolCall],
        _tools: &'a [protocol_interface::ToolDef],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = ToolResponse> + Send + 'a>> {
        let echoed: String = tool_results
            .iter()
            .map(|(_, name, output)| format!("- {name}: {output}"))
            .collect::<Vec<_>>()
            .join("\n");
        Box::pin(async move {
            let s: ChatStream = Box::pin(stream::iter(vec![
                Ok(ChatEvent::Delta { text: echoed }),
                Ok(ChatEvent::Done {
                    full_response: String::new(),
                }),
            ]));
            Ok((s, None))
        })
    }
}
