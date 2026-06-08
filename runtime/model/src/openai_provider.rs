//! OpenAI-compatible provider implementation.

use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestAssistantMessageArgs, ChatCompletionRequestMessage,
        ChatCompletionRequestSystemMessageArgs, ChatCompletionRequestToolMessageArgs,
        ChatCompletionRequestUserMessageArgs, ChatCompletionToolArgs, ChatCompletionToolType,
        CreateChatCompletionRequestArgs, FunctionObjectArgs,
    },
    Client,
};

use protocol_interface::{MessageRole, ToolDef};
use runtime_config::LlmSettings;

use crate::endpoint::normalize_openai_api_base;
use crate::provider::{ChatStream, LlmProvider};
use crate::{ChatEvent, Conversation, ToolCall};

/// OpenAI-compatible LLM provider.
///
/// Wraps the `async-openai` client for use with any OpenAI-compatible API endpoint.
pub struct OpenAiProvider {
    inner: Client<OpenAIConfig>,
    model: String,
}

impl OpenAiProvider {
    /// Create a new OpenAI provider from LLM settings.
    pub fn new(config: &LlmSettings) -> Result<Self> {
        let api_key = config.get_api_key().ok_or_else(|| {
            anyhow::anyhow!("API key not found. Set it in config or environment variable.")
        })?;

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(normalize_openai_api_base(&config.get_base_url()));

        let client = Client::with_config(openai_config);

        Ok(Self {
            inner: client,
            model: config.model.clone(),
        })
    }

    /// Build OpenAI-compatible messages from the conversation history.
    fn build_messages(&self, conversation: &Conversation) -> Vec<ChatCompletionRequestMessage> {
        let mut messages = Vec::new();

        if let Some(system) = conversation.system_prompt() {
            messages.push(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system)
                    .build()
                    .expect("Failed to build system message")
                    .into(),
            );
        }

        for msg in conversation.messages() {
            match msg.role {
                MessageRole::System => {
                    messages.push(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(&msg.content)
                            .build()
                            .expect("Failed to build system message")
                            .into(),
                    );
                }
                MessageRole::User => {
                    messages.push(
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(msg.content.as_str())
                            .build()
                            .expect("Failed to build user message")
                            .into(),
                    );
                }
                MessageRole::Assistant => {
                    messages.push(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(&msg.content)
                            .build()
                            .expect("Failed to build assistant message")
                            .into(),
                    );
                }
                MessageRole::Summary => {
                    messages.push(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(&msg.content)
                            .build()
                            .expect("Failed to build summary message")
                            .into(),
                    );
                }
            }
        }

        messages
    }

    /// Convert ToolDef list to OpenAI tool format.
    fn build_tools(&self, tools: &[ToolDef]) -> Vec<serde_json::Value> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "type": "function",
                    "function": {
                        "name": t.name,
                        "description": t.description,
                        "parameters": t.parameters,
                    }
                })
            })
            .collect()
    }

    /// Execute a tool-calling request and return (synthetic_stream, tool_calls).
    async fn do_tool_request(
        &self,
        messages: Vec<ChatCompletionRequestMessage>,
        tools: &[ToolDef],
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        let openai_tools: Vec<_> = self
            .build_tools(tools)
            .into_iter()
            .filter_map(|t| {
                let name = t["function"]["name"].as_str()?.to_string();
                let description = t["function"]["description"].as_str()?;
                let parameters = t["function"]["parameters"].clone();

                ChatCompletionToolArgs::default()
                    .r#type(ChatCompletionToolType::Function)
                    .function(
                        FunctionObjectArgs::default()
                            .name(name)
                            .description(description)
                            .parameters(parameters)
                            .build()
                            .ok()?,
                    )
                    .build()
                    .ok()
            })
            .collect();

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.model)
            .messages(messages)
            .tools(openai_tools)
            .stream(false)
            .build()?;

        let response = self.inner.chat().create(request).await?;

        let choice = response
            .choices
            .first()
            .ok_or_else(|| anyhow::anyhow!("No response choices"))?;

        let tool_calls = choice.message.tool_calls.as_ref().map(|calls| {
            calls
                .iter()
                .map(|tc| {
                    ToolCall::new(
                        tc.id.clone(),
                        tc.function.name.clone(),
                        serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Null),
                    )
                })
                .collect()
        });

        let text = choice.message.content.clone().unwrap_or_default();
        let events: Vec<Result<ChatEvent>> = if text.is_empty() && tool_calls.is_none() {
            vec![Ok(ChatEvent::Done {
                full_response: String::new(),
            })]
        } else {
            vec![
                Ok(ChatEvent::Delta { text: text.clone() }),
                Ok(ChatEvent::Done {
                    full_response: text,
                }),
            ]
        };

        Ok((Box::pin(futures::stream::iter(events)), tool_calls))
    }
}

