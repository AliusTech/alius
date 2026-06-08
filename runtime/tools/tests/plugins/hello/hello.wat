;; Hello Test Plugin — minimal WASM plugin for Phase 1 verification.
;;
;; ABI contract:
;;   alius_plugin_list_tools() -> i32  (ptr to length-prefixed JSON in memory)
;;   alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len) -> i32

(module
  (memory (export "memory") 2)

  ;; alius_plugin_list_tools() -> i32
  ;; Returns ptr to: [4 bytes LE length][JSON string]
  (func (export "alius_plugin_list_tools") (result i32)
    (i32.const 1024)
  )

  ;; alius_plugin_call_tool(name_ptr, name_len, args_ptr, args_len) -> i32
  ;; Returns ptr to: [4 bytes LE length][JSON string]
  (func (export "alius_plugin_call_tool") (param i32 i32 i32 i32) (result i32)
    (i32.const 2048)
  )

  ;; Tool list JSON at offset 1024
  ;; Length: 155 bytes = 0x9b
  ;; JSON: [{"name":"hello","description":"Returns a greeting","inputSchema":{"type":"object","properties":{"name":{"type":"string"}}},"requires_confirmation":false}]
  (data (i32.const 1024) "\9b\00\00\00[{\"name\":\"hello\",\"description\":\"Returns a greeting\",\"inputSchema\":{\"type\":\"object\",\"properties\":{\"name\":{\"type\":\"string\"}}},\"requires_confirmation\":false}]")

  ;; Tool result JSON at offset 2048
  ;; Length: 41 bytes = 0x29
  ;; JSON: {"output":"Hello, world!","success":true}
  (data (i32.const 2048) "\29\00\00\00{\"output\":\"Hello, world!\",\"success\":true}")
)
