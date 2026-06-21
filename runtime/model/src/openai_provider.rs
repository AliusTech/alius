//! OpenAI-compatible provider implementation.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use async_openai::{
    config::OpenAIConfig,
    types::{
        ChatCompletionMessageToolCall, ChatCompletionRequestAssistantMessageArgs,
        ChatCompletionRequestMessage, ChatCompletionRequestSystemMessageArgs,
        ChatCompletionRequestToolMessageArgs, ChatCompletionRequestUserMessageArgs,
        ChatCompletionToolArgs, ChatCompletionToolType, CreateChatCompletionRequestArgs,
        FunctionCall, FunctionObjectArgs,
    },
    Client,
};

use protocol_interface::{MessageRole, ToolDef};
use runtime_config::LlmSettings;

use crate::endpoint::normalize_openai_api_base;
use crate::provider::{ChatStream, LlmProvider};
use crate::{ChatEvent, Conversation, ToolCall};

/// Parse model IDs from a JSON value like `{"data": [{"id": "..."}]}`.
///
/// Only requires `data[].id` — ignores `created`, `object`, `owned_by` etc.
/// This tolerates providers (e.g. DeepSeek) that omit fields the `async-openai`
/// `Model` struct requires.
fn parse_models_value(body: &serde_json::Value) -> Option<Vec<String>> {
    body.get("data")?.as_array().map(|arr| {
        arr.iter()
            .filter_map(|m| m.get("id").and_then(|id| id.as_str()).map(String::from))
            .collect()
    })
}

/// BigModel General API builtin model list.
const BIGMODEL_GENERAL_MODELS: &[&str] = &[
    "glm-5.1",
    "glm-5",
    "glm-5-air",
    "glm-4.7",
    "glm-4.7-flash",
    "glm-4.6",
    "glm-4.6v",
    "glm-4.5",
    "glm-4.5-air",
    "glm-4-flash-250414",
];

/// BigModel Coding Plan builtin model list.
const BIGMODEL_CODING_MODELS: &[&str] = &[
    "GLM-5.1",
    "GLM-5-Turbo",
    "GLM-4.7",
    "GLM-4.5-Air",
    "glm-5.1",
    "glm-5-turbo",
    "glm-4.7",
    "glm-4.5-air",
];

/// DeepSeek builtin model list.
const DEEPSEEK_MODELS: &[&str] = &["deepseek-chat", "deepseek-reasoner"];

/// OpenAI-compatible LLM provider.
///
/// Serves as the base for all OpenAI-compatible providers. Vendor-specific
/// behavior (default models) is configured via constructor variants.
pub struct OpenAiProvider {
    inner: Client<OpenAIConfig>,
    model: String,
    api_key: String,
    api_base: String,
    default_models: Vec<String>,
}

impl OpenAiProvider {
    /// Create a generic OpenAI-compatible provider.
    pub fn new(config: &LlmSettings) -> Result<Self> {
        Self::build(config, &[])
    }

    /// Create a BigModel provider (General mode).
    pub fn new_bigmodel(config: &LlmSettings) -> Result<Self> {
        Self::build(config, BIGMODEL_GENERAL_MODELS)
    }

    /// Create a BigModel provider (Coding mode).
    pub fn new_bigmodel_coding(config: &LlmSettings) -> Result<Self> {
        Self::build(config, BIGMODEL_CODING_MODELS)
    }

    /// Create a DeepSeek provider.
    pub fn new_deepseek(config: &LlmSettings) -> Result<Self> {
        Self::build(config, DEEPSEEK_MODELS)
    }

