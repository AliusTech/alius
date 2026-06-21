//! LLM client with provider dispatch.
//!
//! `LlmClient` selects the appropriate provider (OpenAI, BigModel, etc.)
//! based on configuration and delegates all operations to it.

use std::sync::RwLock;
use std::time::{Duration, Instant};

use anyhow::Result;

use protocol_interface::{ProviderMode, ProviderType, ToolDef};
use runtime_config::LlmSettings;

use crate::endpoint::{normalize_anthropic_api_base, normalize_openai_api_base};
use crate::provider::{ChatStream, LlmProvider};
use crate::{Conversation, ToolCall};

/// Parse model IDs from a JSON response body like `{"data": [{"id": "..."}]}`.
fn parse_models_json(body: Option<serde_json::Value>) -> Option<Vec<String>> {
    body?.get("data")?.as_array().map(|arr| {
        arr.iter()
            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(String::from))
            .collect()
    })
}

/// Cache entry with expiration time.
struct ModelCacheEntry {
    models: Vec<String>,
    expires_at: Instant,
}

impl ModelCacheEntry {
    fn new(models: Vec<String>, ttl: Duration) -> Self {
        Self {
            models,
            expires_at: Instant::now() + ttl,
        }
    }

    fn is_valid(&self) -> bool {
        Instant::now() < self.expires_at
    }

    fn models(&self) -> Vec<String> {
        self.models.clone()
    }
}

/// LLM client that dispatches to the configured provider.
pub struct LlmClient {
    provider: Box<dyn LlmProvider>,
    model: String,
    provider_type: ProviderType,
    provider_mode: Option<ProviderMode>,
    /// Model list cache with TTL.
    model_cache: RwLock<Option<ModelCacheEntry>>,
    /// Cache TTL (default: 5 minutes).
    cache_ttl: Duration,
}

impl LlmClient {
    /// Construct a client from a provider implementation for cross-crate engine tests.
    #[doc(hidden)]
    #[cfg(any(test, debug_assertions, feature = "testing"))]
    pub fn new_with_provider_for_test(
        provider: Box<dyn crate::provider::LlmProvider>,
        model: impl Into<String>,
        provider_type: ProviderType,
    ) -> Self {
        Self {
            provider,
            model: model.into(),
            provider_type,
            provider_mode: None,
            model_cache: RwLock::new(None),
            cache_ttl: Duration::from_secs(300),
        }
    }

    /// Create a new LLM client from LLM settings.
    ///
    /// Selects the provider implementation based on `settings.provider`.
    pub fn new(settings: LlmSettings) -> Result<Self> {
        let provider: Box<dyn LlmProvider> = match &settings.provider {
            ProviderType::Openai | ProviderType::Custom => {
                Box::new(crate::openai_provider::OpenAiProvider::new(&settings)?)
            }
            ProviderType::Anthropic => Box::new(crate::anthropic_provider::AnthropicProvider::new(
                &settings,
            )?),
            ProviderType::XiaomiMimo => {
                if uses_anthropic_protocol(&settings.provider, &settings.provider_mode) {
                    Box::new(
                        crate::anthropic_provider::AnthropicProvider::new_token_plan(&settings)?,
                    )
                } else {
                    Box::new(crate::openai_provider::OpenAiProvider::new(&settings)?)
                }
            }
            ProviderType::BigModel => {
                if uses_anthropic_protocol(&settings.provider, &settings.provider_mode) {
                    Box::new(crate::anthropic_provider::AnthropicProvider::new(
                        &settings,
                    )?)
                } else {
                    Box::new(crate::openai_provider::OpenAiProvider::new_bigmodel(
                        &settings,
                    )?)
                }
            }
            ProviderType::DeepSeek => {
                if uses_anthropic_protocol(&settings.provider, &settings.provider_mode) {
                    Box::new(crate::anthropic_provider::AnthropicProvider::new(
                        &settings,
                    )?)
                } else {
                    Box::new(crate::openai_provider::OpenAiProvider::new_deepseek(
                        &settings,
                    )?)
                }
            }
            ProviderType::Google => {
                let api_key = settings.get_api_key().ok_or_else(|| {
                    anyhow::anyhow!(
                        "Google API key not found. Set GOOGLE_API_KEY or api_key in config."
                    )
                })?;
                let mut provider =
                    crate::google_provider::GoogleProvider::new(&api_key, &settings.model)?;
                if let Some(ref base_url) = settings.base_url {
                    provider = provider.with_base_url(base_url);
                }
                Box::new(provider)
            }
        };

        let provider_type = settings.provider.clone();
        let provider_mode = settings.provider_mode.clone();

        Ok(Self {
            provider,
            model: settings.model,
            provider_type,
            provider_mode,
            model_cache: RwLock::new(None),
            cache_ttl: Duration::from_secs(300), // 5 minutes
        })
    }

