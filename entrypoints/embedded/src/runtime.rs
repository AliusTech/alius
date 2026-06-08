//! Runtime implementation for embedded SDK
//!
//! Provides the core FFI interface for C/C++ integration

use std::ffi::{CStr, CString};
use std::ptr;
use std::sync::{Arc, LazyLock, Mutex};

use crate::error::AliusError;
use crate::stream::{StreamCallback, StreamHandle};
use crate::types::AliusErrorCode;
use core_runtime::CoreRuntime;
use protocol_interface::core::{
    CapabilityScope, CoreRequest, Origin, ProtocolEnvelope, WorkspaceRef,
};
use protocol_interface::{ProtocolContext, ProtocolInterface};

/// Internal runtime state
struct RuntimeInner {
    protocol: Arc<ProtocolInterface<CoreRuntime>>,
    stream_handle: Option<StreamHandle>,
}

/// Global runtime instance - single session for embedded use
static GLOBAL_RUNTIME: LazyLock<Arc<Mutex<Option<EmbeddedRuntime>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

/// Main runtime instance
pub struct EmbeddedRuntime {
    inner: RuntimeInner,
}

impl EmbeddedRuntime {
    /// Create a new runtime instance
    pub fn new() -> Result<Self, AliusError> {
        let workspace = WorkspaceRef::new("/tmp/alius-embedded");
        let core = CoreRuntime::new(workspace);

        let protocol = Arc::new(ProtocolInterface::new(core));

        Ok(Self {
            inner: RuntimeInner {
                protocol,
                stream_handle: None,
            },
        })
    }

    /// Get protocol interface
    pub fn protocol(&self) -> &Arc<ProtocolInterface<CoreRuntime>> {
        &self.inner.protocol
    }

    /// Set stream handle
    pub fn set_stream_handle(&mut self, handle: Option<StreamHandle>) {
        self.inner.stream_handle = handle;
    }

    /// Get stream handle
    pub fn stream_handle(&self) -> Option<&StreamHandle> {
        self.inner.stream_handle.as_ref()
    }

    /// Cancel active stream
    pub fn cancel_stream(&mut self) {
        self.inner.stream_handle = None;
    }
}

pub mod ffi {
    use super::*;
    use std::ffi::c_void;

    /// Initialize the global Alius runtime
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    #[no_mangle]
    pub extern "C" fn alius_init() -> i32 {
        let mut global = GLOBAL_RUNTIME.lock().unwrap();
        if global.is_some() {
            return 0; // Already initialized
        }

        match EmbeddedRuntime::new() {
            Ok(runtime) => {
                *global = Some(runtime);
                0
            }
            Err(e) => {
                eprintln!("Alius init failed: {:?}", e);
                -1
            }
        }
    }

    /// Cleanup the global Alius runtime
    #[no_mangle]
    pub extern "C" fn alius_cleanup() {
        let mut global = GLOBAL_RUNTIME.lock().unwrap();
        *global = None;
    }

