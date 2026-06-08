mod agent_team;
mod conversation;
mod events;
mod git;
mod helpers;
mod interaction;
mod plans;
mod status_bar;
mod top_bar;

use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseEvent,
    MouseEventKind,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use futures::StreamExt;
use protocol_interface::core::{CoreEventKind, CoreEventPayload};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph};
use runtime_model::ChatEvent;
use rust_i18n::t;

use crate::repl::{completion, init_required_message, missing_runtime_requirements, ReplSession};
use crate::tui::state::{
    AgentHeader, AgentTeamState, ConversationBlock, InteractionMode, MainTab, PlanNode,
    PlanNodeStatus, WorkspaceStatus,
};
use crate::tui::TuiApp;

use events::{CommandOutcome, ExecutionMode, WorkspaceAction};
use helpers::sanitize_for_tui;
use interaction::{DecisionState, InputBuffer, InteractionState, InteractionUi};

const WORKSPACE_POLL_MS: u64 = 100;
const GIT_REFRESH_SECS: u64 = 2;

pub async fn run_workspace(mut session: ReplSession, initial_missing: Vec<String>) -> Result<()> {
    let mut app = TuiApp::enter()?;
    let result = run_loop(&mut app, &mut session, initial_missing).await;
    app.restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    fn type_text(state: &mut WorkspaceState, value: &str) {
        for ch in value.chars() {
            state.input.insert(ch);
        }
    }

    #[test]
    fn tab_completes_slash_command_without_changing_focus() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/he");

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "/help");
        assert_eq!(state.focus_zone, FocusZone::Input);
    }

    #[test]
    fn tab_keeps_focus_for_ambiguous_command_hint() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/m");

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "/m");
        assert_eq!(state.focus_zone, FocusZone::Input);
    }

    #[test]
    fn tab_cycles_focus_for_non_command_input() {
        let mut state = WorkspaceState::new(Vec::new());

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.focus_zone, FocusZone::Conversation);
    }

    #[test]
    fn command_hint_lists_matching_commands() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/he");

        let hint = state.command_hint(&[]).unwrap();

        assert!(hint.contains("/help"));
    }
}

