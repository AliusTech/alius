//! Loader for protocol.toml.

use crate::error::ConfigResult;
use crate::views::{
    A2AConfig, CommandConfig, EventConfig, FfiConfig, IdeRpcConfig, JsonRpcConfig, LocalRustConfig,
    ProtocolConfig, ProtocolVersionConfig,
};
use std::path::Path;

/// Load protocol.toml from the given path.
pub fn load_protocol(path: &Path) -> ConfigResult<ProtocolConfig> {
    let raw: ProtocolToml = super::load_toml(path)?;
    Ok(raw.into())
}

/// protocol.toml raw structure.
#[derive(Debug, Clone, serde::Deserialize)]
struct ProtocolToml {
    protocol: ProtocolVersionConfig,
    local_rust: LocalRustConfig,
    json_rpc: JsonRpcConfig,
    ide_rpc: IdeRpcConfig,
    a2a: A2AConfig,
    ffi: FfiConfig,
    events: EventConfig,
    commands: CommandConfig,
}

impl From<ProtocolToml> for ProtocolConfig {
    fn from(raw: ProtocolToml) -> Self {
        Self {
            protocol: raw.protocol,
            local_rust: raw.local_rust,
            json_rpc: raw.json_rpc,
            ide_rpc: raw.ide_rpc,
            a2a: raw.a2a,
            ffi: raw.ffi,
            events: raw.events,
            commands: raw.commands,
        }
    }
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            protocol: ProtocolVersionConfig {
                major: 1,
                minor: 0,
                trace_enabled: true,
                event_sequence_enabled: true,
            },
            local_rust: LocalRustConfig {
                enabled: true,
                transport: "in-process".to_string(),
                default_origin: "LocalTui".to_string(),
            },
            json_rpc: JsonRpcConfig {
                enabled: false,
                transport: "stdio-or-socket".to_string(),
                socket_path: ".alius/run/alius.sock".to_string(),
                method_prefix: "alius".to_string(),
            },
            ide_rpc: IdeRpcConfig {
                enabled: false,
                transport: "lsp-like".to_string(),
                workspace_scoped_filesystem: true,
            },
            a2a: A2AConfig {
                enabled: false,
                server_enabled: false,
                client_enabled: false,
                agent_card_source: ".alius/config/soul.toml".to_string(),
                default_remote_capability: "minimal".to_string(),
            },
            ffi: FfiConfig {
                enabled: false,
                core: "lite".to_string(),
                event_delivery: "poll".to_string(),
            },
            events: EventConfig {
                buffer_size: 1024,
                persist: true,
                allow_resume: true,
                visibility_default: "ProductVisible".to_string(),
            },
            commands: CommandConfig {
                approve_tool: true,
                reject_tool: true,
                answer_question: true,
                select_option: true,
                update_plan: true,
                cancel_run: true,
                pause_run: false,
                resume_run: true,
            },
        }
    }
}