    /// Get the configured model identifier.
    pub fn model(&self) -> &str {
        &self.model
    }

    /// Get the provider type.
    pub fn provider_type(&self) -> &ProviderType {
        &self.provider_type
    }

    /// Get the provider mode.
    pub fn provider_mode(&self) -> Option<&ProviderMode> {
        self.provider_mode.as_ref()
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
    ///
    /// Uses cache if available and not expired. Falls back to builtin model list
    /// for BigModel providers when the API endpoint is unavailable.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        // Check cache first
        {
            let cache = self.model_cache.read().unwrap();
            if let Some(ref entry) = *cache {
                if entry.is_valid() {
                    return Ok(entry.models());
                }
            }
        }

        // Fetch from API with 429 retry
        let mut attempts = 0;
        let max_retries = 3;
        let mut delay = Duration::from_secs(1);

        loop {
            match self.provider.list_models().await {
                Ok(models) if !models.is_empty() => {
                    // Update cache
                    let mut cache = self.model_cache.write().unwrap();
                    *cache = Some(ModelCacheEntry::new(models.clone(), self.cache_ttl));
                    return Ok(models);
                }
                Ok(_) => {
                    // Empty models list - try fallback
                    break;
                }
                Err(e) => {
                    attempts += 1;
                    let err_str = e.to_string();

                    // Check if it's a 429 error
                    if err_str.contains("429")
                        || err_str.to_lowercase().contains("too many requests")
                    {
                        if attempts >= max_retries {
                            // Return stale cache if available
                            let cache = self.model_cache.read().unwrap();
                            if let Some(ref entry) = *cache {
                                return Ok(entry.models());
                            }
                            return Err(anyhow::anyhow!(
                                "Rate limited (429). Please wait a moment before trying again."
                            ));
                        }

                        // Wait and retry
                        tracing::warn!(
                            "Rate limited (429), retrying after {} seconds (attempt {}/{})",
                            delay.as_secs(),
                            attempts,
                            max_retries
                        );
                        tokio::time::sleep(delay).await;
                        delay = delay.mul_f32(2.0).min(Duration::from_secs(60));
                        continue;
                    }

                    if attempts >= max_retries {
                        break;
                    }
                    attempts += 1;
                    tokio::time::sleep(delay).await;
                    delay = delay.mul_f32(2.0).min(Duration::from_secs(60));
                }
            }
        }

        // Fallback logic
        {
            let cache = self.model_cache.read().unwrap();
            if let Some(ref entry) = *cache {
                return Ok(entry.models());
            }
        }

        Err(anyhow::anyhow!("Failed to fetch model list from provider"))
    }

    /// List available models using a synchronous HTTP client.
    ///
    /// This is safe to call from within `spawn_blocking` or any synchronous
    /// context where creating a tokio runtime would cause nesting panics.
    /// Uses `reqwest::blocking` to perform a single GET request.
    pub fn list_models_blocking(&self, base_url: &str, api_key: &str) -> Vec<String> {
        // Check cache first
        {
            let cache = self.model_cache.read().unwrap();
            if let Some(ref entry) = *cache {
                if entry.is_valid() {
                    return entry.models();
                }
            }
        }

        let client = reqwest::blocking::Client::builder()
            .user_agent(crate::http::user_agent())
            .build()
            .expect("failed to build model list HTTP client");

        let models = if uses_anthropic_protocol(&self.provider_type, &self.provider_mode) {
            let base_url = normalize_anthropic_api_base(base_url);
            let url = format!("{}/v1/models", base_url);
            let resp = client
                .get(&url)
                .header("x-api-key", api_key)
                .header("anthropic-version", "2023-06-01")
                .send();
            match resp {
                Ok(resp) if resp.status().is_success() => parse_models_json(resp.json().ok()),
                Ok(resp) if resp.status().as_u16() == 429 => {
                    // Rate limited - return stale cache if available
                    let cache = self.model_cache.read().unwrap();
                    if let Some(ref entry) = *cache {
                        tracing::warn!("Rate limited (429), returning cached model list");
                        return entry.models();
                    }
                    None
                }
                _ => None,
            }
        } else {
            let base_url = normalize_openai_api_base(base_url);
            let url = format!("{}/models", base_url);
            let resp = client.get(&url).bearer_auth(api_key).send();
            match resp {
                Ok(resp) if resp.status().is_success() => parse_models_json(resp.json().ok()),
                Ok(resp) if resp.status().as_u16() == 429 => {
                    // Rate limited - return stale cache if available
                    let cache = self.model_cache.read().unwrap();
                    if let Some(ref entry) = *cache {
                        tracing::warn!("Rate limited (429), returning cached model list");
                        return entry.models();
                    }
                    None
                }
                _ => None,
            }
        };

        match models {
            Some(m) if !m.is_empty() => {
                // Update cache
                let mut cache = self.model_cache.write().unwrap();
                *cache = Some(ModelCacheEntry::new(m.clone(), self.cache_ttl));
                m
            }
            _ => {
                // Return stale cache if available
                let cache = self.model_cache.read().unwrap();
                if let Some(ref entry) = *cache {
                    tracing::warn!("API request failed, returning cached model list");
                    entry.models()
                } else {
                    Vec::new()
                }
            }
        }
    }

