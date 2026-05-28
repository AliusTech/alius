use async_openai::{Client, config::OpenAIConfig};
use crate::config::Settings;
use crate::error::{Result, AliusError};

/// LLM client for communicating with OpenAI-compatible API endpoints.
///
/// Wraps the `async-openai` client with configuration from `Settings`.
/// Supports any provider that implements the OpenAI chat completion API
/// (OpenAI, Anthropic via proxy, Google via proxy, etc.).
pub struct LlmClient {
    /// The underlying OpenAI-compatible HTTP client.
    client: Client<OpenAIConfig>,
    /// The model identifier to use for chat completions.
    model: String,
}

impl LlmClient {
    /// Create a new LLM client from application settings.
    ///
    /// Reads the API key and base URL from settings. The API key is resolved
    /// from the direct config value or the environment variable.
    pub fn new(settings: &Settings) -> Result<Self> {
        let api_key = settings.api_key()?;
        let base_url = settings.effective_base_url();

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(&base_url);

        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: settings.llm.model.clone(),
        })
    }

    /// Create a client specifically for listing available models.
    ///
    /// Unlike `new()`, this doesn't require a specific model to be configured.
    /// The model field is left empty since it's not needed for the models endpoint.
    pub fn for_model_list(settings: &Settings) -> Result<Self> {
        let api_key = settings.api_key()?;
        let base_url = settings.effective_base_url();

        let config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(&base_url);

        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: String::new(),
        })
    }

    /// Send a chat completion request and return the assistant's response.
    ///
    /// Creates a single-message chat completion request with the given prompt
    /// as the user message. Returns the content of the first choice in the response.
    ///
    /// # Arguments
    /// * `prompt` - The user's input text to send to the LLM.
    ///
    /// # Returns
    /// The assistant's response text, or an error if the request fails.
    pub async fn chat(&self, prompt: &str) -> Result<String> {
        use async_openai::types::{
            ChatCompletionRequestMessage,
            ChatCompletionRequestUserMessageArgs,
            CreateChatCompletionRequestArgs,
        };

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(vec![ChatCompletionRequestMessage::User(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt)
                    .build()
                    .map_err(|e| AliusError::Llm(e.to_string()))?,
            )])
            .build()
            .map_err(|e| AliusError::Llm(e.to_string()))?;

        let response = self
            .client
            .chat()
            .create(request)
            .await
            .map_err(|e| AliusError::Llm(e.to_string()))?;

        // Extract the content from the first choice, defaulting to empty string
        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_default();

        Ok(content)
    }

    /// List all available models from the LLM provider.
    ///
    /// Queries the `/models` endpoint and returns a list of model identifiers.
    /// This is used to populate the model selection list in the REPL.
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let models = self
            .client
            .models()
            .list()
            .await
            .map_err(|e| AliusError::Llm(e.to_string()))?;

        let model_ids: Vec<String> = models
            .data
            .iter()
            .map(|m| m.id.clone())
            .collect();

        Ok(model_ids)
    }
}