async fn run_loop(
    app: &mut TuiApp,
    session: &mut ReplSession,
    initial_missing: Vec<String>,
) -> Result<()> {
    let mut state = WorkspaceState::new(initial_missing);

    loop {
        if state.last_workspace_refresh.elapsed() >= Duration::from_secs(GIT_REFRESH_SECS) {
            state.workspace_status = WorkspaceStatus::load();
            state.last_workspace_refresh = Instant::now();
        }

        let header = AgentHeader::standalone(session.soul());
        let model = session.model();
        app.draw(|frame| state.render(frame, &header, &model, &session.models))?;

        if !event::poll(Duration::from_millis(WORKSPACE_POLL_MS))? {
            continue;
        }

        match event::read()? {
            // Shift: temporarily release mouse capture so the terminal can
            // handle native text selection (select + copy) in the conversation
            // and plans panels.
            Event::Key(KeyEvent {
                code: KeyCode::Modifier(ModifierKeyCode::LeftShift | ModifierKeyCode::RightShift),
                kind,
                ..
            }) => match kind {
                KeyEventKind::Press if !state.shift_held => {
                    execute!(std::io::stdout(), DisableMouseCapture)?;
                    state.shift_held = true;
                }
                KeyEventKind::Release if state.shift_held => {
                    execute!(std::io::stdout(), EnableMouseCapture)?;
                    state.shift_held = false;
                }
                _ => {}
            },
            Event::Key(key) if key.kind == KeyEventKind::Press => match state
                .handle_key(key, &session.models)
            {
                WorkspaceAction::None => {}
                WorkspaceAction::Quit => break,
                WorkspaceAction::Submit(input) => {
                    handle_submit(app, session, &mut state, input).await?;
                    if state.quit_requested {
                        break;
                    }
                }
                WorkspaceAction::ApprovePlan => {
                    let prompt = state.pending_goal.clone().unwrap_or_default();
                    if prompt.trim().is_empty() {
                        state.push_block(ConversationBlock::error(t!("workspace.no_pending_plan")));
                        state.restore_text_input();
                    } else {
                        execute_goal(app, session, &mut state, prompt, ExecutionMode::Plan).await?;
                    }
                }
                WorkspaceAction::ExecuteSelectedNodes => {
                    let prompt = state.pending_goal.clone().unwrap_or_default();
                    if prompt.trim().is_empty() {
                        state.push_block(ConversationBlock::error(t!(
                            "workspace.no_selected_nodes"
                        )));
                        state.restore_text_input();
                    } else {
                        state.push_block(ConversationBlock::decision(t!(
                            "workspace.executing_selected_nodes"
                        )));
                        execute_goal(app, session, &mut state, prompt, ExecutionMode::Plan).await?;
                    }
                }
                WorkspaceAction::RevisePlan(custom) => {
                    if custom.trim().is_empty() {
                        state.push_block(ConversationBlock::decision(t!(
                            "workspace.plan_revision_requested"
                        )));
                    } else {
                        state.push_block(ConversationBlock::decision(t!(
                            "workspace.plan_revision_requested_with_text",
                            text = custom.trim()
                        )));
                    }
                    state.mark_revising();
                    state.restore_text_input();
                }
                WorkspaceAction::CancelDecision => {
                    state.push_block(ConversationBlock::decision(t!(
                        "workspace.decision_cancelled"
                    )));
                    state.restore_text_input();
                }
                WorkspaceAction::ApproveReview => {
                    state.approve_review();
                    state.push_block(ConversationBlock::result(t!("workspace.review_approved")));
                    state.restore_text_input();
                }
                WorkspaceAction::RequestRevision(custom) => {
                    let text = if custom.trim().is_empty() {
                        t!("workspace.revision_requested_node").to_string()
                    } else {
                        t!("workspace.revision_requested", text = custom.trim()).to_string()
                    };
                    state.push_block(ConversationBlock::decision(text));
                    state.mark_revising();
                    state.restore_text_input();
                }
                WorkspaceAction::ViewEvidence => {
                    let evidence = state.evidence_text();
                    state.push_block(ConversationBlock::result(evidence));
                }
                WorkspaceAction::RerunNode => {
                    let prompt = state.pending_goal.clone().unwrap_or_default();
                    if prompt.trim().is_empty() {
                        state.push_block(ConversationBlock::error(t!(
                            "workspace.no_completed_node_rerun"
                        )));
                        state.restore_text_input();
                    } else {
                        execute_goal(app, session, &mut state, prompt, ExecutionMode::Plan).await?;
                    }
                }
                WorkspaceAction::InitReconfigure => {
                    if core_runtime::config::project_config_exists() {
                        let confirmed = app.suspend_for(|| -> anyhow::Result<bool> {
                            println!("{}", t!("init.exists_warning"));
                            println!("{}", t!("init.confirm_reset"));
                            let mut answer = String::new();
                            std::io::stdin().read_line(&mut answer)?;
                            Ok(answer.trim().eq_ignore_ascii_case("y"))
                        })?;
                        if !confirmed {
                            state.push_block(ConversationBlock::result(t!("init.cancelled")));
                            state.restore_text_input();
                            continue;
                        }
                    }

                    let locale = session.settings.read().unwrap().ui.locale.clone();
                    core_runtime::config::reset_project_config(Some(&locale))?;

                    let result =
                        app.suspend_for(crate::tui::init_wizard::run_init_wizard_standalone)?;
                    match result {
                        Some(new_settings) => {
                            *session.settings.write().unwrap() = new_settings;
                            session.rebuild_client();
                            state.push_block(ConversationBlock::result(t!("init.saved")));
                        }
                        None => {
                            state.push_block(ConversationBlock::result(t!("init.cancelled")));
                        }
                    }
                    state.restore_text_input();
                }
            },
            Event::Mouse(mouse) => {
                state.handle_mouse(mouse);
            }
            _ => {}
        }
    }

    Ok(())
}

async fn handle_submit(
    app: &mut TuiApp,
    session: &mut ReplSession,
    state: &mut WorkspaceState,
    input: String,
) -> Result<()> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    if trimmed.starts_with('/') {
        let outcome = run_workspace_command(app, session, trimmed).await?;
        if outcome.show_init_menu {
            state.interaction = InteractionUi::Decision(DecisionState::init_menu());
            return Ok(());
        }
        if outcome.clear_blocks {
            state.blocks.clear();
            state.plans.clear();
            state.pending_goal = None;
            if state.focus_zone == FocusZone::Plans {
                state.focus_zone = FocusZone::Input;
            }
        }
        if !outcome.output.trim().is_empty() {
            state.push_block(ConversationBlock::result(sanitize_for_tui(&outcome.output)));
        }
        if outcome.quit {
            state.quit_requested = true;
        }
        state.workspace_status = WorkspaceStatus::load();
        state.last_workspace_refresh = Instant::now();
        if state.quit_requested {
            app.draw(|frame| {
                let header = AgentHeader::standalone(session.soul());
                let model = session.model();
                state.render(frame, &header, &model, &session.models);
            })?;
        }
        return Ok(());
    }

    match state.mode {
        InteractionMode::Plan => {
            state.create_plan(trimmed);
        }
        InteractionMode::Bypass => {
            execute_goal(
                app,
                session,
                state,
                trimmed.to_string(),
                ExecutionMode::Bypass,
            )
            .await?;
        }
    }

    Ok(())
}

