//! Edit file tool

use async_trait::async_trait;
use serde_json::Value as JsonValue;
use std::path::PathBuf;

use crate::{AliusTool, ToolContext, ToolResult, PermissionLevel};
use alius_protocol::AliusError;

pub struct EditFileTool;

#[async_trait]
impl AliusTool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }

    fn description(&self) -> &'static str {
        "Edit a file by replacing exact string matches. Use for precise code modifications."
    }

    fn input_schema(&self) -> JsonValue {
        serde_json::json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The path to the file to edit (relative to workspace or absolute)"
                },
                "old_string": {
                    "type": "string",
                    "description": "The exact string to find and replace"
                },
                "new_string": {
                    "type": "string",
                    "description": "The replacement string"
                },
                "replace_all": {
                    "type": "boolean",
                    "description": "Replace all occurrences (default: false)",
                    "default": false
                }
            },
            "required": ["path", "old_string", "new_string"]
        })
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Write
    }

    fn requires_confirmation(&self, _args: &JsonValue) -> bool {
        true // Editing files always requires confirmation
    }

    fn confirmation_request(&self, args: &JsonValue) -> Option<crate::ConfirmationRequest> {
        let path = args["path"].as_str().unwrap_or("unknown");
        let old_len = args["old_string"].as_str().map(|s| s.len()).unwrap_or(0);
        Some(crate::ConfirmationRequest {
            tool_name: self.name().to_string(),
            operation: "edit file".to_string(),
            details: format!("Path: {}\nReplace {} chars with {} chars",
                path, old_len,
                args["new_string"].as_str().map(|s| s.len()).unwrap_or(0)),
        })
    }

    async fn execute(
        &self,
        args: JsonValue,
        ctx: ToolContext,
    ) -> Result<ToolResult, AliusError> {
        let path = args["path"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'path' argument".to_string()))?;

        let old_string = args["old_string"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'old_string' argument".to_string()))?;

        let new_string = args["new_string"].as_str()
            .ok_or_else(|| AliusError::Agent("Missing 'new_string' argument".to_string()))?;

        let replace_all = args["replace_all"].as_bool().unwrap_or(false);

        // Resolve path relative to workspace
        let full_path = if path.starts_with('/') {
            PathBuf::from(path)
        } else {
            ctx.workspace.join(path)
        };

        // Validate path is within workspace
        let canonical_path = full_path.canonicalize()
            .map_err(|e| AliusError::Io(e))?;

        let canonical_workspace = ctx.workspace.canonicalize()
            .map_err(|e| AliusError::Io(e))?;

        if !canonical_path.starts_with(&canonical_workspace) {
            return Err(AliusError::Agent(
                "Path is outside workspace - access denied".to_string()
            ));
        }

        // Read file content
        let content = tokio::fs::read_to_string(&full_path)
            .await
            .map_err(|e| AliusError::Io(e))?;

        // Check if old_string exists
        if !content.contains(old_string) {
            return Err(AliusError::Agent(
                format!("String not found in file: {}", old_string)
            ));
        }

        // Perform replacement
        let new_content = if replace_all {
            content.replace(old_string, new_string)
        } else {
            // Replace only first occurrence
            let mut replaced = false;
            let mut result = String::new();
            let mut chars = content.chars().peekable();
            let old_chars: Vec<char> = old_string.chars().collect();

            while let Some(c) = chars.next() {
                if !replaced && c == old_chars[0] {
                    // Check if this is the start of old_string
                    let mut match_pos = 1;
                    let mut temp_chars = Vec::new();
                    temp_chars.push(c);

                    while match_pos < old_chars.len() {
                        if let Some(&next) = chars.peek() {
                            if next == old_chars[match_pos] {
                                temp_chars.push(chars.next().unwrap());
                                match_pos += 1;
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    if match_pos == old_chars.len() {
                        result.push_str(new_string);
                        replaced = true;
                    } else {
                        result.extend(temp_chars);
                    }
                } else {
                    result.push(c);
                }
            }
            result
        };

        // Write modified content
        tokio::fs::write(&full_path, new_content)
            .await
            .map_err(|e| AliusError::Io(e))?;

        Ok(ToolResult::success(format!("File edited: {}", path)))
    }
}