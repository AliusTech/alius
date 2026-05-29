//! Provider abstraction for LLM backends.

use anyhow::Result;
use futures::Stream;
use std::future::Future;
use std::pin::Pin;

use alius_protocol::ToolDef;
use crate::{ChatEvent, Conversation, ToolCall};

/// Type alias for a boxed chat event stream.
pub type ChatStream = Pin<Box<dyn Stream<Item = Result<ChatEvent>> + Send>>;

/// Type alias for a tool-calling response: (stream, optional tool calls).
pub type ToolResponse = Result<(ChatStream, Option<Vec<ToolCall>>)>;

/// Trait for LLM provider implementations.
///
/// Each provider (OpenAI, Anthropic, etc.) implements this trait to handle
/// API-specific request formatting, streaming, and tool calling.
pub trait LlmProvider: Send + Sync {
    /// Stream a chat response (true SSE streaming).
    fn chat_stream<'a>(
        &'a self,
        conversation: &'a Conversation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatStream>> + Send + 'a>>;

    /// Non-streaming single-shot chat.
    fn chat_once<'a>(
        &'a self,
        prompt: &'a str,
        system: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>>;

    /// List available models (returns empty vec if unsupported).
    fn list_models<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>>;

    /// Stream with tool definitions — returns (stream, optional tool calls).
    fn chat_stream_with_tools<'a>(
        &'a self,
        conversation: &'a Conversation,
        tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>>;

    /// Continue with tool results — multi-round tool calling.
    fn continue_with_tool_results<'a>(
        &'a self,
        conversation: &'a Conversation,
        tool_results: &'a [(String, String, String)],
        tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>>;
}