async fn execute_goal(
    app: &mut TuiApp,
    session: &mut ReplSession,
    state: &mut WorkspaceState,
    prompt: String,
    mode: ExecutionMode,
) -> Result<()> {
    state.pending_goal = Some(prompt.clone());
    state.start_execution(mode);

    app.draw(|frame| {
        let header = AgentHeader::standalone(session.soul());
        let model = session.model();
        state.render(frame, &header, &model, &session.models);
    })?;

    match collect_model_response(session, &prompt).await {
        Ok(result) => {
            if result.has_text() {
                state.finish_execution(&result.text, mode);
            } else if result.has_errors() {
                // Errors without response text: restore text input
                for node in &mut state.plans {
                    if node.status == PlanNodeStatus::Running {
                        node.status = PlanNodeStatus::Failed;
                    }
                }
                state.restore_text_input();
            }
            for err_msg in &result.errors {
                state.push_block(ConversationBlock::error(sanitize_for_tui(err_msg)));
            }
            // If we got neither text nor errors, treat it as a silent response
            if !result.has_text() && !result.has_errors() {
                state.restore_text_input();
                state.workspace_status = WorkspaceStatus::load();
                state.last_workspace_refresh = Instant::now();
                return Ok(());
            }
            if session.auto_review && result.has_text() {
                match session.cmd_review(vec!["/review"]).await {
                    Ok(review) if !review.trim().is_empty() => {
                        state.push_block(ConversationBlock::decision(format!(
                            "{}\n{}",
                            t!("workspace.auto_review"),
                            sanitize_for_tui(&review)
                        )));
                    }
                    Err(err) => state.push_block(ConversationBlock::error(t!(
                        "workspace.auto_review_failed",
                        error = err.to_string()
                    ))),
                    _ => {}
                }
            }
        }
        Err(err) => {
            state.fail_execution(&err.to_string());
        }
    }

    state.workspace_status = WorkspaceStatus::load();
    state.last_workspace_refresh = Instant::now();
    Ok(())
}

/// Result of a model response collection, including both the response
/// text and any error/status messages that occurred during the run.
struct ModelResponse {
    text: String,
    errors: Vec<String>,
}

impl ModelResponse {
    fn has_text(&self) -> bool {
        !self.text.is_empty()
    }

    fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }
}

async fn collect_model_response(session: &mut ReplSession, input: &str) -> Result<ModelResponse> {
    let missing = missing_runtime_requirements(&session.settings.read().unwrap());
    if !missing.is_empty() {
        return Err(anyhow::anyhow!("{}", init_required_message(&missing)));
    }

    // Prefer ProtocolBridge path
    if let Some(bridge) = &session.bridge {
        session.conversation.add_user_message(input.to_string());

        // Use send_message_with_mode to get all events, then process them
        // to capture both ModelDelta text and ErrorRaised status messages.
        let events =
            bridge.send_message_with_mode(input, protocol_interface::core::RuntimeMode::Chat)?;
        let mut full_response = String::new();
        let mut errors: Vec<String> = Vec::new();

        for envelope in &events {
            match (&envelope.payload.kind, &envelope.payload.payload) {
                (CoreEventKind::ModelDelta, CoreEventPayload::Text { text }) => {
                    full_response.push_str(text);
                }
                (CoreEventKind::ErrorRaised, CoreEventPayload::Error { message, .. }) => {
                    errors.push(message.clone());
                }
                (
                    CoreEventKind::FinalResult,
                    CoreEventPayload::Final {
                        success: false,
                        content,
                    },
                ) => {
                    let msg = if content.is_empty() {
                        t!("workspace.run_failed").to_string()
                    } else {
                        content.clone()
                    };
                    errors.push(msg);
                }
                _ => {}
            }
        }

        if !full_response.is_empty() {
            session
                .conversation
                .add_assistant_message(full_response.clone());
        }
        session.conversation_store.save_messages(
            &session.session_metadata.id,
            session.conversation.messages(),
        )?;
        let _ = session.session_store.update(&mut session.session_metadata);
        return Ok(ModelResponse {
            text: full_response,
            errors,
        });
    }

    // Fallback: direct path when bridge is unavailable (degraded mode)
    let client = session
        .client
        .clone()
        .ok_or_else(|| anyhow::anyhow!("No LLM client configured. Run /init to set up."))?;

    session.conversation.add_user_message(input.to_string());
    let stream = client.chat_stream(&session.conversation).await?;
    let mut stream = Box::pin(stream);
    let mut full_response = String::new();
    let mut errors: Vec<String> = Vec::new();

    while let Some(event) = stream.next().await {
        match event? {
            ChatEvent::Delta { text } => {
                full_response.push_str(&text);
            }
            ChatEvent::Done {
                full_response: done_text,
            } => {
                if !done_text.is_empty() && done_text.len() >= full_response.len() {
                    full_response = done_text;
                }
                break;
            }
            ChatEvent::Error { message } => {
                // Capture error but don't lose the text accumulated so far
                errors.push(message);
                break;
            }
        }
    }

    if !full_response.is_empty() {
        session
            .conversation
            .add_assistant_message(full_response.clone());
    }

    session.conversation_store.save_messages(
        &session.session_metadata.id,
        session.conversation.messages(),
    )?;
    let _ = session.session_store.update(&mut session.session_metadata);

    Ok(ModelResponse {
        text: full_response,
        errors,
    })
}

