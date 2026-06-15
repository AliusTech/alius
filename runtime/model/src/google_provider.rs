//! Google Generative AI provider implementation.
//!
//! Uses the OpenAI-compatible interface that Google provides.
//! For actual API calls, use the OpenAI provider with Google's base_url.

use anyhow::{bail, Result};
use futures::{Stream, StreamExt};
use std::future::Future;
use std::pin::Pin;

use crate::provider::{ChatStream, LlmProvider, ToolResponse};
use crate::{ChatEvent, Conversation, ToolCall};
use protocol_interface::ToolDef;

/// Google Generative AI provider using OpenAI-compatible interface.
#[derive(Debug)]
pub struct GoogleProvider {
    base_url: String,
    api_key: String,
    model: String,
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
        Ok(Self {
            base_url: "https://generativelanguage.googleapis.com/v1beta/openai".to_string(),
            api_key: api_key.to_string(),
            model: model.to_string(),
        })
    }

    /// Create with custom base URL.
    pub fn with_base_url(mut self, url: &str) -> Self {
        self.base_url = url.to_string();
        self
    }
}

impl LlmProvider for GoogleProvider {
    fn chat_stream<'a>(
        &'a self,
        _conversation: &'a Conversation,
    ) -> Pin<Box<dyn Future<Output = Result<ChatStream>> + Send + 'a>> {
        Box::pin(async {
            bail!("Google provider streaming not yet implemented — use OpenAI provider with Google base_url")
        })
    }

    fn chat_once<'a>(
        &'a self,
        _prompt: &'a str,
        _system: Option<&'a str>,
    ) -> Pin<Box<dyn Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async {
            bail!("Google provider not yet implemented — use OpenAI provider with Google base_url")
        })
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
        _conversation: &'a Conversation,
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        Box::pin(async { bail!("Google provider tool calling not yet implemented") })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        _conversation: &'a Conversation,
        _tool_results: &'a [(String, String, String)],
        _assistant_tool_calls: &'a [ToolCall],
        _tools: &'a [ToolDef],
    ) -> Pin<Box<dyn Future<Output = ToolResponse> + Send + 'a>> {
        Box::pin(async { bail!("Google provider tool results not yet implemented") })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_google_provider_new() {
        let provider = GoogleProvider::new("test-key", "gemini-2.0-flash").unwrap();
        assert_eq!(provider.model, "gemini-2.0-flash");
        assert_eq!(
            provider.base_url,
            "https://generativelanguage.googleapis.com/v1beta/openai"
        );
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
        assert_eq!(
            provider.base_url,
            "https://custom.googleapis.com/v1beta/openai"
        );
    }

    #[tokio::test]
    async fn test_google_list_models() {
        let provider = GoogleProvider::new("key", "gemini-2.0-flash").unwrap();
        let models = provider.list_models().await.unwrap();
        assert!(models.contains(&"gemini-2.5-flash".to_string()));
    }
}
