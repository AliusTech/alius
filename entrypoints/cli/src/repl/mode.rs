//! REPL runtime mode.

use protocol_interface::core::RuntimeMode;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplMode {
    Chat,
    Plan,
}

impl ReplMode {
    pub fn toggle(self) -> Self {
        match self {
            ReplMode::Chat => ReplMode::Plan,
            ReplMode::Plan => ReplMode::Chat,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ReplMode::Chat => "chat",
            ReplMode::Plan => "plan",
        }
    }
}

impl From<ReplMode> for RuntimeMode {
    fn from(value: ReplMode) -> Self {
        match value {
            ReplMode::Chat => RuntimeMode::Chat,
            ReplMode::Plan => RuntimeMode::Plan,
        }
    }
}
