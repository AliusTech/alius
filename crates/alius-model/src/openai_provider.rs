//! OpenAI-compatible provider implementation.

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
use anyhow::Result;

use alius_config::LlmSettings;
use alius_protocol::{MessageRole, ToolDef};

use crate::{ChatEvent, Conversation, ToolCall};
use crate::provider::{LlmProvider, ChatStream};

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
            .with_api_base(config.get_base_url());

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
                    .into()
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

    /// Convert ToolDef list to OpenAI tool format.
    fn build_tools(&self, tools: &[ToolDef]) -> Vec<serde_json::Value> {
        tools.iter().map(|t| {
            serde_json::json!({
                "type": "function",
                "function": {
                    "name": t.name,
                    "description": t.description,
                    "parameters": t.parameters,
                }
            })
        }).collect()
    }

    /// Execute a tool-calling request and return (synthetic_stream, tool_calls).
    async fn do_tool_request(
        &self,
        messages: Vec<ChatCompletionRequestMessage>,
        tools: &[ToolDef],
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        let openai_tools: Vec<_> = self.build_tools(tools)
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
            .model(&self.model)
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
                    serde_json::from_str(&tc.function.arguments).unwrap_or(serde_json::Value::Null),
                )
            }).collect()
        });

        let text = choice.message.content.clone().unwrap_or_default();
        let events: Vec<Result<ChatEvent>> = if text.is_empty() && tool_calls.is_none() {
            vec![Ok(ChatEvent::Done { full_response: String::new() })]
        } else {
            vec![Ok(ChatEvent::Delta { text: text.clone() }), Ok(ChatEvent::Done { full_response: text })]
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
            let messages = self.build_messages(conversation);

            let request = CreateChatCompletionRequestArgs::default()
                .model(&self.model)
                .messages(messages)
                .stream(true)
                .build()?;

            let stream = self.inner.chat().create_stream(request).await?;

            Ok(Box::pin(crate::events::process_stream(stream)) as ChatStream)
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
                .model(&self.model)
                .messages(messages)
                .build()?;

            let response = self.inner.chat().create(request).await?;

            let content = response.choices
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
            let models = self.inner.models().list().await?;
            let model_ids: Vec<String> = models.data
                .into_iter()
                .map(|m| m.id)
                .collect();
            Ok(model_ids)
        })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        conversation: &'a Conversation,
        tools: &'a [ToolDef],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>> + Send + 'a>> {
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>> + Send + 'a>> {
        Box::pin(async move {
            let mut messages = self.build_messages(conversation);

            for (call_id, _tool_name, result) in tool_results {
                messages.push(
                    ChatCompletionRequestToolMessageArgs::default()
                        .tool_call_id(call_id)
                        .content(result.as_str())
                        .build()?
                        .into()
                );
            }

            self.do_tool_request(messages, tools).await
        })
    }
}
