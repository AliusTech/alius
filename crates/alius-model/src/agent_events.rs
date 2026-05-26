//! Agent events for tool calling loop

use serde_json::Value as JsonValue;

/// Event from agent execution
#[derive(Debug, Clone)]
pub enum AgentEvent {
    /// Turn started
    TurnStarted,

    /// Model started streaming
    ModelStarted,

    /// Text delta from model
    ModelDelta { text: String },

    /// Model finished
    ModelFinished { full_response: String },

    /// Tool call started
    ToolCallStarted {
        id: String,
        name: String,
        args: JsonValue,
    },

    /// Tool requires user confirmation
    ToolConfirmationRequested {
        id: String,
        name: String,
        operation: String,
        details: String,
    },

    /// User confirmed tool execution
    ToolConfirmed { id: String },

    /// User denied tool execution
    ToolDenied { id: String, reason: String },

    /// Tool call finished
    ToolCallFinished {
        id: String,
        name: String,
        result: String,
        success: bool,
    },

    /// Turn finished (no more tool calls)
    TurnFinished,

    /// Error occurred
    Error { message: String },
}

/// Tool call from model
#[derive(Debug, Clone)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub args: JsonValue,
}

impl ToolCall {
    pub fn new(id: String, name: String, args: JsonValue) -> Self {
        Self { id, name, args }
    }
}