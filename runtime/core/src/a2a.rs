//! Agent-to-Agent (A2A) transport abstraction.
//!
//! Provides a trait-based interface for sending and receiving messages
//! between agents. The initial implementation is local-only (in-process
//! message passing via channels). Future implementations can add network
//! transport (WebSocket, HTTP, etc.) without changing the consumer code.
//!
//! ## Design
//!
//! - `A2ATransport` trait: send/receive/subscribe to messages
//! - `LocalA2ATransport`: in-process implementation using tokio channels
//! - `A2AMessage`: message envelope with routing metadata
//! - `AgentEndpoint`: identifies an agent by soul/node_id/role

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Agent endpoint identifier.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct AgentEndpoint {
    /// Soul ID (e.g. "coder", "researcher").
    pub soul: String,
    /// Node ID (unique instance identifier).
    pub node_id: String,
    /// Optional role within a team.
    pub role: Option<String>,
}

impl AgentEndpoint {
    pub fn new(soul: &str, node_id: &str) -> Self {
        Self {
            soul: soul.to_string(),
            node_id: node_id.to_string(),
            role: None,
        }
    }

    pub fn with_role(mut self, role: &str) -> Self {
        self.role = Some(role.to_string());
        self
    }
}

/// Message type enum.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum A2AMessageType {
    PlanRequest,
    PlanResponse,
    TaskDelegate,
    TaskResult,
    ReviewRequest,
    ReviewResponse,
    ContextShare,
    Error,
    Heartbeat,
}

/// Message delivery status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum A2AMessageStatus {
    Sending,
    Sent,
    Delivered,
    Acknowledged,
    Failed,
}

/// Agent-to-agent message envelope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessage {
    pub message_id: String,
    pub trace_id: String,
    pub conversation_id: Option<String>,
    pub plan_id: Option<String>,
    pub plan_node_id: Option<String>,
    pub from: AgentEndpoint,
    pub to: AgentEndpoint,
    pub message_type: A2AMessageType,
    pub status: A2AMessageStatus,
    pub content: String,
    pub created_at: String,
}

/// Transport trait for agent-to-agent communication.
///
/// Implementations handle message routing, delivery, and persistence.
/// The trait is async to support network transports.
#[async_trait::async_trait]
pub trait A2ATransport: Send + Sync {
    /// Send a message to a specific agent.
    async fn send(&self, message: A2AMessage) -> Result<()>;

    /// Receive the next message for a specific agent.
    /// Returns None if no messages are available (non-blocking).
    async fn receive(&self, agent: &AgentEndpoint) -> Result<Option<A2AMessage>>;

    /// Subscribe to all messages for a specific agent.
    /// Returns a broadcast receiver that yields messages as they arrive.
    fn subscribe(&self, agent: &AgentEndpoint) -> broadcast::Receiver<A2AMessage>;

    /// List all known agents (registered or recently active).
    async fn list_agents(&self) -> Result<Vec<AgentEndpoint>>;
}

/// Local in-process A2A transport using tokio channels.
///
/// Messages are routed through a shared broadcast channel. Each agent
/// can subscribe to receive messages addressed to it.
pub struct LocalA2ATransport {
    /// Broadcast sender for all messages.
    tx: broadcast::Sender<A2AMessage>,
    /// Registered agents.
    agents: Arc<RwLock<HashMap<String, AgentEndpoint>>>,
}

impl LocalA2ATransport {
    /// Create a new local transport with the given channel capacity.
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self {
            tx,
            agents: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register an agent with the transport.
    pub async fn register(&self, agent: AgentEndpoint) {
        let key = format!("{}:{}", agent.soul, agent.node_id);
        self.agents.write().await.insert(key, agent);
    }
}

#[async_trait::async_trait]
impl A2ATransport for LocalA2ATransport {
    async fn send(&self, mut message: A2AMessage) -> Result<()> {
        message.status = A2AMessageStatus::Sent;
        // Broadcast to all subscribers. Ignores error if no receivers.
        let _ = self.tx.send(message);
        Ok(())
    }

