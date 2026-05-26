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

        let mut config = OpenAIConfig::new().with_api_key(api_key);

        if let Some(base_url) = &settings.llm.base_url {
            config = config.with_api_base(base_url);
        }

        let client = Client::with_config(config);

        Ok(Self {
            client,
            model: settings.llm.model.clone(),
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
}