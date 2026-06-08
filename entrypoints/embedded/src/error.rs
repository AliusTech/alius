//! Error handling for the FFI layer

use crate::types::AliusErrorCode;
use std::ffi::CString;
use std::ptr;

/// FFI-compatible error representation
#[repr(C)]
#[derive(Debug)]
pub struct AliusError {
    pub code: AliusErrorCode,
    pub message: *mut i8, // Owned string, must be freed with alius_string_free
}

impl AliusError {
    /// Create a new error from a Rust error
    pub fn from_err<E: std::error::Error>(err: E) -> Self {
        let message = CString::new(err.to_string()).unwrap();
        let message_ptr = message.into_raw();

        AliusError {
            code: AliusErrorCode::Unknown,
            message: message_ptr,
        }
    }

    /// Create an error with specific code and message
    pub fn new(code: AliusErrorCode, message: &str) -> Self {
        let message = CString::new(message).unwrap();
        let message_ptr = message.into_raw();

        AliusError {
            code,
            message: message_ptr,
        }
    }
}

impl Default for AliusError {
    fn default() -> Self {
        Self {
            code: AliusErrorCode::Success,
            message: ptr::null_mut(),
        }
    }
}

/// Free an error's message string
#[no_mangle]
pub extern "C" fn alius_error_free(error: *mut AliusError) {
    if !error.is_null() {
        unsafe {
            let error_ref = &mut *error;
            if !error_ref.message.is_null() {
                let _ = CString::from_raw(error_ref.message);
            }
        }
    }
}