    /// Set model configuration
    ///
    /// # Safety
    /// `provider` and `model` must be valid null-terminated C strings
    /// `api_key` may be NULL
    ///
    /// # Returns
    /// AliusError with code=0 on success
    #[no_mangle]
    pub unsafe extern "C" fn alius_config_set_model(
        provider: *const i8,
        model: *const i8,
        api_key: *const i8,
    ) -> AliusError {
        let global = GLOBAL_RUNTIME.lock().unwrap();
        let runtime = match global.as_ref() {
            Some(r) => r,
            None => {
                return AliusError::new(AliusErrorCode::RuntimeError, "Runtime not initialized")
            }
        };

        let provider_str = unsafe { CStr::from_ptr(provider) }
            .to_str()
            .unwrap_or("anthropic");
        let model_str = unsafe { CStr::from_ptr(model) }
            .to_str()
            .unwrap_or("claude-haiku-4-20250218");
        let api_key_str = if api_key.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(api_key) }.to_str().unwrap_or(""))
        };

        let ctx = ProtocolContext {
            origin: Origin::EmbeddedSdk,
            capability_scope: CapabilityScope::embedded_sdk(),
            workspace_root: None,
        };

        // Update provider
        if let Err(e) =
            runtime
                .protocol()
                .config_update(&ctx, "llm.provider", serde_json::json!(provider_str))
        {
            return AliusError::from_err(e);
        }

        // Update model
        if let Err(e) =
            runtime
                .protocol()
                .config_update(&ctx, "llm.model", serde_json::json!(model_str))
        {
            return AliusError::from_err(e);
        }

        // Update API key if provided
        if let Some(key) = api_key_str {
            if let Err(e) =
                runtime
                    .protocol()
                    .config_update(&ctx, "llm.api_key", serde_json::json!(key))
            {
                return AliusError::from_err(e);
            }
        }

        AliusError::default()
    }

    /// Start a chat session with streaming support
    ///
    /// # Safety
    /// `message` must be a valid null-terminated C string
    /// `stream_callback` is called for each chunk of the response
    /// `error_callback` is called on errors
    /// `user_data` is passed to callbacks
    ///
    /// # Returns
    /// 0 on success, non-zero on failure
    #[no_mangle]
    pub unsafe extern "C" fn alius_chat(
        message: *const i8,
        stream_callback: StreamCallback,
        error_callback: extern "C" fn(code: i32, message: *const i8, user_data: *mut c_void),
        user_data: *mut c_void,
    ) -> i32 {
        if message.is_null() {
            error_callback(
                AliusErrorCode::InvalidArgument as i32,
                ptr::null(),
                user_data,
            );
            return -1;
        }

        let message_str = match unsafe { CStr::from_ptr(message) }.to_str() {
            Ok(s) => s,
            Err(e) => {
                let msg = CString::new(format!("invalid message: {}", e)).unwrap();
                error_callback(
                    AliusErrorCode::InvalidArgument as i32,
                    msg.as_ptr(),
                    user_data,
                );
                return -1;
            }
        };

        let request = match CoreRequest::start_turn(message_str) {
            Ok(req) => req,
            Err(e) => {
                let msg = CString::new(e.to_string()).unwrap();
                error_callback(
                    AliusErrorCode::InvalidArgument as i32,
                    msg.as_ptr(),
                    user_data,
                );
                return -1;
            }
        };

        let envelope = ProtocolEnvelope::new(
            Origin::EmbeddedSdk,
            CapabilityScope::embedded_sdk(),
            request,
        );

        let trace_id = envelope.trace_id.clone();

        let mut global = GLOBAL_RUNTIME.lock().unwrap();
        let runtime = match global.as_mut() {
            Some(r) => r,
            None => {
                let msg = CString::new("Runtime not initialized").unwrap();
                error_callback(AliusErrorCode::RuntimeError as i32, msg.as_ptr(), user_data);
                return -1;
            }
        };

        let run_ref = match runtime.protocol().start(envelope) {
            Ok(rf) => rf,
            Err(e) => {
                let msg = CString::new(e.to_string()).unwrap();
                error_callback(AliusErrorCode::RuntimeError as i32, msg.as_ptr(), user_data);
                return -1;
            }
        };

        // Start streaming
        let stream_handle = StreamHandle::new(
            Arc::clone(runtime.protocol()),
            run_ref.clone(),
            trace_id,
            stream_callback,
            error_callback,
            user_data,
        );

        runtime.set_stream_handle(Some(stream_handle));

        0
    }

    /// Cancel the active chat session
    #[no_mangle]
    pub extern "C" fn alius_chat_cancel() {
        let mut global = GLOBAL_RUNTIME.lock().unwrap();
        if let Some(runtime) = global.as_mut() {
            runtime.cancel_stream();
        }
    }

    /// Get current runtime version
    ///
    /// # Returns
    /// Pointer to null-terminated string, must be freed with alius_string_free
    #[no_mangle]
    pub extern "C" fn alius_version() -> *const i8 {
        static VERSION: &str = env!("CARGO_PKG_VERSION");
        crate::memory::string_to_c_string(VERSION)
    }

    /// Check runtime health
    ///
    /// # Returns
    /// AliusError with code=0 on success
    #[no_mangle]
    pub extern "C" fn alius_health_check() -> AliusError {
        let global = GLOBAL_RUNTIME.lock().unwrap();
        let runtime = match global.as_ref() {
            Some(r) => r,
            None => {
                return AliusError::new(AliusErrorCode::RuntimeError, "Runtime not initialized")
            }
        };

        let ctx = ProtocolContext {
            origin: Origin::EmbeddedSdk,
            capability_scope: CapabilityScope::embedded_sdk(),
            workspace_root: None,
        };

        match runtime.protocol().health_check(&ctx) {
            Ok(_) => AliusError::default(),
            Err(e) => AliusError::from_err(e),
        }
    }
}
