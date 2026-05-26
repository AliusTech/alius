//! LLM client

use async_openai::{
    Client,
    config::OpenAIConfig,
    types::{
        ChatCompletionRequestMessage,
        CreateChatCompletionRequestArgs,
        ChatCompletionRequestUserMessageArgs,
        ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs,
        ChatCompletionToolArgs,
        ChatCompletionToolType,
        FunctionObjectArgs,
    },
};
use futures::Stream;
use anyhow::Result;
use serde_json::Value as JsonValue;

use alius_config::LlmSettings;
use alius_protocol::MessageRole;

use crate::{ChatEvent, Conversation, ToolCall};

/// LLM client with streaming support
pub struct LlmClient {
    inner: Client<OpenAIConfig>,
    config: LlmSettings,
}

impl LlmClient {
    /// Create a new LLM client
    pub fn new(config: LlmSettings) -> Result<Self> {
        let api_key = config.get_api_key().ok_or_else(|| {
            anyhow::anyhow!("API key not found. Set it in config or environment variable.")
        })?;

        let openai_config = OpenAIConfig::new()
            .with_api_key(api_key)
            .with_api_base(config.get_base_url());

        let client = Client::with_config(openai_config);

        Ok(Self { inner: client, config })
    }

    /// Get the configured model
    pub fn model(&self) -> &str {
        &self.config.model
    }

    /// Stream a chat completion
    pub async fn chat_stream(
        &self,
        conversation: &Conversation,
    ) -> Result<impl Stream<Item = Result<ChatEvent>>> {
        let messages = self.build_messages(conversation);

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(messages)
            .stream(true)
            .build()?;

        let stream = self.inner.chat().create_stream(request).await?;

        Ok(crate::events::process_stream(stream))
    }

    /// Single-shot chat (for run command)
    pub async fn chat_once(
        &self,
        prompt: &str,
        system: Option<&str>,
    ) -> Result<String> {
        let mut messages = Vec::new();

        if let Some(sys) = system {
            messages.push(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(sys)
                    .build()?
                    .into()
            );
        }

        messages.push(
            ChatCompletionRequestUserMessageArgs::default()
                .content(prompt)
                .build()?
                .into()
        );

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(messages)
            .build()?;

        let response = self.inner.chat().create(request).await?;

        let content = response.choices
            .first()
            .and_then(|c| c.message.content.clone())
            .ok_or_else(|| anyhow::anyhow!("No response content"))?;

        Ok(content)
    }

    /// Build OpenAI messages from conversation
    fn build_messages(&self, conversation: &Conversation) -> Vec<ChatCompletionRequestMessage> {
        let mut messages = Vec::new();

        // Add system prompt if present
        if let Some(system) = conversation.system_prompt() {
            messages.push(
                ChatCompletionRequestSystemMessageArgs::default()
                    .content(system)
                    .build()
                    .expect("Failed to build system message")
                    .into()
            );
        }

        // Add conversation messages
        for msg in conversation.messages() {
            match msg.role {
                MessageRole::System => {
                    messages.push(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(&msg.content)
                            .build()
                            .expect("Failed to build system message")
                            .into()
                    );
                }
                MessageRole::User => {
                    messages.push(
                        ChatCompletionRequestUserMessageArgs::default()
                            .content(msg.content.as_str())
                            .build()
                            .expect("Failed to build user message")
                            .into()
                    );
                }
                MessageRole::Assistant => {
                    messages.push(
                        ChatCompletionRequestAssistantMessageArgs::default()
                            .content(&msg.content)
                            .build()
                            .expect("Failed to build assistant message")
                            .into()
                    );
                }
                MessageRole::Summary => {
                    // Treat summary as system message
                    messages.push(
                        ChatCompletionRequestSystemMessageArgs::default()
                            .content(&msg.content)
                            .build()
                            .expect("Failed to build summary message")
                            .into()
                    );
                }
            }
        }

        messages
    }

    /// List available models
    pub async fn list_models(&self) -> Result<Vec<String>> {
        let models = self.inner.models().list().await?;
        let model_ids: Vec<String> = models.data
            .into_iter()
            .map(|m| m.id)
            .collect();
        Ok(model_ids)
    }

