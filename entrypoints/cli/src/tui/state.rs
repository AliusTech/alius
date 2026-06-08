use rust_i18n::t;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentHeader {
    pub version: String,
    pub soul: String,
    pub network_status: AgentNetworkStatus,
    pub node_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum AgentNetworkStatus {
    Standalone,
    AgentNetConnected,
    AgentNetSyncing,
    AgentNetDegraded,
    AgentNetOffline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: String,
    pub title: String,
    pub status: PlanNodeStatus,
    pub description: Option<String>,
    pub acceptance_criteria: Vec<String>,
    pub evidence: Vec<String>,

    /// Reserved for AgentNet / Agent Team coordination.
    /// Examples: "local", "planner-agent", "coder-agent", "reviewer-agent".
    pub owner: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanNodeStatus {
    Pending,
    Running,
    Completed,
    Review,
    Approved,
    Revising,
    Failed,
    Blocked,
    Cancelled,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum InteractionMode {
    Plan,
    Bypass,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub enum InteractionSurface {
    TextInput {
        mode: InteractionMode,
        placeholder: String,
        value: String,
    },
    SingleChoice {
        mode: InteractionMode,
        title: String,
        description: String,
        options: Vec<ChoiceOption>,
        selected_index: usize,

        /// The final single-choice option must allow a custom user response.
        custom_response_enabled: bool,
        custom_response_value: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ChoiceOption {
    pub id: String,
    pub label: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceStatus {
    pub cwd: String,
    pub repo: Option<String>,
    pub branch: Option<String>,
    pub staged: u32,
    pub modified: u32,
    pub untracked: u32,
    pub clean: bool,
    pub git_available: bool,
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentEndpoint {
    pub soul: String,
    pub node_id: String,
    pub role: Option<String>,
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum A2AMessageStatus {
    Sending,
    Sent,
    Delivered,
    Acknowledged,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum A2ADirection {
    In,
    Out,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A2AMessageView {
    pub direction: A2ADirection,
    pub message: A2AMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct TuiState {
    pub header: AgentHeader,
    pub active_tab: MainTab,
    pub conversation: ConversationState,
    pub plans: Vec<PlanNode>,
    pub interaction_surface: InteractionSurface,
    pub workspace_status: WorkspaceStatus,
    pub agent_team: Option<AgentTeamState>,
    pub active_trace_id: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MainTab {
    Conversation,
    AgentTeam,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct ConversationState {
    pub model: Option<String>,
    pub blocks: Vec<ConversationBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationBlock {
    pub block_type: ConversationBlockType,
    pub title: Option<String>,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ConversationBlockType {
    Request,
    Understanding,
    PlanProposal,
    Execution,
    Streaming,
    Decision,
    Result,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTeamState {
    pub messages: Vec<A2AMessageView>,
    pub connected_agents: Vec<AgentEndpoint>,
}

impl AgentHeader {
    #[allow(dead_code)]
    pub fn should_show_agent_team_tab(&self) -> bool {
        !matches!(self.network_status, AgentNetworkStatus::Standalone)
    }

    #[allow(dead_code)]
    pub fn network_label(&self, show_node_id: bool) -> String {
        match self.network_status {
            AgentNetworkStatus::Standalone => t!("workspace.network.standalone").to_string(),
            AgentNetworkStatus::AgentNetConnected => {
                if show_node_id {
                    self.node_id
                        .as_ref()
                        .map(|id| t!("workspace.network.node", id = id).to_string())
                        .unwrap_or_else(|| t!("workspace.network.node_unknown").to_string())
                } else {
                    t!("workspace.network.connected").to_string()
                }
            }
            AgentNetworkStatus::AgentNetSyncing => t!("workspace.network.syncing").to_string(),
            AgentNetworkStatus::AgentNetDegraded => {
                if show_node_id {
                    self.node_id
                        .as_ref()
                        .map(|id| t!("workspace.network.node", id = id).to_string())
                        .unwrap_or_else(|| t!("workspace.network.node_unknown").to_string())
                } else {
                    t!("workspace.network.degraded").to_string()
                }
            }
            AgentNetworkStatus::AgentNetOffline => {
                if show_node_id {
                    self.node_id
                        .as_ref()
                        .map(|id| t!("workspace.network.node", id = id).to_string())
                        .unwrap_or_else(|| t!("workspace.network.node_unknown").to_string())
                } else {
                    t!("workspace.network.offline").to_string()
                }
            }
        }
    }
}

impl PlanNodeStatus {
    #[allow(dead_code)]
    pub fn icon(&self) -> &'static str {
        match self {
            PlanNodeStatus::Pending => "○",
            PlanNodeStatus::Running => "⏺",
            PlanNodeStatus::Completed => "✓",
            PlanNodeStatus::Review => "◎",
            PlanNodeStatus::Approved => "✔",
            PlanNodeStatus::Revising => "↻",
            PlanNodeStatus::Failed => "×",
            PlanNodeStatus::Blocked => "⚠",
            PlanNodeStatus::Cancelled => "⊘",
        }
    }

    #[allow(dead_code)]
    pub fn label(&self) -> String {
        match self {
            PlanNodeStatus::Pending => t!("plan_status.pending").to_string(),
            PlanNodeStatus::Running => t!("plan_status.running").to_string(),
            PlanNodeStatus::Completed => t!("plan_status.completed").to_string(),
            PlanNodeStatus::Review => t!("plan_status.review").to_string(),
            PlanNodeStatus::Approved => t!("plan_status.approved").to_string(),
            PlanNodeStatus::Revising => t!("plan_status.revising").to_string(),
            PlanNodeStatus::Failed => t!("plan_status.failed").to_string(),
            PlanNodeStatus::Blocked => t!("plan_status.blocked").to_string(),
            PlanNodeStatus::Cancelled => t!("plan_status.cancelled").to_string(),
        }
    }
}
