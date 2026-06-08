//! Todo list tool - manage simple task lists

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::{AliusTool, PermissionLevel, ToolContext, ToolResult};
use protocol_interface::AliusError;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// Simple in-memory todo storage (per session)
lazy_static::lazy_static! {
    static ref TODO_STORE: Arc<Mutex<HashMap<String, Vec<TodoItem>>>> = Arc::new(Mutex::new(HashMap::new()));
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct TodoItem {
    id: usize,
    text: String,
    done: bool,
}

pub struct TodoTool;

#[async_trait]
impl AliusTool for TodoTool {
    fn name(&self) -> &'static str {
        "todo"
    }

    fn description(&self) -> &'static str {
        "Manage a simple todo list. Actions: add, list, done, remove, clear"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["add", "list", "done", "remove", "clear"],
                    "description": "Action to perform"
                },
                "text": {
                    "type": "string",
                    "description": "Todo text (for 'add')"
                },
                "id": {
                    "type": "integer",
                    "description": "Todo ID (for 'done' and 'remove')"
                }
            },
            "required": ["action"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read // Low permission - just managing a list
    }

    async fn execute(&self, args: JsonValue, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'action' argument".to_string()))?;

        let session_id = ctx.session_id.clone();
        let mut store = TODO_STORE.lock().unwrap();

        match action {
            "add" => {
                let text = args["text"]
                    .as_str()
                    .ok_or_else(|| AliusError::Agent("Missing 'text' for add".to_string()))?;

                let todos = store.entry(session_id).or_default();
                let id = todos.len() + 1;
                todos.push(TodoItem {
                    id,
                    text: text.to_string(),
                    done: false,
                });

                Ok(ToolResult::success(format!("Added todo #{}: {}", id, text)))
            }
            "list" => {
                let todos = store.entry(session_id).or_default();
                if todos.is_empty() {
                    Ok(ToolResult::success("No todos".to_string()))
                } else {
                    let list = todos
                        .iter()
                        .map(|t| {
                            format!("{} [{}] {}", t.id, if t.done { "x" } else { " " }, t.text)
                        })
                        .collect::<Vec<_>>()
                        .join("\n");
                    Ok(ToolResult::success(list))
                }
            }
            "done" => {
                let id = args["id"]
                    .as_u64()
                    .ok_or_else(|| AliusError::Agent("Missing 'id' for done".to_string()))?
                    as usize;

                let todos = store.entry(session_id).or_default();
                if let Some(todo) = todos.iter_mut().find(|t| t.id == id) {
                    todo.done = true;
                    Ok(ToolResult::success(format!("Marked todo #{} as done", id)))
                } else {
                    Err(AliusError::Agent(format!("Todo #{} not found", id)))
                }
            }
            "remove" => {
                let id = args["id"]
                    .as_u64()
                    .ok_or_else(|| AliusError::Agent("Missing 'id' for remove".to_string()))?
                    as usize;

                let todos = store.entry(session_id).or_default();
                if let Some(pos) = todos.iter().position(|t| t.id == id) {
                    let removed = todos.remove(pos);
                    Ok(ToolResult::success(format!(
                        "Removed todo #{}: {}",
                        id, removed.text
                    )))
                } else {
                    Err(AliusError::Agent(format!("Todo #{} not found", id)))
                }
            }
            "clear" => {
                store.entry(session_id).or_default().clear();
                Ok(ToolResult::success("All todos cleared".to_string()))
            }
            _ => Err(AliusError::Agent(format!("Unknown action: {}", action))),
        }
    }
}