    /// Clear the model cache (useful when configuration changes).
    pub fn clear_model_cache(&self) {
        let mut cache = self.model_cache.write().unwrap();
        *cache = None;
    }

    /// Stream a chat completion with tool calling support.
    pub async fn chat_stream_with_tools(
        &self,
        conversation: &Conversation,
        tools: Vec<ToolDef>,
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        self.provider
            .chat_stream_with_tools(conversation, &tools)
            .await
    }

    /// Continue the conversation with tool execution results.
    pub async fn continue_with_tool_results(
        &self,
        conversation: &Conversation,
        tool_results: Vec<(String, String, String)>,
        assistant_tool_calls: Vec<ToolCall>,
        tools: Vec<ToolDef>,
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        self.provider
            .continue_with_tool_results(conversation, &tool_results, &assistant_tool_calls, &tools)
            .await
    }
}

fn uses_anthropic_protocol(
    provider_type: &ProviderType,
    provider_mode: &Option<ProviderMode>,
) -> bool {
    matches!(provider_type, ProviderType::Anthropic)
        || matches!(
            (provider_type, provider_mode),
            (
                ProviderType::BigModel | ProviderType::XiaomiMimo | ProviderType::DeepSeek,
                Some(ProviderMode::Native)
            )
        )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::sync::mpsc::{self, Receiver};
    use std::time::Duration;

    fn settings(provider_mode: ProviderMode, base_url: String) -> LlmSettings {
        LlmSettings {
            provider: ProviderType::XiaomiMimo,
            provider_mode: Some(provider_mode),
            model: String::new(),
            api_key: Some("test-key".to_string()),
            api_key_env: None,
            base_url: Some(base_url),
            review_model: None,
        }
    }

    fn spawn_model_server() -> Option<(String, Receiver<String>)> {
        let listener = match TcpListener::bind("127.0.0.1:0") {
            Ok(listener) => listener,
            Err(_) => return None,
        };
        let origin = format!("http://{}", listener.local_addr().expect("local addr"));
        let (tx, rx) = mpsc::channel();

        std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().expect("accept request");
            let mut buffer = [0u8; 8192];
            let read = stream.read(&mut buffer).expect("read request");
            let request = String::from_utf8_lossy(&buffer[..read]).to_string();
            tx.send(request).expect("send captured request");

            let body = r#"{"data":[{"id":"mimo-test"}]}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                body.len(),
                body
            );
            stream
                .write_all(response.as_bytes())
                .expect("write response");
        });

        Some((origin, rx))
    }

    #[test]
    fn xiaomi_sgp_openai_model_fetch_uses_bearer_auth() {
        let Some((origin, rx)) = spawn_model_server() else {
            return;
        };
        let base_url = format!("{origin}/v1");
        let client = LlmClient::new(settings(ProviderMode::OpenAICompatible, base_url.clone()))
            .expect("client");

        let models = client.list_models_blocking(&base_url, "test-key");

        assert_eq!(models, vec!["mimo-test"]);
        let request = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured request");
        let lower = request.to_ascii_lowercase();
        assert!(request.starts_with("GET /v1/models "));
        assert!(lower.contains("authorization: bearer test-key"));
    }

    #[test]
    fn xiaomi_sgp_anthropic_model_fetch_uses_x_api_key_auth() {
        let Some((origin, rx)) = spawn_model_server() else {
            return;
        };
        let base_url = format!("{origin}/anthropic");
        let client =
            LlmClient::new(settings(ProviderMode::Native, base_url.clone())).expect("client");

        let models = client.list_models_blocking(&base_url, "test-key");

        assert_eq!(models, vec!["mimo-test"]);
        let request = rx
            .recv_timeout(Duration::from_secs(2))
            .expect("captured request");
        let lower = request.to_ascii_lowercase();
        assert!(request.starts_with("GET /anthropic/v1/models "));
        assert!(lower.contains("x-api-key: test-key"));
        assert!(lower.contains("anthropic-version: 2023-06-01"));
        assert!(!lower.contains("authorization:"));
    }
}
