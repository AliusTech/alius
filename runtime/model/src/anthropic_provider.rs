//! Anthropic native provider implementation.
//!
//! Implements the Anthropic Messages API (`/v1/messages`) with SSE streaming
//! and tool calling support. Uses `reqwest` for HTTP communication.

use std::collections::{HashMap, HashSet};

use anyhow::Result;
use futures::Stream;
use serde_json::Value as JsonValue;

use protocol_interface::{MessageRole, ToolDef};
use runtime_config::LlmSettings;

use crate::endpoint::normalize_anthropic_api_base;
use crate::provider::{ChatStream, LlmProvider};
use crate::{ChatEvent, Conversation, ToolCall};

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic builtin model list (fallback when /v1/models fails or is unsupported).
pub const ANTHROPIC_DEFAULT_MODELS: &[&str] = &[
    "claude-opus-4-20250514",
    "claude-sonnet-4-20250514",
    "claude-haiku-4-5-20251001",
];

/// Token Plan builtin model list.
pub const TOKEN_PLAN_DEFAULT_MODELS: &[&str] = &["mimo-v2.5", "mimo-v2.5-pro", "mimo-v2-omni"];

/// Authentication style for Anthropic-compatible APIs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthStyle {
    /// Standard Anthropic: `x-api-key` header.
    ApiKey,
    /// OpenAI-style Anthropic proxy: `Authorization: Bearer`.
    Bearer,
}

/// Anthropic native LLM provider.
///
/// Serves as the base for all Anthropic-protocol providers. Vendor-specific
/// behavior (auth style, default models) is configured via constructor variants.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
    auth_style: AuthStyle,
    default_models: Vec<String>,
}

impl AnthropicProvider {
    /// Create a standard Anthropic provider.
    pub fn new(config: &LlmSettings) -> Result<Self> {
        Self::build(config, AuthStyle::ApiKey, ANTHROPIC_DEFAULT_MODELS)
    }

    /// Create a Token Plan proxy provider (Anthropic headers, Xiaomi models).
    pub fn new_token_plan(config: &LlmSettings) -> Result<Self> {
        Self::build(config, AuthStyle::ApiKey, TOKEN_PLAN_DEFAULT_MODELS)
    }

    /// Internal constructor shared by all variants.
    fn build(config: &LlmSettings, auth_style: AuthStyle, default_models: &[&str]) -> Result<Self> {
        let api_key = config.get_api_key().ok_or_else(|| {
            anyhow::anyhow!("API key not found. Set it in config or environment variable.")
        })?;
        let client = reqwest::Client::builder()
            .user_agent(crate::http::user_agent())
            .build()?;

        Ok(Self {
            client,
            api_key,
            base_url: normalize_anthropic_api_base(&config.get_base_url()),
            model: config.model.clone(),
            auth_style,
            default_models: default_models.iter().map(|s| s.to_string()).collect(),
        })
    }

    /// Apply authentication header to a request builder.
    fn apply_auth(&self, builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.auth_style {
            AuthStyle::Bearer => builder.bearer_auth(&self.api_key),
            AuthStyle::ApiKey => builder.header("x-api-key", &self.api_key),
        }
    }

