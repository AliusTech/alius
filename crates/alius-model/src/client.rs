//! LLM client with provider dispatch.
//!
//! `LlmClient` selects the appropriate provider (OpenAI, Anthropic, etc.)
//! based on configuration and delegates all operations to it.

use anyhow::Result;

use alius_config::LlmSettings;
use alius_protocol::{ProviderType, ToolDef};

use crate::{Conversation, ToolCall};
use crate::provider::{LlmProvider, ChatStream};

/// LLM client that dispatches to the configured provider.
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
    model: String,
}

impl LlmClient {
    /// Create a new LLM client from LLM settings.
    ///
    /// Selects the provider implementation based on `settings.provider`.
    pub fn new(settings: LlmSettings) -> Result<Self> {
        let provider: Box<dyn LlmProvider> = match settings.provider {
            ProviderType::Openai | ProviderType::Custom => {
                Box::new(crate::openai_provider::OpenAiProvider::new(&settings)?)
            }
            ProviderType::Anthropic => {
                Box::new(crate::anthropic_provider::AnthropicProvider::new(&settings)?)
            }
            ProviderType::Google => {
                return Err(anyhow::anyhow!("Google provider not yet implemented"));
            }
        };

        Ok(Self {
            provider,
            model: settings.model,
        })
    }

    /// Get the configured model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Stream a chat completion from the conversation history.
    pub async fn chat_stream(&self, conversation: &Conversation) -> Result<ChatStream> {
        self.provider.chat_stream(conversation).await
    }

    /// Single-shot chat completion (for the `run` command).
    pub async fn chat_once(&self, prompt: &str, system: Option<&str>) -> Result<String> {
        self.provider.chat_once(prompt, system).await
    }

    /// List available models from the provider.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        self.provider.list_models().await
    }

    /// Stream a chat completion with tool calling support.
    pub async fn chat_stream_with_tools(
        &self,
        conversation: &Conversation,
        tools: Vec<ToolDef>,
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        self.provider.chat_stream_with_tools(conversation, &tools).await
    }

    /// Continue the conversation with tool execution results.
    pub async fn continue_with_tool_results(
        &self,
        conversation: &Conversation,
        tool_results: Vec<(String, String, String)>,
        tools: Vec<ToolDef>,
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        self.provider.continue_with_tool_results(conversation, &tool_results, &tools).await
    }
}
