use std::time::Duration;

use ratatui::prelude::*;
use ratatui::widgets::Paragraph;
use rust_i18n::t;

use super::helpers::fit_left_right;
use crate::tui::state::{AgentHeader, AgentNetworkStatus};
use crate::tui::theme;

pub fn render(frame: &mut Frame, area: Rect, header: &AgentHeader, elapsed: Duration) {
    let left = format!(
        "Alius v{}  {}",
        header.version,
        t!("workspace.header.soul", soul = &header.soul)
    );
    let right = right_text(header, elapsed);
    let text = fit_left_right(&left, &right, area.width as usize);
    frame.render_widget(Paragraph::new(text).style(theme::title()), area);
}

fn right_text(header: &AgentHeader, elapsed: Duration) -> String {
    match header.network_status {
        AgentNetworkStatus::Standalone => t!("workspace.network.standalone").to_string(),
        AgentNetworkStatus::AgentNetConnected => {
            if elapsed.as_secs() % 7 >= 5 {
                header
                    .node_id
                    .as_ref()
                    .map(|id| t!("workspace.network.node", id = id).to_string())
                    .unwrap_or_else(|| t!("workspace.network.connected").to_string())
            } else {
                t!("workspace.network.connected").to_string()
            }
        }
        AgentNetworkStatus::AgentNetSyncing => t!("workspace.network.syncing").to_string(),
        AgentNetworkStatus::AgentNetDegraded => t!("workspace.network.degraded").to_string(),
        AgentNetworkStatus::AgentNetOffline => t!("workspace.network.offline").to_string(),
    }
}

impl AgentHeader {
    pub fn standalone(soul: String) -> Self {
        Self {
            version: option_env!("ALIUS_VERSION")
                .unwrap_or(env!("CARGO_PKG_VERSION"))
                .to_string(),
            soul,
            network_status: AgentNetworkStatus::Standalone,
            node_id: None,
        }
    }
}