impl LlmProvider for OpenAiProvider {
    fn chat_stream<'a>(
        &'a self,
        conversation: &'a Conversation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatStream>> + Send + 'a>> {
        Box::pin(async move {
            let inner = self.inner.clone();
            let model = self.model.clone();
            let mut attempts = 0;
            let max_retries = 3;
            let mut delay = std::time::Duration::from_secs(1);

            loop {
                let messages = self.build_messages(conversation);

                let request = CreateChatCompletionRequestArgs::default()
                    .model(&model)
                    .messages(messages)
                    .stream(true)
                    .build()?;

                match inner.chat().create_stream(request).await {
                    Ok(stream) => {
                        return Ok(Box::pin(crate::events::process_stream(stream)) as ChatStream)
                    }
                    Err(e) => {
                        attempts += 1;
                        let err_str = e.to_string();

                        if (err_str.contains("429")
                            || err_str.to_lowercase().contains("too many requests"))
                            && attempts <= max_retries
                        {
                            tracing::warn!(
                                "Rate limited (429), retrying after {} seconds (attempt {}/{})",
                                delay.as_secs(),
                                attempts,
                                max_retries
                            );
                            tokio::time::sleep(delay).await;
                            delay = std::cmp::min(
                                delay.mul_f32(2.0),
                                std::time::Duration::from_secs(60),
                            );
                            continue;
                        }

                        if attempts > max_retries {
                            return Err(anyhow::anyhow!(e));
                        }

                        tokio::time::sleep(delay).await;
                        delay =
                            std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                    }
                }
            }
        })
    }

    fn chat_once<'a>(
        &'a self,
        prompt: &'a str,
        system: Option<&'a str>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let mut messages = Vec::new();

            if let Some(sys) = system {
                messages.push(
                    ChatCompletionRequestSystemMessageArgs::default()
                        .content(sys)
                        .build()?
                        .into(),
                );
            }

            messages.push(
                ChatCompletionRequestUserMessageArgs::default()
                    .content(prompt)
                    .build()?
                    .into(),
            );

            let request = CreateChatCompletionRequestArgs::default()
                .model(&self.model)
                .messages(messages)
                .build()?;

            let response = self.inner.chat().create(request).await?;

            let content = response
                .choices
                .first()
                .and_then(|c| c.message.content.clone())
                .ok_or_else(|| anyhow::anyhow!("No response content"))?;

            Ok(content)
        })
    }

    fn list_models<'a>(
        &'a self,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<Vec<String>>> + Send + 'a>> {
        Box::pin(async move {
            let mut attempts = 0;
            let max_retries = 3;
            let mut delay = std::time::Duration::from_secs(1);

            loop {
                match self.inner.models().list().await {
                    Ok(models) => {
                        let model_ids: Vec<String> =
                            models.data.into_iter().map(|m| m.id).collect();
                        return Ok(model_ids);
                    }
                    Err(e) => {
                        attempts += 1;
                        let err_str = e.to_string();

                        // Check if it's a 429 error
                        if err_str.contains("429")
                            || err_str.to_lowercase().contains("too many requests")
                        {
                            if attempts >= max_retries {
                                return Err(anyhow::anyhow!(
                                    "Rate limited (429) after {} retries. Please wait before trying again.",
                                    max_retries
                                ));
                            }

                            tracing::warn!(
                                "Rate limited (429), retrying after {} seconds (attempt {}/{})",
                                delay.as_secs(),
                                attempts,
                                max_retries
                            );
                            tokio::time::sleep(delay).await;
                            delay = std::cmp::min(
                                delay.mul_f32(2.0),
                                std::time::Duration::from_secs(60),
                            );
                            continue;
                        }

                        if attempts >= max_retries {
                            return Err(anyhow::anyhow!(e));
                        }

                        attempts += 1;
                        tokio::time::sleep(delay).await;
                        delay =
                            std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                    }
                }
            }
        })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        conversation: &'a Conversation,
        tools: &'a [ToolDef],
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let messages = self.build_messages(conversation);
            self.do_tool_request(messages, tools).await
        })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        conversation: &'a Conversation,
        tool_results: &'a [(String, String, String)],
        tools: &'a [ToolDef],
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let mut messages = self.build_messages(conversation);

            for (call_id, _tool_name, result) in tool_results {
                messages.push(
                    ChatCompletionRequestToolMessageArgs::default()
                        .tool_call_id(call_id)
                        .content(result.as_str())
                        .build()?
                        .into(),
                );
            }

            self.do_tool_request(messages, tools).await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::ProviderType;
    use runtime_config::LlmSettings;

    fn test_settings() -> LlmSettings {
        LlmSettings {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "gpt-4o-mini".to_string(),
            api_key: Some("test-key".to_string()),
            api_key_env: None,
            base_url: None,
            review_model: None,
        }
    }

    #[test]
    fn test_openai_provider_new() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings);
        assert!(provider.is_ok());
    }

    #[test]
    fn test_openai_provider_new_missing_key() {
        let settings = LlmSettings {
            provider: ProviderType::Openai,
            provider_mode: None,
            model: "gpt-4o".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        let provider = OpenAiProvider::new(&settings);
        assert!(provider.is_err());
    }

    #[test]
    fn test_openai_build_messages_empty() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let conversation = Conversation::new(None);
        let messages = provider.build_messages(&conversation);
        assert!(messages.is_empty());
    }

    #[test]
    fn test_openai_build_messages_with_system() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let conversation = Conversation::new(Some("You are helpful".to_string()));
        let messages = provider.build_messages(&conversation);
        assert_eq!(messages.len(), 1);
    }

    #[test]
    fn test_openai_build_tools() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let tools = vec![ToolDef {
            name: "read_file".to_string(),
            description: "Read a file".to_string(),
            parameters: serde_json::json!({"type": "object"}),
        }];
        let result = provider.build_tools(&tools);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0]["function"]["name"], "read_file");
    }
}
