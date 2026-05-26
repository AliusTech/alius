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
        (stream, String::new()),
        |(mut stream, mut full_response)| async move {
            use futures::StreamExt;
            match stream.next().await {
                Some(result) => match result {
                    Ok(response) => {
                        let delta = response.choices
                            .first()
                            .and_then(|c| c.delta.content.clone());

                        if let Some(text) = delta {
                            full_response.push_str(&text);
                            Some((Ok(ChatEvent::Delta { text }), (stream, full_response)))
                        } else {
                            // Check if finished
                            let finish_reason = response.choices
                                .first()
                                .and_then(|c| c.finish_reason);

                            if finish_reason.is_some() {
                                let response_clone = full_response.clone();
                                Some((Ok(ChatEvent::Done { full_response: response_clone }), (stream, full_response)))
                            } else {
                                // No content, no finish - skip
                                Some((Ok(ChatEvent::Delta { text: "".to_string() }), (stream, full_response)))
                            }
                        }
                    }
                    Err(e) => {
                        Some((Err(anyhow::anyhow!("Stream error: {}", e)), (stream, full_response)))
                    }
                },
                None => {
                    // Stream ended without finish reason
                    let response_clone = full_response.clone();
                    Some((Ok(ChatEvent::Done { full_response: response_clone }), (stream, full_response)))
                }
            }
        }
    )
}