async fn run_workspace_command(
    app: &mut TuiApp,
    session: &mut ReplSession,
    input: &str,
) -> Result<CommandOutcome> {
    let parts: Vec<&str> = input.split_whitespace().collect();
    let cmd = parts.first().copied().unwrap_or("");

    let outcome = match cmd {
        "/quit" | "/exit" => CommandOutcome {
            output: t!("workspace.closed").to_string(),
            quit: true,
            clear_blocks: false,
            show_init_menu: false,
        },
        "/clear" => {
            session.conversation.clear();
            CommandOutcome {
                output: t!("clear.conversation").to_string(),
                quit: false,
                clear_blocks: true,
                show_init_menu: false,
            }
        }
        "/help" => CommandOutcome::output(workspace_help()),
        "/history" => CommandOutcome::output(format_history(session)),
        "/init" => CommandOutcome {
            output: String::new(),
            quit: false,
            clear_blocks: false,
            show_init_menu: true,
        },
        "/model" if parts.len() == 1 => {
            let current = session.settings.read().unwrap().llm.model.clone();
            let client = session.client.clone();
            let result = app.suspend_for(|| {
                tokio::runtime::Runtime::new().unwrap().block_on(
                    crate::tui::model_select::select_model(client.as_deref(), &current),
                )
            })?;
            match result {
                Some(new_model) => {
                    session.settings.write().unwrap().llm.model = new_model.clone();
                    session.rebuild_client();
                    CommandOutcome::output(
                        t!("model_select.switched", model = new_model).to_string(),
                    )
                }
                None => CommandOutcome::output(t!("model_select.cancelled").to_string()),
            }
        }
        "/config" if parts.get(1) != Some(&"show") => {
            let current_settings = session.settings.read().unwrap().clone();
            let result =
                app.suspend_for(|| crate::tui::config_panel::run_config_panel(current_settings))?;
            match result {
                Some(new_settings) => {
                    *session.settings.write().unwrap() = new_settings;
                    session.rebuild_client();
                    CommandOutcome::output(t!("config.saved").to_string())
                }
                None => CommandOutcome::output(t!("config.cancelled").to_string()),
            }
        }
        _ => CommandOutcome::output(session.handle_command(input).await?),
    };

    Ok(outcome)
}

fn workspace_help() -> String {
    [
        t!("workspace.help.title").to_string(),
        format!("  /help              {}", t!("workspace.help.help")),
        format!("  /clear             {}", t!("workspace.help.clear")),
        format!("  /history           {}", t!("workspace.help.history")),
        format!("  /config show       {}", t!("workspace.help.config_show")),
        format!("  /model <name>      {}", t!("workspace.help.model")),
        format!(
            "  /session current   {}",
            t!("workspace.help.session_current")
        ),
        format!("  /tools             {}", t!("workspace.help.tools")),
        format!("  /doctor            {}", t!("workspace.help.doctor")),
        format!("  /quit              {}", t!("workspace.help.quit")),
        String::new(),
        t!("workspace.help.keys_title").to_string(),
        t!("workspace.help.key_switch_tab").to_string(),
        t!("workspace.help.key_switch_mode").to_string(),
        t!("workspace.help.key_submit").to_string(),
        t!("workspace.help.key_cancel").to_string(),
        t!("workspace.help.key_move").to_string(),
        t!("workspace.help.key_confirm").to_string(),
    ]
    .join("\n")
}

