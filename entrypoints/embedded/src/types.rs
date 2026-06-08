//! C-compatible type definitions for the FFI layer

use std::ffi::c_void;

/// Opaque handle to an Alius runtime instance
#[repr(C)]
pub struct AliusRuntime {
    // Internal fields - opaque to C
    inner: *mut c_void,
}

/// Opaque handle to a single execution run
#[repr(C)]
pub struct AliusRun {
    // Internal fields - opaque to C
    inner: *mut c_void,
}

/// Stream callback function type
///
/// Called for each chunk of streaming response
#[allow(dead_code)]
pub type StreamCallback = extern "C" fn(delta: *const i8, user_data: *mut c_void);

/// Error callback function type
///
/// Called when an error occurs during execution
#[allow(dead_code)]
pub type ErrorCallback = extern "C" fn(code: i32, message: *const i8, user_data: *mut c_void);

/// C-compatible error codes
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AliusErrorCode {
    Success = 0,
    InvalidArgument = 1,
    RuntimeError = 2,
    NetworkError = 3,
    ModelError = 4,
    ToolError = 5,
    ConfigError = 6,
    Cancelled = 7,
    Unknown = -1,
}

// Safety: AliusRuntime and AliusRun are safe to send across threads
// as long as their inner pointers are valid
unsafe impl Send for AliusRuntime {}
unsafe impl Send for AliusRun {}

// Safety: AliusRuntime and AliusRun can be shared across threads
// with proper synchronization (internally managed)
unsafe impl Sync for AliusRuntime {}
unsafe impl Sync for AliusRun {}
