//! Google Generative AI provider implementation.
//!
//! Uses the OpenAI-compatible interface that Google provides at
//! `https://generativelanguage.googleapis.com/v1beta/openai`.
//!
//! Internally delegates to [`OpenAiProvider`] with the Google base URL,
//! so all chat/tool/streaming functionality works through the same code path.

use anyhow::{bail, Result};
use futures::Stream;
use std::future::Future;
use std::pin::Pin;

use crate::openai_provider::OpenAiProvider;
use crate::provider::{ChatStream, LlmProvider, ToolResponse};
use crate::{ChatEvent, Conversation, ToolCall};
use protocol_interface::ToolDef;
use runtime_config::LlmSettings;

/// Google Generative AI provider using OpenAI-compatible interface.
///
/// Delegates all operations to [`OpenAiProvider`] configured with Google's
/// base URL and the provided API key.
pub struct GoogleProvider {
    inner: OpenAiProvider,
    model: String,
    api_key: String,
    base_url: String,
}

impl std::fmt::Debug for GoogleProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GoogleProvider")
            .field("model", &self.model)
            .field("base_url", &self.base_url)
            .finish()
    }
}

impl GoogleProvider {
    /// Create a new Google provider.
    ///
    /// # Arguments
    /// * `api_key` — Google API key (from `GOOGLE_API_KEY` env var)
    /// * `model` — Model name (e.g. "gemini-2.0-flash")
    pub fn new(api_key: &str, model: &str) -> Result<Self> {
        if api_key.is_empty() {
            bail!("Google API key is required");
        }

        let base_url = "https://generativelanguage.googleapis.com/v1beta/openai".to_string();

        let settings = LlmSettings {
            provider: protocol_interface::ProviderType::Openai,
            provider_mode: None,
            model: model.to_string(),
            api_key: Some(api_key.to_string()),
            api_key_env: None,
            base_url: Some(base_url.clone()),
            review_model: None,
        };

        let inner = OpenAiProvider::new(&settings)?;

        Ok(Self {
            inner,
            model: model.to_string(),
            api_key: api_key.to_string(),
            base_url,
        })
    }

    /// Create with custom base URL.
    pub fn with_base_url(mut self, url: &str) -> Self {
        let settings = LlmSettings {
            provider: protocol_interface::ProviderType::Openai,
            provider_mode: None,
            model: self.model.clone(),
            api_key: Some(self.api_key.clone()),
            api_key_env: None,
            base_url: Some(url.to_string()),
            review_model: None,
        };
        if let Ok(inner) = OpenAiProvider::new(&settings) {
            self.inner = inner;
            self.base_url = url.to_string();
        }
        self
    }
}

impl LlmProvider for GoogleProvider {
    fn chat_stream<'a>(
        &'a self,
        conversation: &'a Conversation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatStream>> + Send + 'a>> {
        self.inner.chat_stream(conversation)
    }

    fn chat_once<'a>(
        &'a self,
        prompt: &'a str,
        system: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        self.inner.chat_once(prompt, system)
    }

    fn list_models<'a>(&'a self) -> Pin<Box<dyn Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async {
            Ok(vec![
                "gemini-2.5-flash".to_string(),
                "gemini-2.5-pro".to_string(),
                "gemini-2.0-flash".to_string(),
            ])
        })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        conversation: &'a Conversation,
        tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        self.inner.chat_stream_with_tools(conversation, tools)
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        conversation: &'a Conversation,
        tool_results: &'a [(String, String, String)],
        assistant_tool_calls: &'a [ToolCall],
        tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        self.inner
            .continue_with_tool_results(conversation, tool_results, assistant_tool_calls, tools)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_provider_new() {
        let provider = GoogleProvider::new("test-key", "gemini-2.0-flash").unwrap();
        assert_eq!(provider.model, "gemini-2.0-flash");
    }

    #[test]
    fn test_google_provider_new_missing_key() {
        let result = GoogleProvider::new("", "gemini-2.0-flash");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("required"));
    }

    #[test]
    fn test_google_provider_custom_base_url() {
        let provider = GoogleProvider::new("key", "gemini-2.0-flash")
            .unwrap()
            .with_base_url("https://custom.googleapis.com/v1beta/openai");
        // Model should still be set
        assert_eq!(provider.model, "gemini-2.0-flash");
    }

    #[tokio::test]
    async fn test_google_list_models() {
        let provider = GoogleProvider::new("key", "gemini-2.0-flash").unwrap();
        let models = provider.list_models().await.unwrap();
        assert!(models.contains(&"gemini-2.5-flash".to_string()));
    }
}