    /// Build Anthropic messages array from conversation.
    ///
    /// Anthropic uses a top-level `system` field rather than a system message.
    /// Returns (system_prompt, messages).
    fn build_messages(&self, conversation: &Conversation) -> (Option<String>, Vec<JsonValue>) {
        let system = conversation.system_prompt().map(|s| s.to_string());
        let mut messages = Vec::new();

        let history = conversation.messages();
        let mut index = 0;
        while index < history.len() {
            let msg = &history[index];
            match msg.role {
                MessageRole::System | MessageRole::Summary => {
                    // Anthropic doesn't have system messages in the messages array.
                    // System/summary content is prepended to the first user message
                    // or sent as top-level system field.
                    // We'll handle this by merging into the system prompt.
                }
                MessageRole::User => {
                    messages.push(serde_json::json!({
                        "role": "user",
                        "content": msg.content,
                    }));
                }
                MessageRole::Assistant => {
                    if let Some(calls) = &msg.tool_calls {
                        // Assistant turn that requested tools: content is a list
                        // of blocks — optional text + tool_use blocks.
                        let mut blocks: Vec<JsonValue> = Vec::new();
                        if !msg.content.is_empty() {
                            blocks.push(serde_json::json!({
                                "type": "text",
                                "text": msg.content,
                            }));
                        }
                        for c in calls {
                            blocks.push(serde_json::json!({
                                "type": "tool_use",
                                "id": c.id,
                                "name": c.name,
                                "input": serde_json::from_str::<JsonValue>(&c.arguments)
                                    .unwrap_or(serde_json::Value::Object(Default::default())),
                            }));
                        }
                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": blocks,
                        }));
                    } else {
                        messages.push(serde_json::json!({
                            "role": "assistant",
                            "content": msg.content,
                        }));
                    }
                }
                MessageRole::Tool => {
                    let mut blocks: Vec<JsonValue> = Vec::new();
                    while index < history.len() && history[index].role == MessageRole::Tool {
                        if let Some(tool_call_id) = history[index].tool_call_id.as_deref() {
                            blocks.push(serde_json::json!({
                                "type": "tool_result",
                                "tool_use_id": tool_call_id,
                                "content": history[index].content.as_str(),
                            }));
                        }
                        index += 1;
                    }
                    if !blocks.is_empty() {
                        messages.push(serde_json::json!({
                            "role": "user",
                            "content": blocks,
                        }));
                    }
                    continue;
                }
            }
            index += 1;
        }

        // Merge any system/summary messages that were skipped into the system prompt
        let system_parts: Vec<String> = conversation
            .messages()
            .iter()
            .filter(|m| matches!(m.role, MessageRole::System | MessageRole::Summary))
            .map(|m| m.content.clone())
            .collect();

        let merged_system = if system_parts.is_empty() {
            system
        } else {
            let mut parts = system_parts;
            if let Some(s) = system {
                parts.insert(0, s);
            }
            Some(parts.join("\n\n"))
        };

        (merged_system, messages)
    }

    /// Build Anthropic tool definitions from ToolDef list.
    fn build_tools(&self, tools: &[ToolDef]) -> Vec<JsonValue> {
        tools
            .iter()
            .map(|t| {
                serde_json::json!({
                    "name": t.name,
                    "description": t.description,
                    "input_schema": t.parameters,
                })
            })
            .collect()
    }

    /// Send a non-streaming request to the Anthropic API.
    async fn send_request(&self, body: JsonValue) -> Result<JsonValue> {
        let mut attempts = 0;
        let max_retries = 3;
        let mut delay = std::time::Duration::from_secs(1);

        loop {
            let url = format!("{}/v1/messages", self.base_url);
            let resp = self
                .apply_auth(
                    self.client
                        .post(&url)
                        .header("anthropic-version", ANTHROPIC_VERSION)
                        .header("content-type", "application/json")
                        .json(&body),
                )
                .send()
                .await;

            match resp {
                Ok(resp) if resp.status().is_success() => {
                    let text = resp.text().await?;
                    return serde_json::from_str(&text)
                        .map_err(|e| anyhow::anyhow!("Failed to parse Anthropic response: {}", e));
                }
                Ok(resp) if resp.status().as_u16() == 429 => {
                    attempts += 1;
                    if attempts >= max_retries {
                        let text = resp.text().await?;
                        return Err(anyhow::anyhow!(
                            "Rate limited (429) after {} retries. Please wait before trying again.\n{}",
                            max_retries,
                            text
                        ));
                    }
                    tracing::warn!(
                        "Rate limited (429), retrying after {} seconds (attempt {}/{})",
                        delay.as_secs(),
                        attempts,
                        max_retries
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                    continue;
                }
                Ok(resp) => {
                    attempts += 1;
                    let status = resp.status();
                    let text = resp.text().await?;
                    if attempts >= max_retries {
                        return Err(anyhow::anyhow!(
                            "Anthropic API error ({}): {}",
                            status,
                            text
                        ));
                    }
                    tracing::warn!("Anthropic API error ({}), retrying: {}", status, text);
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(anyhow::anyhow!("Anthropic API error: {}", e));
                    }
                    tracing::warn!("Anthropic API error, retrying: {}", e);
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                }
            }
        }
    }

    /// Parse tool_use blocks from an Anthropic response.
    fn extract_tool_calls(response: &JsonValue) -> Option<Vec<ToolCall>> {
        let content = response.get("content")?.as_array()?;
        let tool_calls: Vec<ToolCall> = content
            .iter()
            .filter(|block| block["type"] == "tool_use")
            .map(|block| {
                ToolCall::new(
                    block["id"].as_str().unwrap_or_default().to_string(),
                    block["name"].as_str().unwrap_or_default().to_string(),
                    block["input"].clone(),
                )
            })
            .collect();

        if tool_calls.is_empty() {
            None
        } else {
            Some(tool_calls)
        }
    }

    /// Execute a tool-calling request (non-streaming).
    async fn do_tool_request(
        &self,
        system: Option<String>,
        messages: Vec<JsonValue>,
        tools: &[ToolDef],
    ) -> Result<(ChatStream, Option<Vec<ToolCall>>)> {
        let mut body = serde_json::json!({
            "model": self.model,
            "max_tokens": 8192,
            "messages": messages,
        });

        if let Some(sys) = &system {
            body["system"] = serde_json::json!(sys);
        }

        if !tools.is_empty() {
            body["tools"] = serde_json::json!(self.build_tools(tools));
        }

        let response = self.send_request(body).await?;

        let tool_calls = Self::extract_tool_calls(&response);

        // Extract text content
        let text = response
            .get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

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

    /// Create a streaming request and return a ChatStream.
    async fn do_stream_request(&self, body: JsonValue) -> Result<ChatStream> {
        let mut attempts = 0;
        let max_retries = 3;
        let mut delay = std::time::Duration::from_secs(1);

        loop {
            let url = format!("{}/v1/messages", self.base_url);
            let resp = self
                .apply_auth(
                    self.client
                        .post(&url)
                        .header("anthropic-version", ANTHROPIC_VERSION)
                        .header("content-type", "application/json")
                        .json(&body),
                )
                .send()
                .await;

            match resp {
                Ok(resp) if resp.status().is_success() => {
                    let byte_stream = resp.bytes_stream();
                    let stream = Self::parse_sse_stream(byte_stream);
                    return Ok(Box::pin(stream));
                }
                Ok(resp) if resp.status().as_u16() == 429 => {
                    attempts += 1;
                    if attempts >= max_retries {
                        let text = resp.text().await?;
                        return Err(anyhow::anyhow!(
                            "Rate limited (429) after {} retries. Please wait before trying again.\n{}",
                            max_retries,
                            text
                        ));
                    }
                    tracing::warn!(
                        "Rate limited (429), retrying after {} seconds (attempt {}/{})",
                        delay.as_secs(),
                        attempts,
                        max_retries
                    );
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                    continue;
                }
                Ok(resp) => {
                    let status = resp.status();
                    let text = resp.text().await?;
                    return Err(anyhow::anyhow!(
                        "Anthropic API error ({}): {}",
                        status,
                        text
                    ));
                }
                Err(e) => {
                    attempts += 1;
                    if attempts >= max_retries {
                        return Err(anyhow::anyhow!("Anthropic API error: {}", e));
                    }
                    tracing::warn!("Anthropic API error, retrying: {}", e);
                    tokio::time::sleep(delay).await;
                    delay = std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                }
            }
        }
    }

    /// Parse Anthropic SSE events into ChatEvent stream.
    ///
    /// Anthropic SSE format:
    ///   event: content_block_delta
    ///   data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"..."}}
    fn parse_sse_stream(
        byte_stream: impl Stream<Item = Result<bytes::Bytes, reqwest::Error>> + Send + Unpin + 'static,
    ) -> impl Stream<Item = Result<ChatEvent>> {
        use futures::StreamExt;

        futures::stream::unfold(
            (byte_stream, String::new(), String::new(), false),
            |(mut stream, mut buffer, mut full_response, mut done)| async move {
                if done {
                    return None;
                }

                loop {
                    // Try to parse a complete SSE event from the buffer
                    if let Some(event) = parse_next_sse_event(&mut buffer) {
                        match event.event_type.as_str() {
                            "content_block_delta" => {
                                if let Some(text) = event.data["delta"]["text"].as_str() {
                                    full_response.push_str(text);
                                    return Some((
                                        Ok(ChatEvent::Delta {
                                            text: text.to_string(),
                                        }),
                                        (stream, buffer, full_response, done),
                                    ));
                                }
                            }
                            "message_stop" => {
                                done = true;
                                return Some((
                                    Ok(ChatEvent::Done {
                                        full_response: full_response.clone(),
                                    }),
                                    (stream, buffer, full_response, done),
                                ));
                            }
                            // message_delta may contain stop_reason
                            "message_delta" if event.data["delta"]["stop_reason"].is_string() => {
                                done = true;
                                return Some((
                                    Ok(ChatEvent::Done {
                                        full_response: full_response.clone(),
                                    }),
                                    (stream, buffer, full_response, done),
                                ));
                            }
                            // message_start, content_block_start, ping — skip
                            _ => {}
                        }
                        continue;
                    }

                    // Need more data
                    match stream.next().await {
                        Some(Ok(bytes)) => {
                            buffer.push_str(&String::from_utf8_lossy(&bytes));
                        }
                        Some(Err(e)) => {
                            return Some((
                                Err(anyhow::anyhow!("SSE stream error: {}", e)),
                                (stream, buffer, full_response, done),
                            ));
                        }
                        None => {
                            // Stream ended
                            if !done {
                                done = true;
                                return Some((
                                    Ok(ChatEvent::Done {
                                        full_response: full_response.clone(),
                                    }),
                                    (stream, buffer, full_response, done),
                                ));
                            }
                            return None;
                        }
                    }
                }
            },
        )
    }
}

