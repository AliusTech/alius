//! Test utilities for runtime-model.
//!
//! Provides reusable fake LLM provider implementations for testing
//! the model runtime, loop engine, protocol bridge, and workflow execution.

use std::future::Future;
use std::pin::Pin;

use crate::ToolCall;
use anyhow::Result;
use futures::stream;
use protocol_interface::ToolDef;

use crate::events::ChatEvent;
use crate::provider::{ChatStream, LlmProvider, ToolResponse};

/// A fake LLM provider that returns configurable canned responses.
///
/// By default returns "fake-response" as text with no tool calls.
///
/// # Examples
///
/// ```ignore
/// let client = LlmClient::new_with_provider_for_test(
///     Box::new(FakeProvider::new()),
///     "fake-model",
///     ProviderType::OpenAI,
/// );
/// ```
///
/// ```ignore
/// let provider = FakeProvider::new().with_response("custom answer");
/// ```
pub struct FakeProvider {
    response: String,
    model_name: String,
}

impl FakeProvider {
    /// Create a new fake provider with default response "fake-response".
    pub fn new() -> Self {
        Self {
            response: "fake-response".to_string(),
            model_name: "fake-model".to_string(),
        }
    }

    /// Set the text response returned by `chat_once` and `chat_stream`.
    pub fn with_response(mut self, response: impl Into<String>) -> Self {
        self.response = response.into();
        self
    }

    /// Set the model name returned by `list_models`.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.model_name = model.into();
        self
    }
}

impl Default for FakeProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl LlmProvider for FakeProvider {
    fn chat_stream<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatStream>> + Send + 'a>> {
        let text = self.response.clone();
        Box::pin(async move {
            let s: ChatStream = Box::pin(stream::iter(vec![
                Ok(ChatEvent::Delta { text: text.clone() }),
                Ok(ChatEvent::Done {
                    full_response: String::new(),
                }),
            ]));
            Ok(s)
        })
    }

    fn chat_once<'a>(
        &'a self,
        _prompt: &'a str,
        _system: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        let text = self.response.clone();
        Box::pin(async move { Ok(text) })
    }

    fn list_models<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>> {
        let name = self.model_name.clone();
        Box::pin(async move { Ok(vec![name]) })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        let text = self.response.clone();
        Box::pin(async move {
            let s: ChatStream = Box::pin(stream::iter(vec![
                Ok(ChatEvent::Delta { text: text.clone() }),
                Ok(ChatEvent::Done {
                    full_response: String::new(),
                }),
            ]));
            Ok((s, None))
        })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
        _tool_results: &'a [(String, String, String)],
        _assistant_tool_calls: &'a [ToolCall],
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        Box::pin(async move {
            let s: ChatStream = Box::pin(stream::iter(vec![Ok(ChatEvent::Done {
                full_response: String::new(),
            })]));
            Ok((s, None))
        })
    }
}

/// A no-op provider that returns empty responses immediately.
///
/// Useful when the test doesn't care about LLM output.
pub struct NoOpProvider;

impl LlmProvider for NoOpProvider {
    fn chat_stream<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatStream>> + Send + 'a>> {
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
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async { Ok(String::new()) })
    }

    fn list_models<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        Box::pin(async {
            let s: ChatStream = Box::pin(stream::iter(vec![Ok(ChatEvent::Done {
                full_response: String::new(),
            })]));
            Ok((s, None))
        })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
        _tool_results: &'a [(String, String, String)],
        _assistant_tool_calls: &'a [ToolCall],
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        Box::pin(async {
            let s: ChatStream = Box::pin(stream::iter(vec![Ok(ChatEvent::Done {
                full_response: String::new(),
            })]));
            Ok((s, None))
        })
    }
}

/// A fake provider that returns a tool call for a configurable tool.
///
/// On `chat_stream_with_tools`, returns `Some(vec![tool_call])` with
/// the configured tool name and ID. Used to test tool routing and
/// the protocol bridge's tool-call flow.
pub struct FakeToolCallProvider {
    tool_call_id: String,
    tool_name: String,
}

impl FakeToolCallProvider {
    /// Create a provider that will request a tool call with the given ID and name.
    pub fn new(tool_call_id: impl Into<String>, tool_name: impl Into<String>) -> Self {
        Self {
            tool_call_id: tool_call_id.into(),
            tool_name: tool_name.into(),
        }
    }
}

impl LlmProvider for FakeToolCallProvider {
    fn chat_stream<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatStream>> + Send + 'a>> {
        let s: ChatStream = Box::pin(stream::iter(vec![
            Ok(ChatEvent::Delta {
                text: "calling tool".to_string(),
            }),
            Ok(ChatEvent::Done {
                full_response: "calling tool".to_string(),
            }),
        ]));
        Box::pin(async move { Ok(s) })
    }

    fn chat_once<'a>(
        &'a self,
        _prompt: &'a str,
        _system: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async { Ok(String::new()) })
    }

    fn list_models<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async { Ok(Vec::new()) })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        let tool_calls = vec![ToolCall::new(
            self.tool_call_id.clone(),
            self.tool_name.clone(),
            serde_json::json!({}),
        )];
        let s: ChatStream = Box::pin(stream::iter(vec![
            Ok(ChatEvent::Delta {
                text: "requesting tool".to_string(),
            }),
            Ok(ChatEvent::Done {
                full_response: "requesting tool".to_string(),
            }),
        ]));
        Box::pin(async move { Ok((s, Some(tool_calls))) })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        _conversation: &'a crate::conversation::Conversation,
        _tool_results: &'a [(String, String, String)],
        _assistant_tool_calls: &'a [ToolCall],
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        let s: ChatStream = Box::pin(stream::iter(vec![Ok(ChatEvent::Done {
            full_response: String::new(),
        })]));
        Box::pin(async move { Ok((s, None)) })
    }
}
