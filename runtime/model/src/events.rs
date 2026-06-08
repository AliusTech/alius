//! Chat events for streaming

use anyhow::Result;
use async_openai::types::ChatCompletionResponseStream;
use futures::Stream;

/// Event from streaming chat
#[derive(Debug, Clone)]
pub enum ChatEvent {
    /// Text delta received
    Delta { text: String },
    /// Stream completed
    Done { full_response: String },
    /// Error occurred
    Error { message: String },
}

/// Process a streaming response into events
pub fn process_stream(
    stream: ChatCompletionResponseStream,
) -> impl Stream<Item = Result<ChatEvent>> {
    futures::stream::unfold(
        (stream, String::new(), false),
        |(mut stream, mut full_response, done_emitted)| async move {
            use futures::StreamExt;

            if done_emitted {
                return None;
            }

            match stream.next().await {
                Some(result) => match result {
                    Ok(response) => {
                        let delta = response
                            .choices
                            .first()
                            .and_then(|c| c.delta.content.clone());

                        if let Some(text) = delta {
                            full_response.push_str(&text);
                            Some((
                                Ok(ChatEvent::Delta { text }),
                                (stream, full_response, false),
                            ))
                        } else {
                            // Check if finished
                            let finish_reason =
                                response.choices.first().and_then(|c| c.finish_reason);

                            if finish_reason.is_some() {
                                let response_clone = full_response.clone();
                                Some((
                                    Ok(ChatEvent::Done {
                                        full_response: response_clone,
                                    }),
                                    (stream, full_response, true),
                                ))
                            } else {
                                // No content, no finish - skip
                                Some((
                                    Ok(ChatEvent::Delta {
                                        text: "".to_string(),
                                    }),
                                    (stream, full_response, false),
                                ))
                            }
                        }
                    }
                    Err(e) => Some((
                        Err(anyhow::anyhow!("Stream error: {}", e)),
                        (stream, full_response, true),
                    )),
                },
                None => {
                    // Stream ended without finish reason
                    let response_clone = full_response.clone();
                    Some((
                        Ok(ChatEvent::Done {
                            full_response: response_clone,
                        }),
                        (stream, full_response, true),
                    ))
                }
            }
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_openai::error::OpenAIError;
    use async_openai::types::{
        ChatChoiceStream, ChatCompletionResponseStream, ChatCompletionStreamResponseDelta,
        CreateChatCompletionStreamResponse, FinishReason,
    };
    use futures::StreamExt;

    #[allow(deprecated)]
    fn stream_response(
        content: Option<&str>,
        finish_reason: Option<FinishReason>,
    ) -> CreateChatCompletionStreamResponse {
        CreateChatCompletionStreamResponse {
            id: "chatcmpl-test".to_string(),
            choices: vec![ChatChoiceStream {
                index: 0,
                delta: ChatCompletionStreamResponseDelta {
                    content: content.map(str::to_string),
                    function_call: None,
                    tool_calls: None,
                    role: None,
                },
                finish_reason,
                logprobs: None,
            }],
            created: 0,
            model: "test-model".to_string(),
            system_fingerprint: None,
            object: "chat.completion.chunk".to_string(),
        }
    }

    #[tokio::test]
    async fn process_stream_emits_done_once_when_underlying_stream_ends() {
        let chunks: Vec<std::result::Result<CreateChatCompletionStreamResponse, OpenAIError>> =
            vec![Ok(stream_response(Some("hi"), None))];
        let stream: ChatCompletionResponseStream = Box::pin(futures::stream::iter(chunks));

        let events: Vec<_> = process_stream(stream).take(3).collect().await;
        let done_count = events
            .iter()
            .filter(|event| matches!(event, Ok(ChatEvent::Done { .. })))
            .count();

        assert_eq!(done_count, 1);
        assert_eq!(events.len(), 2);
    }
}