    async fn receive(&self, _agent: &AgentEndpoint) -> Result<Option<A2AMessage>> {
        // Non-blocking receive is not supported on broadcast channels.
        // Use subscribe() instead for async receive.
        Ok(None)
    }

    fn subscribe(&self, _agent: &AgentEndpoint) -> broadcast::Receiver<A2AMessage> {
        self.tx.subscribe()
    }

    async fn list_agents(&self) -> Result<Vec<AgentEndpoint>> {
        Ok(self.agents.read().await.values().cloned().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_message(from: &str, to: &str, content: &str) -> A2AMessage {
        A2AMessage {
            message_id: uuid::Uuid::new_v4().to_string(),
            trace_id: "test-trace".to_string(),
            conversation_id: None,
            plan_id: None,
            plan_node_id: None,
            from: AgentEndpoint::new(from, &format!("{}-node", from)),
            to: AgentEndpoint::new(to, &format!("{}-node", to)),
            message_type: A2AMessageType::TaskDelegate,
            status: A2AMessageStatus::Sending,
            content: content.to_string(),
            created_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    #[tokio::test]
    async fn test_local_transport_send_receive() {
        let transport = LocalA2ATransport::new(16);
        let agent = AgentEndpoint::new("coder", "coder-1");
        transport.register(agent.clone()).await;

        let mut rx = transport.subscribe(&agent);

        let msg = make_message("planner", "coder", "implement feature X");
        transport.send(msg).await.unwrap();

        let received = rx.recv().await.unwrap();
        assert_eq!(received.content, "implement feature X");
        assert_eq!(received.from.soul, "planner");
        assert_eq!(received.to.soul, "coder");
        assert_eq!(received.status, A2AMessageStatus::Sent);
    }

    #[tokio::test]
    async fn test_local_transport_multiple_subscribers() {
        let transport = LocalA2ATransport::new(16);

        let agent1 = AgentEndpoint::new("coder", "coder-1");
        let agent2 = AgentEndpoint::new("reviewer", "reviewer-1");
        transport.register(agent1.clone()).await;
        transport.register(agent2.clone()).await;

        let mut rx1 = transport.subscribe(&agent1);
        let mut rx2 = transport.subscribe(&agent2);

        let msg = make_message("planner", "coder", "task assigned");
        transport.send(msg).await.unwrap();

        // Both subscribers should receive the broadcast
        let r1 = rx1.recv().await.unwrap();
        let r2 = rx2.recv().await.unwrap();
        assert_eq!(r1.content, "task assigned");
        assert_eq!(r2.content, "task assigned");
    }

    #[tokio::test]
    async fn test_local_transport_list_agents() {
        let transport = LocalA2ATransport::new(16);

        transport
            .register(AgentEndpoint::new("coder", "coder-1"))
            .await;
        transport
            .register(AgentEndpoint::new("reviewer", "reviewer-1"))
            .await;

        let agents = transport.list_agents().await.unwrap();
        assert_eq!(agents.len(), 2);
    }

    #[tokio::test]
    async fn test_agent_endpoint_with_role() {
        let agent = AgentEndpoint::new("coder", "coder-1").with_role("lead");
        assert_eq!(agent.role, Some("lead".to_string()));
    }

    #[tokio::test]
    async fn test_message_types() {
        let types = [
            A2AMessageType::PlanRequest,
            A2AMessageType::PlanResponse,
            A2AMessageType::TaskDelegate,
            A2AMessageType::TaskResult,
            A2AMessageType::ReviewRequest,
            A2AMessageType::ReviewResponse,
            A2AMessageType::ContextShare,
            A2AMessageType::Error,
            A2AMessageType::Heartbeat,
        ];
        // All message types should be distinct
        for i in 0..types.len() {
            for j in i + 1..types.len() {
                assert_ne!(types[i], types[j]);
            }
        }
    }
}