fn format_history(session: &ReplSession) -> String {
    let messages = session.conversation.messages();
    if messages.is_empty() {
        return t!("workspace.history.none").to_string();
    }

    let mut out = format!("{}\n", t!("workspace.history.title"));
    for (index, message) in messages.iter().enumerate() {
        let label = match message.role {
            protocol_interface::MessageRole::System => t!("workspace.history.runtime"),
            protocol_interface::MessageRole::User => t!("workspace.history.request"),
            protocol_interface::MessageRole::Assistant => t!("workspace.history.result"),
            protocol_interface::MessageRole::Summary => t!("workspace.history.summary"),
        };
        let preview = helpers::truncate_chars(message.content.trim(), 96);
        out.push_str(&format!("  {:3}. {:<8} {}\n", index + 1, label, preview));
    }
    out.trim_end().to_string()
}

// ---------------------------------------------------------------------------
// FocusZone, HoverZone, PanelScroll, LayoutRects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FocusZone {
    Input,
    Conversation,
    Plans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HoverZone {
    Conversation,
    Plans,
    Interaction,
}

#[derive(Debug, Clone)]
struct PanelScroll {
    offset: u16,
    auto_scroll: bool,
}

impl PanelScroll {
    fn new() -> Self {
        Self {
            offset: 0,
            auto_scroll: true,
        }
    }

    fn scroll_up(&mut self, n: u16) {
        self.offset = self.offset.saturating_sub(n);
        self.auto_scroll = false;
    }

    fn scroll_to(&mut self, pos: u16) {
        self.offset = pos;
        self.auto_scroll = false;
    }

    fn snap_to_bottom(&mut self, max_offset: u16) {
        if self.auto_scroll {
            self.offset = max_offset;
        }
    }

    fn clamp(&mut self, max_offset: u16) {
        self.offset = self.offset.min(max_offset);
    }
}

#[derive(Debug, Clone, Default)]
struct LayoutRects {
    conversation: Rect,
    agent_team: Rect,
    plans: Rect,
    interaction: Rect,
}

// ---------------------------------------------------------------------------
// WorkspaceState
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct WorkspaceState {
    mode: InteractionMode,
    active_tab: MainTab,
    blocks: Vec<ConversationBlock>,
    plans: Vec<PlanNode>,
    agent_team: Option<AgentTeamState>,
    input: InputBuffer,
    interaction: InteractionUi,
    pending_goal: Option<String>,
    workspace_status: WorkspaceStatus,
    last_workspace_refresh: Instant,
    started_at: Instant,
    quit_requested: bool,
    focus_zone: FocusZone,
    hover_zone: Option<HoverZone>,
    shift_held: bool,
    conv_scroll: PanelScroll,
    plans_scroll: PanelScroll,
    agent_team_scroll: PanelScroll,
    layout_rects: LayoutRects,
}

impl WorkspaceState {
    fn new(initial_missing: Vec<String>) -> Self {
        let mut blocks = vec![ConversationBlock::result(t!("workspace.ready"))];

        if !initial_missing.is_empty() {
            blocks.push(ConversationBlock::error(sanitize_for_tui(
                &init_required_message(&initial_missing),
            )));
        }

        Self {
            mode: InteractionMode::Plan,
            active_tab: MainTab::Conversation,
            blocks,
            plans: Vec::new(),
            agent_team: None,
            input: InputBuffer::default(),
            interaction: InteractionUi::TextInput,
            pending_goal: None,
            workspace_status: WorkspaceStatus::load(),
            last_workspace_refresh: Instant::now(),
            started_at: Instant::now(),
            quit_requested: false,
            focus_zone: FocusZone::Input,
            hover_zone: None,
            shift_held: false,
            conv_scroll: PanelScroll::new(),
            plans_scroll: PanelScroll::new(),
            agent_team_scroll: PanelScroll::new(),
            layout_rects: LayoutRects::default(),
        }
    }

