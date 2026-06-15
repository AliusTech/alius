//! Build Core RunLoop requests from REPL input.

use protocol_interface::core::{CoreRequest, LoopPolicy, ProtocolError};

use crate::repl::mode::ReplMode;

#[allow(dead_code)]
pub fn build_loop_request(input: &str, mode: ReplMode) -> Result<CoreRequest, ProtocolError> {
    let policy = match mode {
        ReplMode::Chat => LoopPolicy::chat(),
        ReplMode::Plan => LoopPolicy::plan(),
    };

    CoreRequest::run_loop(input.to_string(), mode.into(), policy)
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::core::{CoreRequestKind, RequestInput, RuntimeMode};

    #[test]
    fn chat_mode_builds_tool_enabled_bypass_policy() {
        let request = build_loop_request("hello", ReplMode::Chat).unwrap();
        assert_eq!(request.kind, CoreRequestKind::RunLoop);

        match request.input {
            RequestInput::RunLoop { input } => {
                assert_eq!(input.mode, RuntimeMode::Chat);
                assert_eq!(input.policy.max_iterations, 10);
                assert!(input.policy.tools_enabled);
                assert!(!input.policy.planning_enabled);
            }
            other => panic!("unexpected input: {:?}", other),
        }
    }

    #[test]
    fn plan_mode_builds_multi_iteration_tools_policy() {
        let request = build_loop_request("change code", ReplMode::Plan).unwrap();

        match request.input {
            RequestInput::RunLoop { input } => {
                assert_eq!(input.mode, RuntimeMode::Plan);
                assert_eq!(input.policy.max_iterations, 20);
                assert!(input.policy.tools_enabled);
                assert!(input.policy.planning_enabled);
            }
            other => panic!("unexpected input: {:?}", other),
        }
    }
}