    /// Stream a chat completion with tools
    pub async fn chat_stream_with_tools(
        &self,
        conversation: &Conversation,
        tools: Vec<JsonValue>,
    ) -> Result<(impl Stream<Item = Result<ChatEvent>>, Option<Vec<ToolCall>>)> {
        let messages = self.build_messages(conversation);

        // Build tool definitions
        let openai_tools: Vec<_> = tools
            .into_iter()
            .filter_map(|t| {
                let name = t["function"]["name"].as_str()?.to_string();
                let description = t["function"]["description"].as_str()?;
                let parameters = t["function"]["parameters"].clone();

                ChatCompletionToolArgs::default()
                    .r#type(ChatCompletionToolType::Function)
                    .function(FunctionObjectArgs::default()
                        .name(name)
                        .description(description)
                        .parameters(parameters)
                        .build()
                        .ok()?
                    )
                    .build()
                    .ok()
            })
            .collect();

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(messages)
            .tools(openai_tools)
            .stream(false) // Tools need non-streaming for now
            .build()?;

        // Use non-streaming for tool support
        let response = self.inner.chat().create(request).await?;

        let choice = response.choices.first().ok_or_else(|| {
            anyhow::anyhow!("No response choices")
        })?;

        // Check for tool calls
        let tool_calls = choice.message.tool_calls.as_ref().map(|calls| {
            calls.iter().map(|tc| {
                ToolCall::new(
                    tc.id.clone(),
                    tc.function.name.clone(),
                    serde_json::from_str(&tc.function.arguments).unwrap_or(JsonValue::Null),
                )
            }).collect()
        });

        // Create a synthetic stream for the text response
        let text = choice.message.content.clone().unwrap_or_default();
        let events: Vec<Result<ChatEvent>> = if text.is_empty() && tool_calls.is_none() {
            vec![Ok(ChatEvent::Done { full_response: String::new() })]
        } else {
            vec![Ok(ChatEvent::Delta { text: text.clone() }), Ok(ChatEvent::Done { full_response: text })]
        };

        Ok((futures::stream::iter(events), tool_calls))
    }

    /// Send tool results back to the model
    pub async fn continue_with_tool_results(
        &self,
        conversation: &Conversation,
        tool_results: Vec<(String, String, String)>, // (tool_call_id, tool_name, result)
        tools: Vec<JsonValue>,
    ) -> Result<(impl Stream<Item = Result<ChatEvent>>, Option<Vec<ToolCall>>)> {
        let mut messages = self.build_messages(conversation);

        // Add tool result messages
        for (call_id, _tool_name, result) in tool_results {
            messages.push(
                ChatCompletionRequestToolMessageArgs::default()
                    .tool_call_id(call_id)
                    .content(result)
                    .build()?
                    .into()
            );
        }

        // Build tool definitions
        let openai_tools: Vec<_> = tools
            .into_iter()
            .filter_map(|t| {
                let name = t["function"]["name"].as_str()?.to_string();
                let description = t["function"]["description"].as_str()?;
                let parameters = t["function"]["parameters"].clone();

                ChatCompletionToolArgs::default()
                    .r#type(ChatCompletionToolType::Function)
                    .function(FunctionObjectArgs::default()
                        .name(name)
                        .description(description)
                        .parameters(parameters)
                        .build()
                        .ok()?
                    )
                    .build()
                    .ok()
            })
            .collect();

        let request = CreateChatCompletionRequestArgs::default()
            .model(&self.config.model)
            .messages(messages)
            .tools(openai_tools)
            .stream(false)
            .build()?;

        let response = self.inner.chat().create(request).await?;

        let choice = response.choices.first().ok_or_else(|| {
            anyhow::anyhow!("No response choices")
        })?;

        let tool_calls = choice.message.tool_calls.as_ref().map(|calls| {
            calls.iter().map(|tc| {
                ToolCall::new(
                    tc.id.clone(),
                    tc.function.name.clone(),
                    serde_json::from_str(&tc.function.arguments).unwrap_or(JsonValue::Null),
                )
            }).collect()
        });

        let text = choice.message.content.clone().unwrap_or_default();
        let events: Vec<Result<ChatEvent>> = if text.is_empty() && tool_calls.is_none() {
            vec![Ok(ChatEvent::Done { full_response: String::new() })]
        } else {
            vec![Ok(ChatEvent::Delta { text: text.clone() }), Ok(ChatEvent::Done { full_response: text })]
        };

        Ok((futures::stream::iter(events), tool_calls))
    }
}