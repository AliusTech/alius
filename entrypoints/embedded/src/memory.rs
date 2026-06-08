//! Memory management utilities for the FFI layer

use std::ffi::{c_char, c_int, CStr, CString};

/// Convert a C string to a Rust String
///
/// # Safety
/// `ptr` must be a valid pointer to a null-terminated C string, or NULL
#[allow(dead_code)]
pub unsafe fn c_str_to_string(ptr: *const c_char) -> Result<String, std::str::Utf8Error> {
    if ptr.is_null() {
        return Ok(String::new());
    }
    CStr::from_ptr(ptr).to_str().map(|s| s.to_string())
}

/// Convert a Rust String to a C string (allocated on the heap)
///
/// Returns a pointer that must be freed with `alius_string_free`
pub fn string_to_c_string(s: &str) -> *mut c_char {
    CString::new(s).unwrap().into_raw()
}

/// Convert a Rust String to a C string in a pre-allocated buffer
///
/// # Safety
/// `buffer` must be a valid pointer to a buffer of at least `buffer_size` bytes
#[allow(dead_code)]
pub unsafe fn string_to_c_buffer(s: &str, buffer: *mut c_char, buffer_size: usize) -> c_int {
    if buffer.is_null() || buffer_size == 0 {
        return -(s.len() as c_int) - 1; // Return required size (negative)
    }

    let bytes = s.as_bytes();
    let copy_len = bytes.len().min(buffer_size - 1);

    std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer as *mut u8, copy_len);
    *buffer.add(copy_len) = 0; // Null terminator

    if copy_len < bytes.len() {
        -(bytes.len() as c_int) // Buffer too small, return required size
    } else {
        0 // Success
    }
}

/// Free a string allocated by the FFI layer
#[no_mangle]
pub extern "C" fn alius_string_free(s: *mut c_char) {
    if !s.is_null() {
        unsafe {
            let _ = CString::from_raw(s);
        }
    }
}

/// Get the length of a C string
///
/// # Safety
/// `s` must be a valid pointer to a null-terminated C string, or NULL
#[no_mangle]
pub extern "C" fn alius_string_len(s: *const c_char) -> usize {
    if s.is_null() {
        return 0;
    }
    unsafe {
        let mut len = 0;
        while *s.add(len) != 0 {
            len += 1;
        }
        len
    }
}