    fn render(&mut self, frame: &mut Frame, header: &AgentHeader, model: &str, models: &[String]) {
        let area = frame.area();
        let outer = Block::default().borders(Borders::ALL);
        let inner = outer.inner(area);
        frame.render_widget(outer, area);

        if inner.height < 8 || inner.width < 40 {
            frame.render_widget(
                Paragraph::new(t!("workspace.terminal_too_small").to_string())
                    .alignment(Alignment::Center),
                inner,
            );
            return;
        }

        let interaction_height = self.interaction.height(inner.height);
        let layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Min(6),
                Constraint::Length(interaction_height),
                Constraint::Length(1),
            ])
            .split(inner);

        self.layout_rects.interaction = layout[2];
        top_bar::render(frame, layout[0], header, self.started_at.elapsed());
        self.render_main(frame, layout[1], model);
        interaction::render_interaction(
            frame,
            layout[2],
            &self.interaction,
            &InteractionState {
                mode: self.mode,
                active_tab: self.active_tab,
                input: &self.input,
                has_agent_team: self.has_agent_team_tab(),
                command_hint: self.command_hint(models),
            },
            self.focus_zone == FocusZone::Input,
            self.hover_zone == Some(HoverZone::Interaction),
        );
        status_bar::render(frame, layout[3], &self.workspace_status);
    }

    fn render_main(&mut self, frame: &mut Frame, area: Rect, model: &str) {
        let has_plans = !self.plans.is_empty();
        let constraints = if has_plans {
            [Constraint::Percentage(68), Constraint::Percentage(32)]
        } else {
            [Constraint::Percentage(100), Constraint::Percentage(0)]
        };
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        self.layout_rects.plans = if has_plans {
            chunks[1]
        } else {
            Rect::default()
        };
        let conv_focused = self.focus_zone == FocusZone::Conversation;
        let conv_hovered = self.hover_zone == Some(HoverZone::Conversation);

        let tab_title = conversation::tab_title(self.active_tab, self.has_agent_team_tab());
        match self.active_tab {
            MainTab::Conversation => {
                self.layout_rects.conversation = chunks[0];
                conversation::render(
                    frame,
                    chunks[0],
                    &self.blocks,
                    model,
                    &tab_title,
                    &mut self.conv_scroll,
                    conv_focused,
                    conv_hovered,
                )
            }
            MainTab::AgentTeam => {
                self.layout_rects.agent_team = chunks[0];
                agent_team::render(
                    frame,
                    chunks[0],
                    self.agent_team.as_ref(),
                    &tab_title,
                    &mut self.agent_team_scroll,
                    conv_focused,
                    conv_hovered,
                )
            }
        }
        if has_plans {
            plans::render(
                frame,
                chunks[1],
                &self.plans,
                self.has_agent_team_tab(),
                self.agent_team.as_ref(),
                &mut self.plans_scroll,
                self.focus_zone == FocusZone::Plans,
                self.hover_zone == Some(HoverZone::Plans),
            );
        }
    }

    fn has_agent_team_tab(&self) -> bool {
        self.agent_team.is_some()
    }

    fn handle_key(&mut self, key: KeyEvent, models: &[String]) -> WorkspaceAction {
        if self.quit_requested {
            return WorkspaceAction::Quit;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            return WorkspaceAction::Quit;
        }

        if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_tab();
            return WorkspaceAction::None;
        }

        if key.code == KeyCode::BackTab {
            if matches!(self.interaction, InteractionUi::TextInput) {
                self.toggle_mode();
            }
            return WorkspaceAction::None;
        }

        if key.code == KeyCode::Tab
            && key.modifiers.is_empty()
            && self.focus_zone == FocusZone::Input
            && matches!(self.interaction, InteractionUi::TextInput)
            && self.complete_command(models)
        {
            return WorkspaceAction::None;
        }

        // Tab: cycle focus zones (TextInput mode only)
        if key.code == KeyCode::Tab
            && key.modifiers.is_empty()
            && matches!(self.interaction, InteractionUi::TextInput)
        {
            self.focus_zone = match self.focus_zone {
                FocusZone::Input => FocusZone::Conversation,
                FocusZone::Conversation => {
                    if self.plans.is_empty() {
                        FocusZone::Input
                    } else {
                        FocusZone::Plans
                    }
                }
                FocusZone::Plans => FocusZone::Input,
            };
            return WorkspaceAction::None;
        }

        // Keyboard scrolling when a scrollable panel is focused
        if matches!(self.interaction, InteractionUi::TextInput) && self.handle_scroll_keys(key) {
            return WorkspaceAction::None;
        }

        match &mut self.interaction {
            InteractionUi::TextInput => self.handle_text_input_key(key),
            InteractionUi::Decision(decision) => decision.handle_key(key),
        }
    }

    fn handle_scroll_keys(&mut self, key: KeyEvent) -> bool {
        let scroll = match self.focus_zone {
            FocusZone::Conversation => match self.active_tab {
                MainTab::Conversation => &mut self.conv_scroll,
                MainTab::AgentTeam => &mut self.agent_team_scroll,
            },
            FocusZone::Plans => &mut self.plans_scroll,
            FocusZone::Input => return false,
        };

        match key.code {
            KeyCode::Up => {
                scroll.scroll_up(1);
                true
            }
            KeyCode::Down => {
                scroll.offset = scroll.offset.saturating_add(1);
                scroll.auto_scroll = false;
                true
            }
            KeyCode::PageUp => {
                scroll.scroll_up(10);
                true
            }
            KeyCode::PageDown => {
                scroll.offset = scroll.offset.saturating_add(10);
                scroll.auto_scroll = false;
                true
            }
            KeyCode::Home => {
                scroll.scroll_to(0);
                true
            }
            KeyCode::End => {
                scroll.auto_scroll = true;
                true
            }
            _ => false,
        }
    }

    fn handle_mouse(&mut self, mouse: MouseEvent) {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.scroll_for_position(mouse.column, mouse.row)
                    .scroll_up(3);
            }
            MouseEventKind::ScrollDown => {
                let scroll = self.scroll_for_position(mouse.column, mouse.row);
                scroll.offset = scroll.offset.saturating_add(3);
                scroll.auto_scroll = false;
            }
            MouseEventKind::Moved => {
                self.hover_zone = self.zone_for_position(mouse.column, mouse.row);
            }
            _ => {}
        }
    }

    fn scroll_for_position(&mut self, col: u16, row: u16) -> &mut PanelScroll {
        let pos = Position { x: col, y: row };
        if self.layout_rects.plans.contains(pos) {
            &mut self.plans_scroll
        } else if self.layout_rects.conversation.contains(pos) {
            &mut self.conv_scroll
        } else if self.layout_rects.agent_team.contains(pos) {
            &mut self.agent_team_scroll
        } else {
            match self.active_tab {
                MainTab::Conversation => &mut self.conv_scroll,
                MainTab::AgentTeam => &mut self.agent_team_scroll,
            }
        }
    }

    fn zone_for_position(&self, col: u16, row: u16) -> Option<HoverZone> {
        let pos = Position { x: col, y: row };
        if self.layout_rects.conversation.contains(pos)
            || self.layout_rects.agent_team.contains(pos)
        {
            Some(HoverZone::Conversation)
        } else if self.layout_rects.plans.contains(pos) {
            Some(HoverZone::Plans)
        } else if self.layout_rects.interaction.contains(pos) {
            Some(HoverZone::Interaction)
        } else {
            None
        }
    }

    fn handle_text_input_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::CONTROL) || !self.input.is_empty() {
                    let input = self.input.take();
                    WorkspaceAction::Submit(input)
                } else {
                    WorkspaceAction::None
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                WorkspaceAction::None
            }
            _ => {
                self.input.handle_key(key);
                WorkspaceAction::None
            }
        }
    }

    fn complete_command(&mut self, models: &[String]) -> bool {
        let Some(result) = completion::complete(self.input.value(), self.input.cursor(), models)
        else {
            return false;
        };

        let current = self
            .input
            .value()
            .chars()
            .skip(result.start)
            .take(result.end.saturating_sub(result.start))
            .collect::<String>();

        let replacement = if result.matches.len() == 1 {
            result.matches[0].replacement.clone()
        } else if let Some(prefix) = completion::common_prefix(&result.matches) {
            if prefix.chars().count() <= current.chars().count() {
                return true;
            }
            prefix
        } else {
            return true;
        };

        self.input
            .replace_range(result.start, result.end, &replacement);
        true
    }

    fn command_hint(&self, models: &[String]) -> Option<String> {
        if !matches!(self.interaction, InteractionUi::TextInput)
            || !self.input.value().trim_start().starts_with('/')
        {
            return None;
        }

        let Some(result) = completion::complete(self.input.value(), self.input.cursor(), models)
        else {
            return Some(t!("workspace.command_hint.none").to_string());
        };

        let visible = 5;
        let mut choices = result
            .matches
            .iter()
            .take(visible)
            .map(|item| item.display.as_str())
            .collect::<Vec<_>>()
            .join("  ");
        let remaining = result.matches.len().saturating_sub(visible);
        if remaining > 0 {
            choices.push_str("  ");
            choices.push_str(t!("workspace.command_hint.more", count = remaining).as_ref());
        }

        Some(format!(
            "{} · {}",
            t!("workspace.command_hint.complete"),
            choices
        ))
    }

    fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            InteractionMode::Plan => InteractionMode::Bypass,
            InteractionMode::Bypass => InteractionMode::Plan,
        };
        // 不再清空输入框 — 模式切换时保留用户输入内容
    }

    fn toggle_tab(&mut self) {
        if !self.has_agent_team_tab() {
            return;
        }

        self.active_tab = match self.active_tab {
            MainTab::Conversation => MainTab::AgentTeam,
            MainTab::AgentTeam => MainTab::Conversation,
        };
        // 不再清空输入框 — Tab 切换时保留用户输入内容
    }

    fn create_plan(&mut self, goal: &str) {
        self.pending_goal = Some(goal.to_string());
        self.push_block(ConversationBlock::request(goal));
        self.push_block(ConversationBlock::understanding(t!(
            "workspace.create_plan_understanding",
            goal = goal
        )));
        self.plans = vec![
            PlanNode::new(
                "understand",
                t!("workspace.plan_node.understand"),
                PlanNodeStatus::Completed,
            )
            .with_owner("local"),
            PlanNode::new(
                "decompose",
                t!("workspace.plan_node.decompose"),
                PlanNodeStatus::Completed,
            )
            .with_owner("local"),
            PlanNode::new(
                "execute",
                t!("workspace.plan_node.execute_approved"),
                PlanNodeStatus::Pending,
            )
            .with_owner("local"),
            PlanNode::new(
                "review",
                t!("workspace.plan_node.review_completed"),
                PlanNodeStatus::Pending,
            )
            .with_owner("local"),
            PlanNode::new(
                "finalize",
                t!("workspace.plan_node.finalize"),
                PlanNodeStatus::Pending,
            )
            .with_owner("local"),
        ];
        self.push_block(ConversationBlock::plan_proposal(t!(
            "workspace.plan_proposal"
        )));
        self.interaction = InteractionUi::Decision(DecisionState::plan_approval());
    }

    fn start_execution(&mut self, mode: ExecutionMode) {
        self.push_block(ConversationBlock::execution(match mode {
            ExecutionMode::Plan => t!("workspace.executing_plan"),
            ExecutionMode::Bypass => t!("workspace.executing_direct"),
        }));

        if mode == ExecutionMode::Plan {
            if self.plans.is_empty() {
                self.plans = vec![
                    PlanNode::new(
                        "understand",
                        t!("workspace.plan_node.understand"),
                        PlanNodeStatus::Completed,
                    )
                    .with_owner("local"),
                    PlanNode::new(
                        "execute",
                        t!("workspace.plan_node.execute_instruction"),
                        PlanNodeStatus::Running,
                    )
                    .with_owner("local"),
                    PlanNode::new(
                        "finalize",
                        t!("workspace.plan_node.finalize"),
                        PlanNodeStatus::Pending,
                    )
                    .with_owner("local"),
                ];
            } else if let Some(node) = self.plans.get_mut(2) {
                node.status = PlanNodeStatus::Running;
            }
        } else {
            self.plans.clear();
            if self.focus_zone == FocusZone::Plans {
                self.focus_zone = FocusZone::Input;
            }
        }

        self.interaction = InteractionUi::TextInput;
    }

    fn finish_execution(&mut self, response: &str, mode: ExecutionMode) {
        self.push_block(ConversationBlock::result(sanitize_for_tui(response)));

        if mode == ExecutionMode::Plan {
            if let Some(node) = self.plans.get_mut(2) {
                node.status = PlanNodeStatus::Completed;
                node.evidence = vec![t!("workspace.model_response_evidence").to_string()];
            }
            if let Some(node) = self.plans.get_mut(3) {
                node.status = PlanNodeStatus::Review;
            }
            self.interaction = InteractionUi::Decision(DecisionState::node_review());
        } else {
            self.restore_text_input();
        }
    }

    fn fail_execution(&mut self, message: &str) {
        self.push_block(ConversationBlock::error(sanitize_for_tui(message)));
        for node in &mut self.plans {
            if node.status == PlanNodeStatus::Running {
                node.status = PlanNodeStatus::Failed;
            }
        }
        self.restore_text_input();
    }

    fn approve_review(&mut self) {
        if let Some(node) = self.plans.get_mut(2) {
            if node.status == PlanNodeStatus::Completed {
                node.status = PlanNodeStatus::Approved;
            }
        }
        if let Some(node) = self.plans.get_mut(3) {
            node.status = PlanNodeStatus::Approved;
        }
        if let Some(node) = self.plans.get_mut(4) {
            node.status = PlanNodeStatus::Completed;
        }
    }

    fn mark_revising(&mut self) {
        if let Some(node) = self.plans.iter_mut().find(|node| {
            matches!(
                node.status,
                PlanNodeStatus::Completed | PlanNodeStatus::Review
            )
        }) {
            node.status = PlanNodeStatus::Revising;
        }
    }

    fn evidence_text(&self) -> String {
        let mut evidence = Vec::new();
        for node in &self.plans {
            for item in &node.evidence {
                evidence.push(format!("{}: {}", node.title, item));
            }
        }
        if evidence.is_empty() {
            t!("workspace.no_evidence").to_string()
        } else {
            evidence.join("\n")
        }
    }

    fn restore_text_input(&mut self) {
        self.interaction = InteractionUi::TextInput;
        self.input.clear();
    }

    fn push_block(&mut self, block: ConversationBlock) {
        self.blocks.push(block);
        let excess = self.blocks.len().saturating_sub(32);
        if excess > 0 {
            self.blocks.drain(0..excess);
        }
    }
}
