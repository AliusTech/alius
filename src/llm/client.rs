use async_openai::{Client, config::OpenAIConfig};
use crate::config::Settings;
use crate::error::{Result, AliusError};

pub struct LlmClient {
    client: Client<OpenAIConfig>,
    model: String,
}

impl LlmClient {
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

        let content = response
            .choices
            .first()
            .and_then(|choice| choice.message.content.clone())
            .unwrap_or_default();

        Ok(content)
    }

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