    /// Internal constructor shared by all variants.
    fn build(config: &LlmSettings, default_models: &[&str]) -> Result<Self> {
        let api_key = config.get_api_key().ok_or_else(|| {
            anyhow::anyhow!("API key not found. Set it in config or environment variable.")
        })?;

        let openai_config = OpenAIConfig::new()
            .with_api_key(&api_key)
            .with_api_base(normalize_openai_api_base(&config.get_base_url()));

        let http_client = reqwest::Client::builder()
            .user_agent(crate::http::user_agent())
            .build()?;
        let client = Client::with_config(openai_config).with_http_client(http_client);

        Ok(Self {
            inner: client,
            model: config.model.clone(),
            api_key,
            api_base: normalize_openai_api_base(&config.get_base_url()),
            default_models: default_models.iter().map(|s| s.to_string()).collect(),
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
                    if let Some(calls) = &msg.tool_calls {
                        let tool_calls = calls
                            .iter()
                            .map(|c| {
                                openai_tool_call(c.id.clone(), c.name.clone(), c.arguments.clone())
                            })
                            .collect::<Vec<_>>();
                        messages.push(
                            ChatCompletionRequestAssistantMessageArgs::default()
                                .content(&msg.content)
                                .tool_calls(tool_calls)
                                .build()
                                .expect("Failed to build assistant tool_calls message")
                                .into(),
                        );
                    } else {
                        messages.push(
                            ChatCompletionRequestAssistantMessageArgs::default()
                                .content(&msg.content)
                                .build()
                                .expect("Failed to build assistant message")
                                .into(),
                        );
                    }
                }
                MessageRole::Tool => {
                    if let Some(tool_call_id) = msg.tool_call_id.as_deref() {
                        messages.push(
                            ChatCompletionRequestToolMessageArgs::default()
                                .tool_call_id(tool_call_id)
                                .content(msg.content.as_str())
                                .build()
                                .expect("Failed to build tool message")
                                .into(),
                        );
                    }
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

    fn build_continue_messages(
        &self,
        conversation: &Conversation,
        tool_results: &[(String, String, String)],
        assistant_tool_calls: &[ToolCall],
    ) -> Result<Vec<ChatCompletionRequestMessage>> {
        let normalized_tool_results =
            normalize_provider_tool_results(assistant_tool_calls, tool_results)?;
        let mut messages = self.build_messages(conversation);

        if !conversation_ends_with_tool_calls(conversation, assistant_tool_calls) {
            messages.push(assistant_tool_calls_message(assistant_tool_calls)?);
        }

        for (call_id, _tool_name, result) in normalized_tool_results {
            messages.push(
                ChatCompletionRequestToolMessageArgs::default()
                    .tool_call_id(call_id)
                    .content(result.as_str())
                    .build()?
                    .into(),
            );
        }

        Ok(messages)
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
            let client = reqwest::Client::builder()
                .user_agent(crate::http::user_agent())
                .build()?;
            let url = format!("{}/models", self.api_base);

            let mut attempts = 0;
            let max_retries = 3;
            let mut delay = std::time::Duration::from_secs(1);

            loop {
                match client.get(&url).bearer_auth(&self.api_key).send().await {
                    Ok(resp) if resp.status().is_success() => {
                        match resp.json::<serde_json::Value>().await {
                            Ok(body) if parse_models_value(&body).is_some() => {
                                let models = parse_models_value(&body).unwrap();
                                if models.is_empty() && !self.default_models.is_empty() {
                                    return Ok(self.default_models.clone());
                                }
                                return Ok(models);
                            }
                            Ok(_) => {
                                if !self.default_models.is_empty() {
                                    return Ok(self.default_models.clone());
                                }
                                return Err(anyhow::anyhow!(
                                    "Failed to parse model list from provider"
                                ));
                            }
                            Err(e) => {
                                tracing::warn!("Failed to parse /models response: {}", e);
                                if !self.default_models.is_empty() {
                                    return Ok(self.default_models.clone());
                                }
                                attempts += 1;
                                if attempts >= max_retries {
                                    return Err(anyhow::anyhow!(
                                        "Failed to parse /models response after {} retries",
                                        max_retries
                                    ));
                                }
                                tokio::time::sleep(delay).await;
                                delay = std::cmp::min(
                                    delay.mul_f32(2.0),
                                    std::time::Duration::from_secs(60),
                                );
                            }
                        }
                    }
                    Ok(resp) => {
                        let status = resp.status();
                        attempts += 1;

                        if status.as_u16() == 429 && attempts < max_retries {
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

                        if !self.default_models.is_empty() {
                            tracing::warn!(
                                "GET /models failed ({}), using default model list",
                                status
                            );
                            return Ok(self.default_models.clone());
                        }
                        return Err(anyhow::anyhow!(
                            "GET /models failed with status {} after {} retries",
                            status,
                            max_retries
                        ));
                    }
                    Err(e) => {
                        attempts += 1;
                        if attempts >= max_retries {
                            if !self.default_models.is_empty() {
                                tracing::warn!(
                                    "GET /models failed: {}, using default model list",
                                    e
                                );
                                return Ok(self.default_models.clone());
                            }
                            return Err(anyhow::anyhow!(
                                "GET /models request failed after {} retries: {}",
                                max_retries,
                                e
                            ));
                        }
                        tracing::warn!(
                            "GET /models request error (attempt {}/{}): {}",
                            attempts,
                            max_retries,
                            e
                        );
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
        assistant_tool_calls: &'a [ToolCall],
        tools: &'a [ToolDef],
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>>
                + Send
                + 'a,
        >,
    > {
        Box::pin(async move {
            let messages =
                self.build_continue_messages(conversation, tool_results, assistant_tool_calls)?;
            self.do_tool_request(messages, tools).await
        })
    }
}

fn openai_tool_call(id: String, name: String, arguments: String) -> ChatCompletionMessageToolCall {
    ChatCompletionMessageToolCall {
        id,
        r#type: ChatCompletionToolType::Function,
        function: FunctionCall { name, arguments },
    }
}

fn assistant_tool_calls_message(
    assistant_tool_calls: &[ToolCall],
) -> Result<ChatCompletionRequestMessage> {
    validate_assistant_tool_calls(assistant_tool_calls)?;
    let tool_calls = assistant_tool_calls
        .iter()
        .map(|call| openai_tool_call(call.id.clone(), call.name.clone(), call.args.to_string()))
        .collect::<Vec<_>>();

    Ok(ChatCompletionRequestAssistantMessageArgs::default()
        .content("")
        .tool_calls(tool_calls)
        .build()?
        .into())
}

fn conversation_ends_with_tool_calls(
    conversation: &Conversation,
    assistant_tool_calls: &[ToolCall],
) -> bool {
    let Some(last) = conversation.messages().last() else {
        return false;
    };
    if last.role != MessageRole::Assistant {
        return false;
    }
    let Some(message_tool_calls) = &last.tool_calls else {
        return false;
    };
    if message_tool_calls.len() != assistant_tool_calls.len() {
        return false;
    }
    message_tool_calls
        .iter()
        .map(|call| call.id.as_str())
        .eq(assistant_tool_calls.iter().map(|call| call.id.as_str()))
}

fn validate_assistant_tool_calls(assistant_tool_calls: &[ToolCall]) -> Result<()> {
    if assistant_tool_calls.is_empty() {
        anyhow::bail!("missing assistant tool_calls for tool result continuation");
    }

    let mut seen = HashSet::new();
    for call in assistant_tool_calls {
        if call.id.trim().is_empty() {
            anyhow::bail!(
                "model returned an empty tool_call_id for tool '{}'",
                call.name
            );
        }
        if !seen.insert(call.id.as_str()) {
            anyhow::bail!("model returned duplicate tool_call_id '{}'", call.id);
        }
    }
    Ok(())
}

fn normalize_provider_tool_results(
    assistant_tool_calls: &[ToolCall],
    tool_results: &[(String, String, String)],
) -> Result<Vec<(String, String, String)>> {
    validate_assistant_tool_calls(assistant_tool_calls)?;

    let by_id = tool_results
        .iter()
        .map(|(call_id, name, result)| (call_id.as_str(), (name.as_str(), result.as_str())))
        .collect::<HashMap<_, _>>();

    assistant_tool_calls
        .iter()
        .map(|call| {
            by_id
                .get(call.id.as_str())
                .map(|(name, result)| (call.id.clone(), (*name).to_string(), (*result).to_string()))
                .ok_or_else(|| {
                    anyhow::anyhow!("missing tool result for tool_call_id '{}'", call.id)
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::{MessageToolCall, ProviderType};
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

    #[test]
    fn test_openai_tool_result_follows_tool_call_message() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let mut conversation = Conversation::new(None);
        conversation.add_user_message("读取 README.md 并总结".to_string());
        conversation.add_assistant_with_tools(
            String::new(),
            vec![MessageToolCall {
                id: "call-readme".to_string(),
                name: "read_file".to_string(),
                arguments: r#"{"path":"README.md"}"#.to_string(),
            }],
        );

        let mut messages = provider.build_messages(&conversation);
        messages.push(
            ChatCompletionRequestToolMessageArgs::default()
                .tool_call_id("call-readme")
                .content("# README\n")
                .build()
                .unwrap()
                .into(),
        );
        let values = messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 3);
        assert_eq!(values[0]["role"], "user");
        assert_eq!(values[1]["role"], "assistant");
        assert!(values[1]["tool_calls"].is_array());
        assert_eq!(values[1]["tool_calls"][0]["id"], "call-readme");
        assert_eq!(values[2]["role"], "tool");
        assert_eq!(values[2]["tool_call_id"], "call-readme");
    }

    #[test]
    fn test_openai_rebuilds_missing_assistant_tool_call_frame() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let mut conversation = Conversation::new(None);
        conversation.add_user_message("执行 git clone".to_string());
        let assistant_tool_calls = vec![ToolCall::new(
            "call-shell".to_string(),
            "shell".to_string(),
            serde_json::json!({"command": "git clone https://github.com/lc345/repo.git"}),
        )];
        let tool_results = vec![(
            "call-shell".to_string(),
            "shell".to_string(),
            "[exit:0]\n".to_string(),
        )];

        let messages = provider
            .build_continue_messages(&conversation, &tool_results, &assistant_tool_calls)
            .unwrap();
        let values = messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 3);
        assert_eq!(values[1]["role"], "assistant");
        assert_eq!(values[1]["tool_calls"][0]["id"], "call-shell");
        assert_eq!(values[2]["role"], "tool");
        assert_eq!(values[2]["tool_call_id"], "call-shell");
    }

    #[test]
    fn test_openai_preserves_previous_tool_result_frames() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let mut conversation = Conversation::new(None);
        conversation.add_user_message("clone and inspect".to_string());
        conversation.add_assistant_with_tools(
            String::new(),
            vec![MessageToolCall {
                id: "call-clone".to_string(),
                name: "shell".to_string(),
                arguments: r#"{"command":"git clone https://github.com/AliusTech/alius.git"}"#
                    .to_string(),
            }],
        );
        conversation.add_tool_result(
            "call-clone".to_string(),
            "shell".to_string(),
            "[exit:0]\nCloning into 'alius'".to_string(),
        );
        conversation.add_assistant_with_tools(
            String::new(),
            vec![MessageToolCall {
                id: "call-list".to_string(),
                name: "list_dir".to_string(),
                arguments: r#"{"path":"alius"}"#.to_string(),
            }],
        );
        let assistant_tool_calls = vec![ToolCall::new(
            "call-list".to_string(),
            "list_dir".to_string(),
            serde_json::json!({"path": "alius"}),
        )];
        let tool_results = vec![(
            "call-list".to_string(),
            "list_dir".to_string(),
            "dir .git\nfile README.md".to_string(),
        )];

        let messages = provider
            .build_continue_messages(&conversation, &tool_results, &assistant_tool_calls)
            .unwrap();
        let values = messages
            .iter()
            .map(|message| serde_json::to_value(message).unwrap())
            .collect::<Vec<_>>();

        assert_eq!(values.len(), 5);
        assert_eq!(values[0]["role"], "user");
        assert_eq!(values[1]["role"], "assistant");
        assert_eq!(values[1]["tool_calls"][0]["id"], "call-clone");
        assert_eq!(values[2]["role"], "tool");
        assert_eq!(values[2]["tool_call_id"], "call-clone");
        assert_eq!(values[3]["role"], "assistant");
        assert_eq!(values[3]["tool_calls"][0]["id"], "call-list");
        assert_eq!(values[4]["role"], "tool");
        assert_eq!(values[4]["tool_call_id"], "call-list");
    }

    #[test]
    fn test_openai_rejects_missing_tool_result_before_request() {
        let settings = test_settings();
        let provider = OpenAiProvider::new(&settings).unwrap();
        let conversation = Conversation::new(None);
        let assistant_tool_calls = vec![ToolCall::new(
            "call-shell".to_string(),
            "shell".to_string(),
            serde_json::json!({"command": "git status"}),
        )];

        let err = provider
            .build_continue_messages(&conversation, &[], &assistant_tool_calls)
            .unwrap_err();

        assert!(err
            .to_string()
            .contains("missing tool result for tool_call_id 'call-shell'"));
    }
}
