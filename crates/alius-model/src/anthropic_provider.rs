//! Anthropic native provider implementation.
//!
//! Implements the Anthropic Messages API (`/v1/messages`) with SSE streaming
//! and tool calling support. Uses `reqwest` for HTTP communication.

use anyhow::Result;
use futures::Stream;
use serde_json::Value as JsonValue;

use alius_config::LlmSettings;
use alius_protocol::{MessageRole, ToolDef};

use crate::{ChatEvent, Conversation, ToolCall};
use crate::provider::{LlmProvider, ChatStream};

const ANTHROPIC_VERSION: &str = "2023-06-01";

/// Anthropic native LLM provider.
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,
    base_url: String,
    model: String,
}

impl AnthropicProvider {
    /// Create a new Anthropic provider from LLM settings.
    pub fn new(config: &LlmSettings) -> Result<Self> {
        let api_key = config.get_api_key().ok_or_else(|| {
            anyhow::anyhow!("API key not found. Set it in config or environment variable.")
        })?;

        Ok(Self {
            client: reqwest::Client::new(),
            api_key,
            base_url: config.get_base_url(),
            model: config.model.clone(),
        })
    }

    /// Build Anthropic messages array from conversation.
    ///
    /// Anthropic uses a top-level `system` field rather than a system message.
    /// Returns (system_prompt, messages).
    fn build_messages(&self, conversation: &Conversation) -> (Option<String>, Vec<JsonValue>) {
        let system = conversation.system_prompt().map(|s| s.to_string());
        let mut messages = Vec::new();

        for msg in conversation.messages() {
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
                    messages.push(serde_json::json!({
                        "role": "assistant",
                        "content": msg.content,
                    }));
                }
            }
        }

        // Merge any system/summary messages that were skipped into the system prompt
        let system_parts: Vec<String> = conversation.messages().iter()
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
        tools.iter().map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "input_schema": t.parameters,
            })
        }).collect()
    }

    /// Send a non-streaming request to the Anthropic API.
    async fn send_request(&self, body: JsonValue) -> Result<JsonValue> {
        let url = format!("{}/messages", self.base_url);
        let resp = self.client.post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let text = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow::anyhow!("Anthropic API error ({}): {}", status, text));
        }

        serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("Failed to parse Anthropic response: {}", e))
    }

    /// Parse tool_use blocks from an Anthropic response.
    fn extract_tool_calls(response: &JsonValue) -> Option<Vec<ToolCall>> {
        let content = response.get("content")?.as_array()?;
        let tool_calls: Vec<ToolCall> = content.iter()
            .filter(|block| block["type"] == "tool_use")
            .map(|block| {
                ToolCall::new(
                    block["id"].as_str().unwrap_or_default().to_string(),
                    block["name"].as_str().unwrap_or_default().to_string(),
                    block["input"].clone(),
                )
            })
            .collect();

        if tool_calls.is_empty() { None } else { Some(tool_calls) }
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
        let text = response.get("content")
            .and_then(|c| c.as_array())
            .map(|blocks| {
                blocks.iter()
                    .filter(|b| b["type"] == "text")
                    .filter_map(|b| b["text"].as_str())
                    .collect::<Vec<_>>()
                    .join("")
            })
            .unwrap_or_default();

        let events: Vec<Result<ChatEvent>> = if text.is_empty() && tool_calls.is_none() {
            vec![Ok(ChatEvent::Done { full_response: String::new() })]
        } else {
            vec![
                Ok(ChatEvent::Delta { text: text.clone() }),
                Ok(ChatEvent::Done { full_response: text }),
            ]
        };

        Ok((Box::pin(futures::stream::iter(events)), tool_calls))
    }

    /// Create a streaming request and return a ChatStream.
    async fn do_stream_request(
        &self,
        body: JsonValue,
    ) -> Result<ChatStream> {
        let url = format!("{}/messages", self.base_url);
        let resp = self.client.post(&url)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", ANTHROPIC_VERSION)
            .header("content-type", "application/json")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await?;
            return Err(anyhow::anyhow!("Anthropic API error ({}): {}", status, text));
        }

        let byte_stream = resp.bytes_stream();
        let stream = Self::parse_sse_stream(byte_stream);
        Ok(Box::pin(stream))
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
                                        Ok(ChatEvent::Delta { text: text.to_string() }),
                                        (stream, buffer, full_response, done),
                                    ));
                                }
                            }
                            "message_stop" => {
                                done = true;
                                return Some((
                                    Ok(ChatEvent::Done { full_response: full_response.clone() }),
                                    (stream, buffer, full_response, done),
                                ));
                            }
                            // message_delta may contain stop_reason
                            "message_delta" if event.data["delta"]["stop_reason"].is_string() => {
                                done = true;
                                return Some((
                                    Ok(ChatEvent::Done { full_response: full_response.clone() }),
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
                                    Ok(ChatEvent::Done { full_response: full_response.clone() }),
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

            let text = response.get("content")
                .and_then(|c| c.as_array())
                .map(|blocks| {
                    blocks.iter()
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
        // Anthropic doesn't support /v1/models — return empty list
        Box::pin(async { Ok(vec![]) })
    }

    fn chat_stream_with_tools<'a>(
        &'a self,
        conversation: &'a Conversation,
        tools: &'a [ToolDef],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>> + Send + 'a>> {
        Box::pin(async move {
            let (system, messages) = self.build_messages(conversation);
            self.do_tool_request(system, messages, tools).await
        })
    }

    fn continue_with_tool_results<'a>(
        &'a self,
        conversation: &'a Conversation,
        tool_results: &'a [(String, String, String)],
        tools: &'a [ToolDef],
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(ChatStream, Option<Vec<ToolCall>>)>> + Send + 'a>> {
        Box::pin(async move {
            let (system, mut messages) = self.build_messages(conversation);

            // Add tool results as a user message with tool_result content blocks
            let tool_result_blocks: Vec<JsonValue> = tool_results.iter()
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

            self.do_tool_request(system, messages, tools).await
        })
    }
}