/// A parsed SSE event with type and data.
struct SseEvent {
    event_type: String,
    data: JsonValue,
}

/// Try to parse the next complete SSE event from the buffer.
/// Returns None if the buffer doesn't contain a complete event yet.
/// Consumes the parsed bytes from the buffer.
fn parse_next_sse_event(buffer: &mut String) -> Option<SseEvent> {
    // SSE events are separated by double newlines
    let end = buffer.find("\n\n")?;
    let block = buffer[..end].to_string();
    *buffer = buffer[end + 2..].to_string();

    let mut event_type = String::new();
    let mut data_str = String::new();

    for line in block.lines() {
        if let Some(rest) = line.strip_prefix("event: ") {
            event_type = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("data: ") {
            data_str = rest.trim().to_string();
        }
    }

    if event_type.is_empty() || data_str.is_empty() {
        return None;
    }

    let data: JsonValue = serde_json::from_str(&data_str).ok()?;
    Some(SseEvent { event_type, data })
}

impl LlmProvider for AnthropicProvider {
    fn chat_stream<'a>(
        &'a self,
        conversation: &'a Conversation,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<ChatStream>> + Send + 'a>> {
        Box::pin(async move {
            let (system, messages) = self.build_messages(conversation);

            let mut body = serde_json::json!({
                "model": self.model,
                "max_tokens": 8192,
                "messages": messages,
                "stream": true,
            });

            if let Some(sys) = system {
                body["system"] = serde_json::json!(sys);
            }

            self.do_stream_request(body).await
        })
    }

    fn chat_once<'a>(
        &'a self,
        prompt: &'a str,
        system: Option<&'a str>,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<String>> + Send + 'a>> {
        Box::pin(async move {
            let messages = vec![serde_json::json!({
                "role": "user",
                "content": prompt,
            })];

            let mut body = serde_json::json!({
                "model": self.model,
                "max_tokens": 8192,
                "messages": messages,
            });

            if let Some(sys) = system {
                body["system"] = serde_json::json!(sys);
            }

            let response = self.send_request(body).await?;

            let text = response
                .get("content")
                .and_then(|c| c.as_array())
                .map(|blocks| {
                    blocks
                        .iter()
                        .filter(|b| b["type"] == "text")
                        .filter_map(|b| b["text"].as_str())
                        .collect::<Vec<_>>()
                        .join("")
                })
                .unwrap_or_default();

            Ok(text)
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
                let url = format!("{}/v1/models", self.base_url);
                let resp = self
                    .apply_auth(
                        self.client
                            .get(&url)
                            .header("anthropic-version", ANTHROPIC_VERSION)
                            .header("content-type", "application/json"),
                    )
                    .send()
                    .await;

                match resp {
                    Ok(resp) if resp.status().is_success() => {
                        let body: JsonValue = resp.json().await?;
                        let models: Vec<String> = body
                            .get("data")
                            .and_then(|d| d.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|m| {
                                        m.get("id").and_then(|id| id.as_str()).map(String::from)
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        if models.is_empty() {
                            tracing::info!("No models returned, using default model list");
                            return Ok(self.default_models.clone());
                        }
                        return Ok(models);
                    }
                    Ok(resp) if resp.status().as_u16() == 429 => {
                        attempts += 1;
                        if attempts >= max_retries {
                            tracing::warn!("Rate limited, using default model list");
                            return Ok(self.default_models.clone());
                        }
                        tracing::warn!(
                            "Rate limited (429), retrying after {} seconds (attempt {}/{})",
                            delay.as_secs(),
                            attempts,
                            max_retries
                        );
                        tokio::time::sleep(delay).await;
                        delay =
                            std::cmp::min(delay.mul_f32(2.0), std::time::Duration::from_secs(60));
                        continue;
                    }
                    Ok(resp) => {
                        tracing::warn!(
                            "Anthropic list models error ({}), using default model list",
                            resp.status()
                        );
                        return Ok(self.default_models.clone());
                    }
                    Err(e) => {
                        attempts += 1;
                        if attempts >= max_retries {
                            tracing::warn!("Anthropic API error: {}, using default model list", e);
                            return Ok(self.default_models.clone());
                        }
                        tracing::warn!("Anthropic API error, retrying: {}", e);
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
            let (system, messages) = self.build_messages(conversation);
            self.do_tool_request(system, messages, tools).await
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
            let (system, messages) =
                self.build_continue_messages(conversation, tool_results, assistant_tool_calls)?;
            self.do_tool_request(system, messages, tools).await
        })
    }
}

impl AnthropicProvider {
    fn build_continue_messages(
        &self,
        conversation: &Conversation,
        tool_results: &[(String, String, String)],
        assistant_tool_calls: &[ToolCall],
    ) -> Result<(Option<String>, Vec<JsonValue>)> {
        let normalized_tool_results =
            normalize_provider_tool_results(assistant_tool_calls, tool_results)?;
        let (system, mut messages) = self.build_messages(conversation);

        if !conversation_ends_with_tool_calls(conversation, assistant_tool_calls) {
            messages.push(assistant_tool_use_message(assistant_tool_calls)?);
        }

        let tool_result_blocks: Vec<JsonValue> = normalized_tool_results
            .iter()
            .map(|(call_id, _name, result)| {
                serde_json::json!({
                    "type": "tool_result",
                    "tool_use_id": call_id,
                    "content": result,
                })
            })
            .collect();

        messages.push(serde_json::json!({
            "role": "user",
            "content": tool_result_blocks,
        }));

        Ok((system, messages))
    }
}

fn assistant_tool_use_message(assistant_tool_calls: &[ToolCall]) -> Result<JsonValue> {
    validate_assistant_tool_calls(assistant_tool_calls)?;
    let blocks = assistant_tool_calls
        .iter()
        .map(|call| {
            serde_json::json!({
                "type": "tool_use",
                "id": call.id,
                "name": call.name,
                "input": call.args.clone(),
            })
        })
        .collect::<Vec<_>>();

    Ok(serde_json::json!({
        "role": "assistant",
        "content": blocks,
    }))
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
            provider: ProviderType::Anthropic,
            provider_mode: None,
            model: "claude-3-5-sonnet-20241022".to_string(),
            api_key: Some("test-key".to_string()),
            api_key_env: None,
            base_url: None,
            review_model: None,
        }
    }

    #[test]
    fn test_anthropic_rebuilds_missing_assistant_tool_use_frame() {
        let settings = test_settings();
        let provider = AnthropicProvider::new(&settings).unwrap();
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

        let (_system, messages) = provider
            .build_continue_messages(&conversation, &tool_results, &assistant_tool_calls)
            .unwrap();

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"][0]["type"], "tool_use");
        assert_eq!(messages[1]["content"][0]["id"], "call-shell");
        assert_eq!(messages[2]["role"], "user");
        assert_eq!(messages[2]["content"][0]["type"], "tool_result");
        assert_eq!(messages[2]["content"][0]["tool_use_id"], "call-shell");
    }

    #[test]
    fn test_anthropic_preserves_previous_tool_result_frames() {
        let settings = test_settings();
        let provider = AnthropicProvider::new(&settings).unwrap();
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

        let (_system, messages) = provider
            .build_continue_messages(&conversation, &tool_results, &assistant_tool_calls)
            .unwrap();

        assert_eq!(messages.len(), 5);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[1]["role"], "assistant");
        assert_eq!(messages[1]["content"][0]["id"], "call-clone");
        assert_eq!(messages[2]["role"], "user");
        assert_eq!(messages[2]["content"][0]["type"], "tool_result");
        assert_eq!(messages[2]["content"][0]["tool_use_id"], "call-clone");
        assert_eq!(messages[3]["role"], "assistant");
        assert_eq!(messages[3]["content"][0]["id"], "call-list");
        assert_eq!(messages[4]["role"], "user");
        assert_eq!(messages[4]["content"][0]["tool_use_id"], "call-list");
    }

    #[test]
    fn test_anthropic_rejects_missing_tool_result_before_request() {
        let settings = test_settings();
        let provider = AnthropicProvider::new(&settings).unwrap();
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

    #[tokio::test]
    async fn test_anthropic_provider_new() {
        let settings = test_settings();
        let provider = AnthropicProvider::new(&settings);
        assert!(provider.is_ok());
    }

    #[tokio::test]
    async fn test_anthropic_provider_new_missing_key() {
        let settings = LlmSettings {
            provider: ProviderType::Anthropic,
            provider_mode: None,
            model: "claude-3-5-sonnet".to_string(),
            api_key: None,
            api_key_env: None,
            base_url: None,
            review_model: None,
        };
        let provider = AnthropicProvider::new(&settings);
        assert!(provider.is_err());
    }

    #[tokio::test]
    async fn test_anthropic_list_models_connection_error() {
        let settings = LlmSettings {
            provider: ProviderType::Anthropic,
            provider_mode: None,
            model: "test".to_string(),
            api_key: Some("key".to_string()),
            api_key_env: None,
            base_url: Some("http://127.0.0.1:1".to_string()),
            review_model: None,
        };
        let provider = AnthropicProvider::new(&settings).unwrap();
        let result = provider.list_models().await;
        // On connection error, falls back to default models
        assert!(result.is_ok());
        assert!(!result.unwrap().is_empty());
    }

    #[test]
    fn test_parse_sse_event_complete() {
        let mut buffer = "event: content_block_delta\ndata: {\"type\":\"content_block_delta\",\"delta\":{\"text\":\"hello\"}}\n\n".to_string();
        let event = parse_next_sse_event(&mut buffer);
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_type, "content_block_delta");
        assert_eq!(event.data["delta"]["text"], "hello");
    }

    #[test]
    fn test_parse_sse_event_incomplete() {
        let mut buffer = "event: content_block_delta\n".to_string();
        let event = parse_next_sse_event(&mut buffer);
        assert!(event.is_none());
    }

    #[test]
    fn test_parse_sse_event_message_stop() {
        let mut buffer = "event: message_stop\ndata: {\"type\":\"message_stop\"}\n\n".to_string();
        let event = parse_next_sse_event(&mut buffer);
        assert!(event.is_some());
        let event = event.unwrap();
        assert_eq!(event.event_type, "message_stop");
    }
}
