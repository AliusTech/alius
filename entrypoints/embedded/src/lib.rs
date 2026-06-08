//! Alius Embedded SDK - FFI Library
//!
//! This library provides a C-compatible FFI interface for integrating Alius
//! into embedded systems (ESP32, STM32) and third-party applications.

#![allow(non_camel_case_types)]

mod error;
mod memory;
mod runtime;
mod stream;
mod types;

pub use error::AliusError;
pub use runtime::EmbeddedRuntime;
pub use stream::{StreamCallback, StreamHandle};
pub use types::AliusErrorCode;
pub use types::*;

// Re-export C-compatible FFI functions
pub use runtime::ffi::*;
