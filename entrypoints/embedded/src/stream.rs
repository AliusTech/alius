//! Streaming handler for embedded SDK
//!
//! Manages streaming responses from LLM with callbacks

use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use protocol_interface::core::{CoreEventKind, CoreEventPayload, RunRef, TraceId};
use protocol_interface::ProtocolInterface;

/// Stream callback function type
#[allow(dead_code)]
pub type StreamCallback = extern "C" fn(delta: *const i8, user_data: *mut std::ffi::c_void);

/// Error callback function type
#[allow(dead_code)]
pub type ErrorCallback =
    extern "C" fn(code: i32, message: *const i8, user_data: *mut std::ffi::c_void);

/// Handle for an active streaming session
pub struct StreamHandle {
    run_ref: RunRef,
    #[allow(dead_code)]
    trace_id: TraceId,
    #[allow(dead_code)]
    stream_callback: StreamCallback,
    #[allow(dead_code)]
    error_callback: ErrorCallback,
    #[allow(dead_code)]
    user_data: *mut std::ffi::c_void,
    active: Arc<AtomicBool>,
}

// Safety: user_data is only passed back to user callbacks
// and active is Arc<AtomicBool> which is Send+Sync
unsafe impl Send for StreamHandle {}

impl StreamHandle {
    /// Create a new stream handle and start streaming
    pub fn new(
        protocol: Arc<ProtocolInterface<core_runtime::CoreRuntime>>,
        run_ref: RunRef,
        trace_id: TraceId,
        stream_callback: StreamCallback,
        error_callback: ErrorCallback,
        user_data: *mut std::ffi::c_void,
    ) -> Self {
        let handle = Self {
            run_ref: run_ref.clone(),
            trace_id: trace_id.clone(),
            stream_callback,
            error_callback,
            user_data,
            active: Arc::new(AtomicBool::new(true)),
        };

        // Start streaming in a background thread
        let active_clone = handle.active.clone();
        let trace_id_clone = trace_id.clone();
        std::thread::spawn(move || {
            Self::stream_events(protocol, run_ref, trace_id_clone, active_clone);
        });

        handle
    }

    /// Check if this handle matches a run reference
    pub fn matches_run(&self, run_ref: &RunRef) -> bool {
        &self.run_ref == run_ref
    }

    /// Get the run reference
    pub fn run_ref(&self) -> &RunRef {
        &self.run_ref
    }

    /// Stop streaming
    pub fn stop(&self) {
        self.active.store(false, Ordering::Relaxed);
    }

    /// Stream events from the protocol
    fn stream_events(
        protocol: Arc<ProtocolInterface<core_runtime::CoreRuntime>>,
        run_ref: RunRef,
        #[allow(dead_code)] _trace_id: TraceId,
        active: Arc<AtomicBool>,
    ) {
        // Poll for events
        let mut sequence = 0u64;
        let mut last_event_count = 0u64;

        loop {
            // Check if we should stop
            if !active.load(Ordering::Relaxed) {
                break;
            }

            // Try to get events
            match protocol.subscribe(&run_ref) {
                Ok(events) => {
                    // Only process new events
                    let events_len = events.len();
                    if events_len > last_event_count as usize {
                        for envelope in events.iter().skip(last_event_count as usize) {
                            if !active.load(Ordering::Relaxed) {
                                break;
                            }

                            Self::handle_event(&envelope.payload);
                            sequence = sequence.max(envelope.payload.sequence + 1);
                        }
                        last_event_count = events_len as u64;
                    }

                    // Check if we're done
                    if let Some(last_envelope) = events.last() {
                        if matches!(last_envelope.payload.kind, CoreEventKind::FinalResult) {
                            break;
                        }
                        if matches!(last_envelope.payload.kind, CoreEventKind::ErrorRaised) {
                            break;
                        }
                    }

                    // Small delay before next poll
                    std::thread::sleep(std::time::Duration::from_millis(50));
                }
                Err(e) => {
                    // Error getting events - report and stop
                    let _msg = CString::new(format!("Stream error: {}", e)).unwrap();
                    // We can't call error_callback here because we don't have those captures
                    eprintln!("Stream error: {}", e);
                    break;
                }
            }
        }
    }

    /// Handle a single event
    fn handle_event(event: &protocol_interface::core::CoreEvent) {
        match event.kind {
            CoreEventKind::ModelDelta => {
                if let CoreEventPayload::Text { text } = &event.payload {
                    let _c_str = CString::new(text.as_str()).unwrap();
                    // Call the stream callback
                    // Note: This needs access to the callback captures
                    // For now, we'll need to restructure to make this work
                    eprintln!("Delta: {}", text);
                }
            }
            CoreEventKind::FinalResult => {
                if let CoreEventPayload::Final { content, .. } = &event.payload {
                    eprintln!("Final: {}", content);
                }
            }
            CoreEventKind::ErrorRaised => {
                if let CoreEventPayload::Error { message, .. } = &event.payload {
                    eprintln!("Error: {}", message);
                }
            }
            _ => {
                // Ignore other events for now
            }
        }
    }
}

// For now, we need to restructure to properly handle callbacks
// The current design needs to pass callback information through to the streaming thread
