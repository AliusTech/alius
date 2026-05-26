//! JSON tool - parse and query JSON data

use async_trait::async_trait;
use serde_json::Value as JsonValue;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel};
use alius_protocol::AliusError;

pub struct JsonTool;

#[async_trait]
impl AliusTool for JsonTool {
    fn name(&self) -> &'static str {
        "json"
    }

    fn description(&self) -> &'static str {
        "Parse and query JSON data. Actions: parse, get, keys, values"
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["parse", "get", "keys", "values", "validate"],
                    "description": "Action to perform"
                },
                "data": {
                    "type": "string",
                    "description": "JSON string to parse"
                },
                "path": {
                    "type": "string",
                    "description": "JSON path (e.g. 'key.nested.0')"
                }
            },
            "required": ["action", "data"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    async fn execute(
        &self,
        args: JsonValue,
        _ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let action = args["action"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'action'".to_string()))?;

        let data = args["data"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'data'".to_string()))?;

        match action {
            "parse" | "validate" => {
                match serde_json::from_str::<JsonValue>(data) {
                    Ok(json) => Ok(ToolResult::success(
                        serde_json::to_string_pretty(&json).unwrap_or_default()
                    )),
                    Err(e) => Err(AliusError::Agent(format!("Invalid JSON: {}", e))),
                }
            }
            "get" => {
                let path = args["path"].as_str()
                    .ok_or_else(|| AliusError::Agent("Missing 'path'".to_string()))?;

                let json: JsonValue = serde_json::from_str(data)
                    .map_err(|e| AliusError::Agent(format!("Invalid JSON: {}", e)))?;

                let result = Self::get_by_path(&json, path)?;
                Ok(ToolResult::success(serde_json::to_string_pretty(&result).unwrap_or_default()))
            }
            "keys" => {
                let json: JsonValue = serde_json::from_str(data)
                    .map_err(|e| AliusError::Agent(format!("Invalid JSON: {}", e)))?;

                if let Some(obj) = json.as_object() {
                    let keys: Vec<&str> = obj.keys().map(|s| s.as_str()).collect();
                    Ok(ToolResult::success(keys.join(", ")))
                } else {
                    Err(AliusError::Agent("Not a JSON object".to_string()))
                }
            }
            "values" => {
                let json: JsonValue = serde_json::from_str(data)
                    .map_err(|e| AliusError::Agent(format!("Invalid JSON: {}", e)))?;

                if let Some(obj) = json.as_object() {
                    let values: Vec<String> = obj.values()
                        .map(|v| v.to_string())
                        .collect();
                    Ok(ToolResult::success(values.join("\n")))
                } else if let Some(arr) = json.as_array() {
                    let values: Vec<String> = arr.iter()
                        .map(|v| v.to_string())
                        .collect();
                    Ok(ToolResult::success(values.join("\n")))
                } else {
                    Err(AliusError::Agent("Not a JSON object or array".to_string()))
                }
            }
            _ => Err(AliusError::Agent(format!("Unknown action: {}", action)))
        }
    }
}

impl JsonTool {
    fn get_by_path(json: &JsonValue, path: &str) -> Result<JsonValue, AliusError> {
        let parts: Vec<&str> = path.split('.').collect();
        let mut current = json.clone();

        for part in parts {
            if let Ok(index) = part.parse::<usize>() {
                if let Some(arr) = current.as_array() {
                    current = arr.get(index)
                        .ok_or_else(|| AliusError::Agent(format!("Index {} out of bounds", index)))?
                        .clone();
                } else {
                    return Err(AliusError::Agent("Not an array".to_string()));
                }
            } else if let Some(obj) = current.as_object() {
                current = obj.get(part)
                    .ok_or_else(|| AliusError::Agent(format!("Key '{}' not found", part)))?
                    .clone();
            } else {
                return Err(AliusError::Agent("Not an object".to_string()));
            }
        }

        Ok(current)
    }
}