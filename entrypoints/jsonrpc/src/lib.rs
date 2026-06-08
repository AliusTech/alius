//! JSON-RPC adapter boundary for Alius.
//!
//! This crate is intentionally minimal for the current architecture pass. It
//! owns JSON serialization/deserialization at the product/interface boundary
//! and delegates execution semantics to `protocol-interface`.

use protocol_interface::core::{CoreCommand, CoreEvent, CoreRequest, ProtocolEnvelope};

pub fn decode_request(
    value: serde_json::Value,
) -> serde_json::Result<ProtocolEnvelope<CoreRequest>> {
    serde_json::from_value(value)
}

pub fn decode_command(
    value: serde_json::Value,
) -> serde_json::Result<ProtocolEnvelope<CoreCommand>> {
    serde_json::from_value(value)
}

pub fn encode_event(event: &ProtocolEnvelope<CoreEvent>) -> serde_json::Result<serde_json::Value> {
    serde_json::to_value(event)
}
