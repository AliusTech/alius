//! Hello World WASM plugin for Alius.
//!
//! Demonstrates the plugin ABI:
//! - `alius_plugin_list_tools()` → JSON array of tool definitions
//! - `alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len)` → result ptr
//!
//! This plugin exposes a single tool: `hello` which returns a greeting.

use std::alloc::{alloc, Layout};
use std::mem;

/// Write a length-prefixed string to a newly allocated region.
/// Returns a pointer to the 4-byte LE length prefix followed by the string bytes.
fn write_result(s: &[u8]) -> *mut u8 {
    let total = 4 + s.len();
    let layout = Layout::from_size_align(total, 4).unwrap();
    unsafe {
        let ptr = alloc(layout);
        let len_bytes = (s.len() as u32).to_le_bytes();
        std::ptr::copy_nonoverlapping(len_bytes.as_ptr(), ptr, 4);
        std::ptr::copy_nonoverlapping(s.as_ptr(), ptr.add(4), s.len());
        ptr
    }
}

/// Read a string from WASM linear memory.
unsafe fn read_string(ptr: *const u8, len: usize) -> String {
    let slice = std::slice::from_raw_parts(ptr, len);
    String::from_utf8_lossy(slice).to_string()
}

/// Export: list available tools as JSON array.
#[no_mangle]
pub extern "C" fn alius_plugin_list_tools() -> i32 {
    let tools = serde_json::json!([
        {
            "name": "hello",
            "description": "Returns a greeting message. Args: {\"name\": \"...\"}",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "name": { "type": "string", "description": "Name to greet" }
                },
                "required": ["name"]
            },
            "requires_confirmation": false
        }
    ]);
    let json = serde_json::to_string(&tools).unwrap();
    write_result(json.as_bytes()) as i32
}

/// Export: call a tool by name.
#[no_mangle]
pub extern "C" fn alius_plugin_call_tool(
    name_ptr: *const u8,
    name_len: i32,
    args_ptr: *const u8,
    args_len: i32,
) -> i32 {
    let name = unsafe { read_string(name_ptr, name_len as usize) };
    let args_str = unsafe { read_string(args_ptr, args_len as usize) };

    let result = match name.as_str() {
        "hello" => {
            let args: serde_json::Value = serde_json::from_str(&args_str).unwrap_or_default();
            let who = args["name"].as_str().unwrap_or("World");
            serde_json::json!({
                "success": true,
                "output": format!("Hello, {who}!")
            })
        }
        _ => {
            serde_json::json!({
                "success": false,
                "output": format!("Unknown tool: {name}")
            })
        }
    };

    let json = serde_json::to_string(&result).unwrap();
    write_result(json.as_bytes()) as i32
}
