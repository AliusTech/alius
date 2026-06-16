mod agent_team;
mod config_task;
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
use protocol_interface::core::{CoreEventKind, CoreEventPayload, RunRef, RuntimeMode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use runtime_config::ModelAssignmentRole;
use rust_i18n::t;

use crate::repl::{completion, init_required_message, missing_runtime_requirements, ReplSession};
use crate::tui::state::{
    AgentHeader, AgentTeamState, ConversationBlock, ConversationBlockType, InteractionMode,
    MainTab, PlanNode, PlanNodeStatus, WorkspaceStatus,
};
use crate::tui::TuiApp;

use config_task::{
    ConfigPrompt, ConfigSaveTarget, ConfigSidePanel, ConfigTask, ConfigTaskKind, ConfigTaskOutcome,
};
use events::{CommandOutcome, DecisionKind, ExecutionMode, WorkspaceAction};
use helpers::{sanitize_for_tui, truncate_chars};
use interaction::{
    DecisionState, InputBuffer, InteractionState, InteractionUi, PromptChoice, PromptInputAction,
    PromptInputKind, PromptInputState,
};

const WORKSPACE_POLL_MS: u64 = 100;
const GIT_REFRESH_SECS: u64 = 2;
const MAX_COLLAPSED_LINES: usize = 3;

pub async fn run_workspace(mut session: ReplSession, initial_missing: Vec<String>) -> Result<()> {
    let mut app = TuiApp::enter()?;
    let result = run_loop(&mut app, &mut session, initial_missing).await;
    app.restore()?;
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    struct CwdGuard(std::path::PathBuf);

    impl Drop for CwdGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }

    fn enter_temp_cwd() -> (TempDir, CwdGuard) {
        let original = std::env::current_dir().unwrap();
        let dir = TempDir::new().unwrap();
        std::env::set_current_dir(dir.path()).unwrap();
        (dir, CwdGuard(original))
    }

    fn type_text(state: &mut WorkspaceState, value: &str) {
        for ch in value.chars() {
            state.input.insert(ch);
        }
    }

    fn submit_input(state: &mut WorkspaceState, value: &str) -> String {
        type_text(state, value);
        match state.handle_key(key(KeyCode::Enter), &[]) {
            WorkspaceAction::Submit(input) => input,
            action => panic!("expected submit action, got {action:?}"),
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
    fn startup_uses_welcome_block_without_init_error_spam() {
        rust_i18n::set_locale("en");
        let state = WorkspaceState::new(vec!["model".to_string(), "soul".to_string()]);

        assert_eq!(state.blocks.len(), 1);
        assert_eq!(
            state.blocks[0].block_type,
            crate::tui::state::ConversationBlockType::Welcome
        );
        assert!(state.blocks[0].content.contains("\"ready\":false"));
        assert!(state.blocks[0].content.contains("\"version\":\"v"));
        assert!(!state.blocks[0].content.contains("Alius is not initialized"));
    }

    #[test]
    fn conversation_updates_stick_scroll_to_latest() {
        let mut state = WorkspaceState::new(Vec::new());

        state.conv_scroll.auto_scroll = false;
        state.push_block(ConversationBlock::result("new output"));
        assert!(state.conv_scroll.auto_scroll);

        state.push_block(ConversationBlock::execution(""));
        state.conv_scroll.auto_scroll = false;
        state.update_streaming_text("delta");
        assert!(state.conv_scroll.auto_scroll);

        state.conv_scroll.auto_scroll = false;
        state.update_streaming_text(" more");
        assert!(state.conv_scroll.auto_scroll);
    }

    #[test]
    fn tool_events_are_visible_in_conversation() {
        rust_i18n::set_locale("en");
        let mut state = WorkspaceState::new(Vec::new());
        let started = serde_json::json!({
            "id": "call-1",
            "name": "shell",
            "args": {
                "command": "git clone https://github.com/lc345/repo.git"
            }
        });
        let completed = serde_json::json!({
            "id": "call-1",
            "name": "shell",
            "args": {
                "command": "git clone https://github.com/lc345/repo.git"
            },
            "success": true,
            "output": "exit=0\nstdout:\ncloned\nstderr:\n"
        });

        state.start_execution(ExecutionMode::Bypass);
        state.conv_scroll.auto_scroll = false;
        state.record_tool_call_started(&started);
        assert!(state.conv_scroll.auto_scroll);
        assert!(state
            .blocks
            .iter()
            .any(|block| block.content.contains("shell: git clone")));

        state.record_tool_call_completed(&completed);
        let block = state.blocks.last().expect("tool completion block");
        assert!(block.content.contains("Tool completed"));
        assert!(block.content.contains("cloned"));
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
    fn tab_in_config_task_switches_config_tabs_not_command_completion() {
        let mut state = WorkspaceState::new(Vec::new());
        state.config_task = Some(ConfigTask::new(runtime_config::Settings::default()));
        state.prompt_input = Some(PromptInputState::new(
            "Provider",
            interaction::PromptInputKind::SingleSelect,
            vec![
                interaction::PromptChoice::new("Help", "/help"),
                interaction::PromptChoice::new("Config", "/config"),
            ],
            "help",
        ));
        type_text(&mut state, "/he");

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "");
        assert_eq!(state.focus_zone, FocusZone::Input);
        let prompt = state.prompt_input.as_ref().unwrap();
        // Initial section is Model Assignment (jump-to-first-missing); Tab forward → Language.
        assert_eq!(prompt.title, "Language");
        assert_eq!(
            prompt.scope_title.as_deref(),
            Some("configuration-language")
        );

        let action = state.handle_key(key(KeyCode::BackTab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.title, "Model Assignment");
        assert_eq!(prompt.scope_title.as_deref(), Some("configuration-models"));
        assert_eq!(state.focus_zone, FocusZone::Input);
    }

    #[test]
    fn submitted_inputs_can_be_recalled_with_up_down() {
        let mut state = WorkspaceState::new(Vec::new());

        assert_eq!(submit_input(&mut state, "first prompt"), "first prompt");
        assert_eq!(submit_input(&mut state, "/config"), "/config");

        assert!(matches!(
            state.handle_key(key(KeyCode::Up), &[]),
            WorkspaceAction::None
        ));
        assert_eq!(state.input.value(), "/config");

        state.handle_key(key(KeyCode::Up), &[]);
        assert_eq!(state.input.value(), "first prompt");

        state.handle_key(key(KeyCode::Down), &[]);
        assert_eq!(state.input.value(), "/config");

        state.handle_key(key(KeyCode::Down), &[]);
        assert_eq!(state.input.value(), "");
    }

    #[test]
    fn input_history_restores_unsent_draft() {
        let mut state = WorkspaceState::new(Vec::new());
        assert_eq!(submit_input(&mut state, "sent prompt"), "sent prompt");
        type_text(&mut state, "draft prompt");

        state.handle_key(key(KeyCode::Up), &[]);
        assert_eq!(state.input.value(), "sent prompt");

        state.handle_key(key(KeyCode::Down), &[]);
        assert_eq!(state.input.value(), "draft prompt");
    }

    #[test]
    fn record_input_request_adds_visible_user_block() {
        let mut state = WorkspaceState::new(Vec::new());

        state.record_input_request("explain this file");

        let block = state.blocks.last().expect("request block");
        assert_eq!(
            block.block_type,
            crate::tui::state::ConversationBlockType::Request
        );
        assert_eq!(block.content, "explain this file");
    }

    #[test]
    fn plan_draft_starts_without_showing_plan_panel() {
        let mut state = WorkspaceState::new(Vec::new());

        state.begin_plan_draft("build a feature");

        assert!(state.plan_draft.is_some());
        assert!(state.plans.is_empty());
        assert_eq!(state.pending_goal.as_deref(), Some("build a feature"));
    }

    #[test]
    fn plan_controller_questions_keep_plan_panel_hidden() {
        let mut state = WorkspaceState::new(Vec::new());
        state.begin_plan_draft("build a feature");
        state.push_block(ConversationBlock::execution(""));

        state.apply_plan_controller_response(
            "ALIUS_NEED_DETAILS\nWhich files should be changed?".to_string(),
        );

        assert!(state.plan_draft.is_some());
        assert!(state.plans.is_empty());
        assert!(matches!(state.interaction, InteractionUi::TextInput));
        assert!(state.blocks.last().unwrap().content.contains("Which files"));
        assert!(matches!(
            state.prompt_input.as_ref().unwrap().kind,
            interaction::PromptInputKind::Text { masked: false }
        ));
    }

    #[test]
    fn plan_controller_question_uses_single_select_prompt_options() {
        let mut state = WorkspaceState::new(Vec::new());
        state.begin_plan_draft("design an api");
        state.push_block(ConversationBlock::execution(""));

        state.apply_plan_controller_response(
            "ALIUS_NEED_DETAILS\nquestion: What is the request method?\ntype: single\nallow_custom: false\noptions:\n- GET\n- POST\n- PUT\n- DELETE".to_string(),
        );

        let prompt = state.prompt_input.as_ref().unwrap();
        assert!(matches!(
            prompt.kind,
            interaction::PromptInputKind::SingleSelect
        ));
        assert_eq!(
            prompt
                .choices
                .iter()
                .map(|choice| choice.label.as_str())
                .collect::<Vec<_>>(),
            vec!["GET", "POST", "PUT", "DELETE"]
        );
        assert_eq!(
            state.blocks.last().unwrap().content,
            "What is the request method?"
        );
    }

    #[test]
    fn plan_controller_question_uses_multi_select_prompt_options() {
        let clarification = parse_plan_clarification(
            "question: What does this interface do?\ntype: multi\nallow_custom: false\noptions:\n- User login\n- Data query",
        );

        assert_eq!(clarification.question, "What does this interface do?");
        assert_eq!(clarification.input_kind, PlanClarificationInputKind::Multi);
        assert_eq!(
            clarification.options,
            vec!["User login".to_string(), "Data query".to_string()]
        );
        assert!(matches!(
            clarification.to_prompt_input().kind,
            interaction::PromptInputKind::MultiSelect
        ));
    }

    #[test]
    fn plan_prompt_input_handles_selection_without_mode_switching() {
        let mut state = WorkspaceState::new(Vec::new());
        state.plan_draft = Some(PlanDraft::new("design an api"));
        state.prompt_input = Some(
            PlanClarification {
                question: "What is the request method?".to_string(),
                input_kind: PlanClarificationInputKind::Single,
                options: vec!["GET".to_string(), "POST".to_string()],
                allow_custom: false,
            }
            .to_prompt_input(),
        );

        assert!(matches!(
            state.handle_key(key(KeyCode::Down), &[]),
            WorkspaceAction::None
        ));
        assert_eq!(state.prompt_input.as_ref().unwrap().highlighted, 1);
        assert!(matches!(
            state.handle_key(key(KeyCode::Tab), &[]),
            WorkspaceAction::None
        ));
        assert_eq!(state.mode, InteractionMode::Plan);
    }

    #[test]
    fn plan_controller_waiting_clears_previous_prompt_input() {
        let mut state = WorkspaceState::new(Vec::new());
        state.plan_draft = Some(PlanDraft::new("design an api"));
        state.prompt_input = Some(
            PlanClarification {
                question: "What is the request method?".to_string(),
                input_kind: PlanClarificationInputKind::Single,
                options: vec!["GET".to_string(), "POST".to_string()],
                allow_custom: false,
            }
            .to_prompt_input(),
        );
        type_text(&mut state, "GET");

        state.start_plan_controller_wait();

        assert!(state.prompt_input.is_none());
        assert!(state.input.is_empty());
        assert!(matches!(state.interaction, InteractionUi::TextInput));
        assert_eq!(state.focus_zone, FocusZone::Input);
        assert_eq!(
            state.blocks.last().unwrap().block_type,
            crate::tui::state::ConversationBlockType::Execution
        );
    }

    #[test]
    fn plan_ready_waits_for_approval_before_showing_plan_panel() {
        let mut state = WorkspaceState::new(Vec::new());
        state.begin_plan_draft("build a feature");
        state.push_block(ConversationBlock::execution(""));

        state.apply_plan_controller_response(
            "ALIUS_PLAN_READY\n1. Inspect code\n2. Implement change\n3. Validate".to_string(),
        );

        assert!(state.plans.is_empty());
        assert!(matches!(state.interaction, InteractionUi::Decision(_)));
        assert_eq!(state.plan_draft.as_ref().unwrap().nodes.len(), 3);

        assert!(state.activate_draft_plan());
        assert_eq!(state.plans.len(), 3);
        assert!(state.plan_draft.is_none());
    }

    #[test]
    fn closing_completed_plan_hides_plan_panel() {
        let mut state = WorkspaceState::new(Vec::new());
        state.plans = vec![PlanNode::new("step-1", "Done", PlanNodeStatus::Completed)];
        state.pending_goal = Some("goal".to_string());

        state.close_plan();

        assert!(state.plans.is_empty());
        assert!(state.pending_goal.is_none());
        assert!(matches!(state.interaction, InteractionUi::TextInput));
    }

    #[test]
    fn parse_plan_nodes_reads_numbered_and_bulleted_steps() {
        let nodes = parse_plan_nodes("1. Inspect code\n2) Implement change\n- Validate");

        assert_eq!(
            nodes
                .iter()
                .map(|node| node.title.as_str())
                .collect::<Vec<_>>(),
            vec!["Inspect code", "Implement change", "Validate"]
        );
    }

    #[test]
    fn config_task_up_down_still_moves_prompt_selection() {
        let mut state = WorkspaceState::new(Vec::new());
        state.config_task = Some(ConfigTask::new(runtime_config::Settings::default()));
        state.prompt_input = Some(PromptInputState::new(
            "Provider",
            interaction::PromptInputKind::SingleSelect,
            vec![
                interaction::PromptChoice::new("OpenAI", "openai"),
                interaction::PromptChoice::new("BigModel", "bigmodel"),
            ],
            "help",
        ));

        let action = state.handle_key(key(KeyCode::Down), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "");
        assert_eq!(state.prompt_input.as_ref().unwrap().highlighted, 1);
    }

    #[test]
    fn paste_updates_normal_input() {
        let mut state = WorkspaceState::new(Vec::new());

        state.handle_paste("hello");

        assert_eq!(state.input.value(), "hello");
        assert_eq!(state.focus_zone, FocusZone::Input);
    }

    #[test]
    fn paste_in_config_task_updates_prompt_input() {
        let mut state = WorkspaceState::new(Vec::new());
        state.config_task = Some(ConfigTask::new(runtime_config::Settings::default()));
        state.prompt_input = Some(PromptInputState::new(
            "API key",
            interaction::PromptInputKind::Text { masked: false },
            Vec::new(),
            "help",
        ));

        state.handle_paste("sk-pasted");

        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.custom_input.value(), "sk-pasted");
        assert!(prompt.input_focused);
    }

    #[test]
    fn init_task_starts_inline_with_project_initialization_title() {
        rust_i18n::set_locale("en");
        let (_dir, _guard) = enter_temp_cwd();
        let mut state = WorkspaceState::new(Vec::new());

        state.start_init_task(runtime_config::Settings::default());

        assert!(state.config_task.is_some());
        assert!(matches!(state.interaction, InteractionUi::TextInput));
        let prompt = state.prompt_input.as_ref().unwrap();
        // Auto-start skips the Start confirmation and lands on language select.
        assert_eq!(prompt.title, "Choose interface language");
        assert_eq!(prompt.scope_title.as_deref(), Some("select-language"));
        assert!(!state
            .blocks
            .iter()
            .any(|block| block.content.trim() == "/init"));
        assert!(state
            .blocks
            .iter()
            .any(|block| block.content.contains("Project initialization started")));
        assert!(state.config_side_panel.is_some());
        assert!(state
            .config_side_panel
            .as_ref()
            .unwrap()
            .content
            .contains("Configuration check"));
        assert!(!state
            .blocks
            .iter()
            .any(|block| block.content.contains("Configuration check")));
    }

    #[test]
    fn config_task_opens_software_configuration_without_echoing_command() {
        rust_i18n::set_locale("en");
        let mut state = WorkspaceState::new(Vec::new());

        state.start_config_task(runtime_config::Settings::default());

        assert!(state.config_task.is_some());
        assert!(matches!(state.interaction, InteractionUi::TextInput));
        assert!(!state
            .blocks
            .iter()
            .any(|block| block.content.trim() == "/config"));
        assert!(state
            .blocks
            .iter()
            .any(|block| block.content.contains("Software configuration opened")));
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.title, "Model Assignment");
        assert_eq!(prompt.scope_title.as_deref(), Some("configuration-models"));

        // Tab cycles Model Assignment -> Language -> Soul -> Model Assignment
        assert!(matches!(
            state.handle_key(key(KeyCode::Tab), &[]),
            WorkspaceAction::None
        ));
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.title, "Language");
        assert_eq!(
            prompt.scope_title.as_deref(),
            Some("configuration-language")
        );

        assert!(matches!(
            state.handle_key(key(KeyCode::Tab), &[]),
            WorkspaceAction::None
        ));
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.title, "Role");
        assert_eq!(prompt.scope_title.as_deref(), Some("configuration-soul"));

        assert!(matches!(
            state.handle_key(key(KeyCode::Tab), &[]),
            WorkspaceAction::None
        ));
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.title, "Model Assignment");
        assert_eq!(prompt.scope_title.as_deref(), Some("configuration-models"));
    }

    #[test]
    fn init_task_clears_stale_plan_draft() {
        let (_dir, _guard) = enter_temp_cwd();
        let mut state = WorkspaceState::new(Vec::new());
        state.begin_plan_draft("build a feature");

        state.start_init_task(runtime_config::Settings::default());

        assert!(state.config_task.is_some());
        assert!(state.plan_draft.is_none());
        assert!(state.pending_goal.is_none());
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.title, "Choose interface language");
        assert_eq!(prompt.scope_title.as_deref(), Some("select-language"));
        assert!(state.config_side_panel.is_some());
    }

    #[test]
    fn esc_during_execution_shows_interrupt_confirmation() {
        let mut state = WorkspaceState::new(Vec::new());
        state.start_execution(ExecutionMode::Plan);

        let action = handle_execution_key(&mut state, key(KeyCode::Esc));

        assert_eq!(action, ExecutionInputAction::None);
        match &state.interaction {
            InteractionUi::Decision(decision) => {
                assert_eq!(decision.kind, DecisionKind::ExecutionInterrupt);
                assert_eq!(
                    decision.scope_title.as_deref(),
                    Some(t!("workspace.execution_interrupt.input_title").as_ref())
                );
                assert_eq!(decision.selected, 0);
            }
            interaction => panic!("expected interrupt decision, got {interaction:?}"),
        }
    }

    #[test]
    fn interrupt_confirmation_can_continue_waiting() {
        let mut state = WorkspaceState::new(Vec::new());
        state.start_execution(ExecutionMode::Plan);
        state.show_execution_interrupt_prompt();

        assert_eq!(
            handle_execution_key(&mut state, key(KeyCode::Down)),
            ExecutionInputAction::None
        );
        assert_eq!(
            handle_execution_key(&mut state, key(KeyCode::Enter)),
            ExecutionInputAction::None
        );

        assert!(matches!(state.interaction, InteractionUi::TextInput));
    }

    #[test]
    fn interrupt_confirmation_marks_running_plan_cancelled() {
        let mut state = WorkspaceState::new(Vec::new());
        state.plans = vec![PlanNode::new(
            "step-1",
            "Execute approved step",
            PlanNodeStatus::Pending,
        )];
        state.start_execution(ExecutionMode::Plan);
        state.show_execution_interrupt_prompt();

        assert_eq!(
            handle_execution_key(&mut state, key(KeyCode::Enter)),
            ExecutionInputAction::InterruptConfirmed
        );
        state.interrupt_execution();

        assert!(matches!(state.interaction, InteractionUi::TextInput));
        assert!(state
            .plans
            .iter()
            .any(|node| node.status == PlanNodeStatus::Cancelled));
        assert!(state
            .blocks
            .iter()
            .any(|block| block.content == t!("workspace.execution_interrupted")));
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
    let mut state = WorkspaceState::new_for_session(initial_missing, session);

    loop {
        if state.last_workspace_refresh.elapsed() >= Duration::from_secs(GIT_REFRESH_SECS) {
            state.workspace_status = WorkspaceStatus::load();
            state.last_workspace_refresh = Instant::now();
        }

        let header = AgentHeader::copilot(session.soul());
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
                    if state.activate_draft_plan() {
                        execute_approved_plan(app, session, &mut state).await?;
                    } else {
                        state.push_block(ConversationBlock::error(t!("workspace.no_pending_plan")));
                        state.restore_text_input();
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
                        execute_goal(app, session, &mut state, prompt, ExecutionMode::Plan).await?;
                    }
                }
                WorkspaceAction::RevisePlan(custom) => {
                    if state.plan_draft.is_some() {
                        if custom.trim().is_empty() {
                            state.push_block(ConversationBlock::decision(t!(
                                "workspace.plan_revision_requested"
                            )));
                            state.restore_text_input();
                        } else {
                            state.record_input_request(custom.trim());
                            state.add_plan_detail(custom.trim());
                            advance_plan_draft(app, session, &mut state).await?;
                        }
                    } else {
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
                }
                WorkspaceAction::CancelDecision => {
                    if state.plan_draft.is_some() {
                        state.cancel_plan_draft();
                    } else {
                        state.push_block(ConversationBlock::decision(t!(
                            "workspace.decision_cancelled"
                        )));
                        state.restore_text_input();
                    }
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
                    let current_settings = session.settings.read().unwrap().clone();
                    state.start_init_task(current_settings);
                }
                WorkspaceAction::InterruptExecution => {
                    state.interrupt_execution();
                }
                WorkspaceAction::ContinueExecution => {
                    state.dismiss_execution_interrupt_prompt();
                }
                WorkspaceAction::ContinueConfig => {
                    state.dismiss_config_exit_prompt();
                }
                WorkspaceAction::ClosePlan => {
                    state.close_plan();
                }
            },
            Event::Paste(text) => {
                state.handle_paste(&text);
            }
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
    // Handle tool confirmation response
    if let Some((run_ref, tool_call_id, approved)) = state.handle_tool_confirmation_response(&input)
    {
        if let Some(bridge) = session.bridge.as_ref() {
            bridge
                .respond_confirmation(&run_ref, &tool_call_id, approved)
                .map_err(|e| anyhow::anyhow!("Failed to respond to tool confirmation: {}", e))?;
        }
        return Ok(());
    }

    if state.config_task.is_some() {
        state.submit_config_answer(session, input)?;
        return Ok(());
    }

    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    if state.plan_draft.is_some()
        && matches!(state.interaction, InteractionUi::TextInput)
        && (trimmed.eq_ignore_ascii_case("/cancel") || trimmed.eq_ignore_ascii_case("cancel"))
    {
        state.cancel_plan_draft();
        return Ok(());
    }

    if trimmed == "/config" {
        let current_settings = session.settings.read().unwrap().clone();
        state.start_config_task(current_settings);
        return Ok(());
    }

    if trimmed == "/model" {
        let current_settings = session.settings.read().unwrap().clone();
        state.start_model_task(current_settings);
        return Ok(());
    }

    if trimmed == "/init" {
        let current_settings = session.settings.read().unwrap().clone();
        state.start_init_task(current_settings);
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
            state.plan_draft = None;
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
                let header = AgentHeader::copilot(session.soul());
                let model = session.model();
                state.render(frame, &header, &model, &session.models);
            })?;
        }
        return Ok(());
    }

    if state.plan_draft.is_some() && matches!(state.interaction, InteractionUi::TextInput) {
        state.record_input_request(trimmed);
        state.add_plan_detail(trimmed);
        advance_plan_draft(app, session, state).await?;
        return Ok(());
    }

    match state.mode {
        InteractionMode::Plan => {
            state.begin_plan_draft(trimmed);
            advance_plan_draft(app, session, state).await?;
        }
        InteractionMode::Bypass => {
            state.record_input_request(trimmed);
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

fn config_task_start_message(kind: ConfigTaskKind) -> String {
    match kind {
        ConfigTaskKind::Config => t!("workspace.config_task.feedback.start_config").to_string(),
        ConfigTaskKind::Init => t!("workspace.config_task.feedback.start_init").to_string(),
        ConfigTaskKind::ModelPool => {
            t!("workspace.config_task.feedback.start_model_pool").to_string()
        }
    }
}

fn config_task_saved_message(kind: ConfigTaskKind) -> String {
    match kind {
        ConfigTaskKind::Config => t!("workspace.config_task.feedback.saved_config").to_string(),
        ConfigTaskKind::Init => t!("workspace.config_task.feedback.saved_init").to_string(),
        ConfigTaskKind::ModelPool => {
            t!("workspace.config_task.feedback.saved_model_pool").to_string()
        }
    }
}

fn config_task_cancelled_message(kind: ConfigTaskKind) -> String {
    match kind {
        ConfigTaskKind::Config => t!("workspace.config_task.feedback.cancelled_config").to_string(),
        ConfigTaskKind::Init => t!("workspace.config_task.feedback.cancelled_init").to_string(),
        ConfigTaskKind::ModelPool => {
            t!("workspace.config_task.feedback.cancelled_model_pool").to_string()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ExecutionInputAction {
    None,
    InterruptConfirmed,
    /// User responded to a tool confirmation prompt (value = "approve" or "deny").
    ToolConfirmationResponse(String),
}

fn handle_execution_key(state: &mut WorkspaceState, key: KeyEvent) -> ExecutionInputAction {
    if key.kind != KeyEventKind::Press {
        return ExecutionInputAction::None;
    }

    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
        return ExecutionInputAction::InterruptConfirmed;
    }

    // If there's a pending tool confirmation, route keys to the prompt handler.
    if state.pending_tool_confirmation.is_some() && state.prompt_input.is_some() {
        match state.handle_prompt_input_key(key) {
            WorkspaceAction::Submit(value) => {
                return ExecutionInputAction::ToolConfirmationResponse(value);
            }
            WorkspaceAction::None => return ExecutionInputAction::None,
            _ => return ExecutionInputAction::None,
        }
    }

    match &mut state.interaction {
        InteractionUi::Decision(decision) if decision.kind == DecisionKind::ExecutionInterrupt => {
            match decision.handle_key(key) {
                WorkspaceAction::InterruptExecution => ExecutionInputAction::InterruptConfirmed,
                WorkspaceAction::ContinueExecution | WorkspaceAction::CancelDecision => {
                    state.dismiss_execution_interrupt_prompt();
                    ExecutionInputAction::None
                }
                _ => ExecutionInputAction::None,
            }
        }
        _ if key.code == KeyCode::Esc => {
            state.show_execution_interrupt_prompt();
            ExecutionInputAction::None
        }
        _ => ExecutionInputAction::None,
    }
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
        let header = AgentHeader::copilot(session.soul());
        let model = session.model();
        state.render(frame, &header, &model, &session.models);
    })?;

    // Use streaming path when bridge is available
    if let Some(bridge) = &session.bridge {
        session.conversation.add_user_message(prompt.clone());

        let rt_mode = match mode {
            ExecutionMode::Plan => protocol_interface::core::RuntimeMode::Plan,
            ExecutionMode::Bypass => protocol_interface::core::RuntimeMode::Chat,
        };

        match bridge.start_streaming(&prompt, rt_mode) {
            Ok((run_ref, mut event_rx)) => {
                let mut full_response = String::new();
                let mut errors: Vec<String> = Vec::new();
                let mut done = false;
                let mut interrupted = false;

                while !done {
                    // Render current state (includes streaming text so far)
                    let header = AgentHeader::copilot(session.soul());
                    let model = session.model();
                    app.draw(|frame| state.render(frame, &header, &model, &session.models))?;

                    // Drain all available events from the channel
                    loop {
                        match event_rx.try_recv() {
                            Ok(event) => match (&event.kind, &event.payload) {
                                (CoreEventKind::ModelDelta, CoreEventPayload::Text { text }) => {
                                    full_response.push_str(text);
                                    state.update_streaming_text(text);
                                }
                                (
                                    CoreEventKind::ErrorRaised,
                                    CoreEventPayload::Error { message, .. },
                                ) => {
                                    errors.push(message.clone());
                                }
                                (
                                    CoreEventKind::ToolCallStarted,
                                    CoreEventPayload::Json { value },
                                ) => {
                                    state.record_tool_call_started(value);
                                }
                                (
                                    CoreEventKind::ToolCallCompleted,
                                    CoreEventPayload::Json { value },
                                ) => {
                                    state.record_tool_call_completed(value);
                                }
                                (
                                    CoreEventKind::ToolConfirmationRequired,
                                    CoreEventPayload::ToolConfirmation {
                                        tool_call_id,
                                        tool_name,
                                        details,
                                    },
                                ) => {
                                    // Store confirmation state and wait for user response
                                    let confirmation = ToolConfirmationState {
                                        tool_call_id: tool_call_id.clone(),
                                        tool_name: tool_name.clone(),
                                        details: details.clone(),
                                        run_ref: run_ref.clone(),
                                    };
                                    state.show_tool_confirmation(confirmation);
                                    // Don't break the loop yet - wait for user response
                                }
                                (CoreEventKind::FinalResult, _) => {
                                    done = true;
                                }
                                _ => {}
                            },
                            Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                            Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                                done = true;
                                break;
                            }
                        }
                    }

                    if done {
                        break;
                    }

                    // Non-blocking check for UI events while execution is running.
                    if event::poll(Duration::from_millis(30))? {
                        if let Event::Key(key) = event::read()? {
                            match handle_execution_key(state, key) {
                                ExecutionInputAction::InterruptConfirmed => {
                                    let _ = bridge.cancel(
                                        &run_ref,
                                        Some("user interrupted execution".to_string()),
                                    );
                                    interrupted = true;
                                    done = true;
                                }
                                ExecutionInputAction::ToolConfirmationResponse(response) => {
                                    // User responded to tool confirmation
                                    if let Some(confirmation) =
                                        state.pending_tool_confirmation.take()
                                    {
                                        let approved = response == "approve";
                                        let _ = bridge.respond_confirmation(
                                            &confirmation.run_ref,
                                            &confirmation.tool_call_id,
                                            approved,
                                        );
                                        // Clear the prompt input and restore state
                                        state.prompt_input = None;
                                        state.restore_text_input();
                                    }
                                }
                                _ => {}
                            }
                        }
                    }
                }

                // Finish
                if interrupted {
                    state.interrupt_execution();
                } else if !full_response.is_empty() {
                    state.finish_streaming(&full_response, mode);
                    session
                        .conversation
                        .add_assistant_message(full_response.clone());
                } else if !errors.is_empty() {
                    state.fail_execution(&errors.join(", "));
                } else {
                    state.restore_text_input();
                }

                session.conversation_store.save_messages(
                    &session.session_metadata.id,
                    session.conversation.messages(),
                )?;
                let _ = session.session_store.update(&mut session.session_metadata);

                state.workspace_status = WorkspaceStatus::load();
                state.last_workspace_refresh = Instant::now();
                return Ok(());
            }
            Err(err) => {
                state.fail_execution(&err.to_string());
                state.workspace_status = WorkspaceStatus::load();
                state.last_workspace_refresh = Instant::now();
                return Ok(());
            }
        }
    }

    // Fallback: non-streaming path (collect_model_response)
    match collect_model_response(session, &prompt).await {
        Ok(result) => {
            if result.has_text() {
                state.finish_execution(&result.text, mode);
            } else if result.has_errors() {
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

async fn advance_plan_draft(
    app: &mut TuiApp,
    session: &mut ReplSession,
    state: &mut WorkspaceState,
) -> Result<()> {
    let Some(draft) = state.plan_draft.as_ref() else {
        return Ok(());
    };
    let controller_prompt = build_plan_controller_prompt(draft);

    state.start_plan_controller_wait();
    app.draw(|frame| {
        let header = AgentHeader::copilot(session.soul());
        let model = session.model();
        state.render(frame, &header, &model, &session.models);
    })?;

    match collect_plan_controller_response(app, session, state, &controller_prompt).await {
        Ok(Some(result)) if result.has_text() => {
            state.apply_plan_controller_response(result.text);
            for error in result.errors {
                state.push_block(ConversationBlock::error(sanitize_for_tui(&error)));
            }
        }
        Ok(Some(result)) if result.has_errors() => {
            state.fail_execution(&result.errors.join(", "));
        }
        Ok(Some(_)) => {
            state.fail_execution(t!("workspace.plan_generation_empty").as_ref());
        }
        Ok(None) => {}
        Err(err) => {
            state.fail_execution(&err.to_string());
        }
    }

    state.workspace_status = WorkspaceStatus::load();
    state.last_workspace_refresh = Instant::now();
    Ok(())
}

async fn collect_plan_controller_response(
    app: &mut TuiApp,
    session: &mut ReplSession,
    state: &mut WorkspaceState,
    prompt: &str,
) -> Result<Option<ModelResponse>> {
    let missing = missing_runtime_requirements(&session.settings.read().unwrap());
    if !missing.is_empty() {
        return Err(anyhow::anyhow!("{}", init_required_message(&missing)));
    }

    let Some(bridge) = &session.bridge else {
        return Err(anyhow::anyhow!(
            "Runtime manager unavailable. Workspace execution must run through Core Runtime."
        ));
    };

    let (run_ref, mut event_rx) = bridge.start_streaming(prompt, RuntimeMode::Chat)?;
    let mut full_response = String::new();
    let mut errors: Vec<String> = Vec::new();
    let mut done = false;
    let mut interrupted = false;

    while !done {
        let header = AgentHeader::copilot(session.soul());
        let model = session.model();
        app.draw(|frame| state.render(frame, &header, &model, &session.models))?;

        loop {
            match event_rx.try_recv() {
                Ok(event) => match (&event.kind, &event.payload) {
                    (CoreEventKind::ModelDelta, CoreEventPayload::Text { text }) => {
                        full_response.push_str(text);
                    }
                    (CoreEventKind::ErrorRaised, CoreEventPayload::Error { message, .. }) => {
                        errors.push(message.clone());
                    }
                    (CoreEventKind::ToolCallStarted, CoreEventPayload::Json { value }) => {
                        state.record_tool_call_started(value);
                    }
                    (CoreEventKind::ToolCallCompleted, CoreEventPayload::Json { value }) => {
                        state.record_tool_call_completed(value);
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
                        done = true;
                    }
                    (
                        CoreEventKind::FinalResult,
                        CoreEventPayload::Final {
                            success: true,
                            content,
                        },
                    ) => {
                        if full_response.is_empty() && !content.is_empty() {
                            full_response.push_str(content);
                        }
                        done = true;
                    }
                    (CoreEventKind::FinalResult, _) => {
                        done = true;
                    }
                    _ => {}
                },
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }

        if done {
            break;
        }

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                if handle_execution_key(state, key) == ExecutionInputAction::InterruptConfirmed {
                    let _ =
                        bridge.cancel(&run_ref, Some("user interrupted plan drafting".to_string()));
                    interrupted = true;
                    done = true;
                }
            }
        }
    }

    if interrupted {
        state.interrupt_execution();
        Ok(None)
    } else {
        Ok(Some(ModelResponse {
            text: full_response,
            errors,
        }))
    }
}

async fn execute_approved_plan(
    app: &mut TuiApp,
    session: &mut ReplSession,
    state: &mut WorkspaceState,
) -> Result<()> {
    while let Some(index) = state.next_pending_plan_node_index() {
        if !execute_plan_step(app, session, state, index).await? {
            break;
        }
    }
    Ok(())
}

async fn execute_plan_step(
    app: &mut TuiApp,
    session: &mut ReplSession,
    state: &mut WorkspaceState,
    index: usize,
) -> Result<bool> {
    let Some(prompt) = state.plan_step_prompt(index) else {
        return Ok(false);
    };
    state.start_plan_step(index);

    app.draw(|frame| {
        let header = AgentHeader::copilot(session.soul());
        let model = session.model();
        state.render(frame, &header, &model, &session.models);
    })?;

    let Some(bridge) = &session.bridge else {
        state.fail_plan_step(index, "Runtime manager unavailable.");
        return Ok(false);
    };

    let (run_ref, mut event_rx) = match bridge.start_streaming(&prompt, RuntimeMode::Plan) {
        Ok(value) => value,
        Err(err) => {
            state.fail_plan_step(index, &err.to_string());
            return Ok(false);
        }
    };

    let mut full_response = String::new();
    let mut errors: Vec<String> = Vec::new();
    let mut done = false;
    let mut interrupted = false;

    while !done {
        let header = AgentHeader::copilot(session.soul());
        let model = session.model();
        app.draw(|frame| state.render(frame, &header, &model, &session.models))?;

        loop {
            match event_rx.try_recv() {
                Ok(event) => match (&event.kind, &event.payload) {
                    (CoreEventKind::ModelDelta, CoreEventPayload::Text { text }) => {
                        full_response.push_str(text);
                        state.update_streaming_text(text);
                    }
                    (CoreEventKind::ErrorRaised, CoreEventPayload::Error { message, .. }) => {
                        errors.push(message.clone());
                    }
                    (CoreEventKind::ToolCallStarted, CoreEventPayload::Json { value }) => {
                        state.record_tool_call_started(value);
                    }
                    (CoreEventKind::ToolCallCompleted, CoreEventPayload::Json { value }) => {
                        state.record_tool_call_completed(value);
                    }
                    (
                        CoreEventKind::ToolConfirmationRequired,
                        CoreEventPayload::ToolConfirmation {
                            tool_call_id,
                            tool_name,
                            details,
                        },
                    ) => {
                        let confirmation = ToolConfirmationState {
                            tool_call_id: tool_call_id.clone(),
                            tool_name: tool_name.clone(),
                            details: details.clone(),
                            run_ref: run_ref.clone(),
                        };
                        state.show_tool_confirmation(confirmation);
                    }
                    (CoreEventKind::FinalResult, _) => {
                        done = true;
                    }
                    _ => {}
                },
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    done = true;
                    break;
                }
            }
        }

        if done {
            break;
        }

        if event::poll(Duration::from_millis(30))? {
            if let Event::Key(key) = event::read()? {
                match handle_execution_key(state, key) {
                    ExecutionInputAction::InterruptConfirmed => {
                        let _ =
                            bridge.cancel(&run_ref, Some("user interrupted execution".to_string()));
                        interrupted = true;
                        done = true;
                    }
                    ExecutionInputAction::ToolConfirmationResponse(response) => {
                        if let Some(confirmation) = state.pending_tool_confirmation.take() {
                            let approved = response == "approve";
                            let _ = bridge.respond_confirmation(
                                &confirmation.run_ref,
                                &confirmation.tool_call_id,
                                approved,
                            );
                            state.prompt_input = None;
                            state.restore_text_input();
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    if interrupted {
        state.interrupt_execution();
        return Ok(false);
    }

    if !errors.is_empty() {
        state.fail_plan_step(index, &errors.join(", "));
        return Ok(false);
    }

    state.finish_plan_step(index, &full_response);
    Ok(true)
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

async fn collect_protocol_response(
    session: &mut ReplSession,
    input: &str,
    mode: RuntimeMode,
) -> Result<ModelResponse> {
    let missing = missing_runtime_requirements(&session.settings.read().unwrap());
    if !missing.is_empty() {
        return Err(anyhow::anyhow!("{}", init_required_message(&missing)));
    }

    let Some(bridge) = &session.bridge else {
        return Err(anyhow::anyhow!(
            "Runtime manager unavailable. Workspace execution must run through Core Runtime."
        ));
    };

    let events = bridge.send_message_with_mode(input, mode)?;
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

    Ok(ModelResponse {
        text: full_response,
        errors,
    })
}

async fn collect_model_response(session: &mut ReplSession, input: &str) -> Result<ModelResponse> {
    session.conversation.add_user_message(input.to_string());
    let response = collect_protocol_response(session, input, RuntimeMode::Chat).await?;

    if !response.text.is_empty() {
        session
            .conversation
            .add_assistant_message(response.text.clone());
    }
    session.conversation_store.save_messages(
        &session.session_metadata.id,
        session.conversation.messages(),
    )?;
    let _ = session.session_store.update(&mut session.session_metadata);
    Ok(response)
}

async fn run_workspace_command(
    _app: &mut TuiApp,
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
            output: t!("workspace.init_task.inline_only").to_string(),
            quit: false,
            clear_blocks: false,
            show_init_menu: false,
        },
        "/model" if parts.len() == 1 => {
            CommandOutcome::output("Use /model in the workspace to manage the project model pool.")
        }
        _ => CommandOutcome::output(session.handle_command(input).await?),
    };

    Ok(outcome)
}

fn save_workspace_providers(providers: &runtime_config::ProviderConfig) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = runtime_config::find_project_root(&cwd).unwrap_or(cwd);
    let path = root.join(".alius/config/providers.toml");
    runtime_config::loaders::save_providers(&path, providers)
        .map_err(|error| anyhow::anyhow!("{error}"))
}

fn save_workspace_model_assignment(
    assignment: &runtime_config::ModelAssignmentConfig,
) -> Result<()> {
    let cwd = std::env::current_dir().unwrap_or_default();
    let root = runtime_config::find_project_root(&cwd).unwrap_or(cwd);
    let path = root.join(".alius/config/model.toml");
    runtime_config::save_model_assignment(&path, assignment)
        .map_err(|error| anyhow::anyhow!("{error}"))
}

#[derive(Debug, Clone)]
enum PlanControllerOutput {
    NeedDetails(PlanClarification),
    PlanReady(String, Vec<PlanNode>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanClarification {
    question: String,
    input_kind: PlanClarificationInputKind,
    options: Vec<String>,
    allow_custom: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanClarificationInputKind {
    Text,
    Single,
    Multi,
}

impl PlanClarification {
    fn to_prompt_input(&self) -> PromptInputState {
        let choices = self
            .options
            .iter()
            .map(|option| PromptChoice::new(option, option))
            .collect::<Vec<_>>();
        let kind = match (self.input_kind, choices.is_empty(), self.allow_custom) {
            (PlanClarificationInputKind::Text, _, _) => PromptInputKind::Text { masked: false },
            (PlanClarificationInputKind::Single, false, false) => PromptInputKind::SingleSelect,
            (PlanClarificationInputKind::Single, false, true) => {
                PromptInputKind::SingleSelectWithInput { masked: false }
            }
            (PlanClarificationInputKind::Single, true, _) => {
                PromptInputKind::Text { masked: false }
            }
            (PlanClarificationInputKind::Multi, false, false) => PromptInputKind::MultiSelect,
            (PlanClarificationInputKind::Multi, false, true) => {
                PromptInputKind::MultiSelectWithInput { masked: false }
            }
            (PlanClarificationInputKind::Multi, true, _) => PromptInputKind::Text { masked: false },
        };

        PromptInputState::new(
            t!("workspace.plan_draft.input_title").to_string(),
            kind,
            choices,
            t!("workspace.plan_draft.input_help").to_string(),
        )
        .with_scope_title(t!("workspace.mode.plan").to_string())
        .with_placeholder(t!("workspace.plan_draft.input_placeholder").to_string())
    }

    fn transcript_text(&self) -> String {
        if self.options.is_empty() {
            return self.question.clone();
        }
        let mode = match self.input_kind {
            PlanClarificationInputKind::Text => "text",
            PlanClarificationInputKind::Single => "single",
            PlanClarificationInputKind::Multi => "multi",
        };
        format!(
            "{}\nOptions ({mode}): {}",
            self.question,
            self.options.join(", ")
        )
    }
}

fn build_plan_controller_prompt(draft: &PlanDraft) -> String {
    format!(
        r#"You are the Alius Plan Mode state controller.

Your job is to decide whether Alius has enough task detail and all necessary preconditions to produce an execution plan.

Rules:
- Do not produce a plan until the task objective, scope, constraints, success criteria, relevant files or systems, and risky assumptions are clear enough.
- If details are missing, ask exactly one minimum necessary clarifying question at a time.
- Prefer concrete choice options over free-form input. Use `single` when one answer is expected, `multi` when multiple answers may apply, and `text` only when choices would be misleading.
- Put the question in the `question:` field only. Put candidate answers only under `options:`.
- If details are sufficient, produce a concrete execution plan with ordered steps.
- Keep the output concise and actionable.

Respond using exactly one of these formats:

ALIUS_NEED_DETAILS
question: <one clarifying question>
type: single|multi|text
allow_custom: true|false
options:
- <first option>
- <second option>

ALIUS_PLAN_READY
1. <first execution step>
2. <second execution step>
3. <third execution step>

Original goal:
{goal}

Current planning dialogue:
{transcript}
"#,
        goal = draft.goal.trim(),
        transcript = draft.transcript()
    )
}

fn parse_plan_controller_output(response: &str, goal: &str) -> PlanControllerOutput {
    let trimmed = response.trim();
    let (marker, body) = split_plan_marker(trimmed);

    match marker {
        Some("ALIUS_PLAN_READY") => {
            let proposal = body.trim();
            let nodes = plan_nodes_from_proposal(proposal, goal);
            PlanControllerOutput::PlanReady(proposal.to_string(), nodes)
        }
        Some("ALIUS_NEED_DETAILS") => {
            PlanControllerOutput::NeedDetails(parse_plan_clarification(body))
        }
        _ => {
            let nodes = parse_plan_nodes(trimmed);
            if nodes.len() >= 2 && !trimmed.contains('?') {
                PlanControllerOutput::PlanReady(trimmed.to_string(), nodes)
            } else {
                PlanControllerOutput::NeedDetails(parse_plan_clarification(trimmed))
            }
        }
    }
}

fn parse_plan_clarification(body: &str) -> PlanClarification {
    let mut question = String::new();
    let mut input_kind = PlanClarificationInputKind::Text;
    let mut options = Vec::new();
    let mut allow_custom = true;
    let mut in_options = false;
    let mut fallback_lines = Vec::new();

    for raw_line in body.lines() {
        let line = raw_line.trim();
        if line.is_empty() {
            continue;
        }
        let lower = line.to_ascii_lowercase();
        if let Some(value) = line
            .strip_prefix("question:")
            .or_else(|| line.strip_prefix("Question:"))
        {
            question = value.trim().to_string();
            in_options = false;
            continue;
        }
        if let Some(value) = lower.strip_prefix("type:") {
            input_kind = match value.trim() {
                "single" | "single-select" | "radio" => PlanClarificationInputKind::Single,
                "multi" | "multi-select" | "checkbox" => PlanClarificationInputKind::Multi,
                _ => PlanClarificationInputKind::Text,
            };
            in_options = false;
            continue;
        }
        if let Some(value) = lower.strip_prefix("allow_custom:") {
            allow_custom = matches!(value.trim(), "true" | "yes" | "1");
            in_options = false;
            continue;
        }
        if lower == "options:" {
            in_options = true;
            continue;
        }
        if in_options {
            if let Some(option) = plan_option_from_line(line) {
                options.push(option);
                continue;
            }
        }
        fallback_lines.push(line.to_string());
    }

    if question.is_empty() {
        question = fallback_lines.join("\n").trim().to_string();
    }
    if question.is_empty() {
        question = t!("workspace.plan_draft.need_details_fallback").to_string();
    }
    if input_kind == PlanClarificationInputKind::Text && !options.is_empty() {
        input_kind = if options.len() <= 2 {
            PlanClarificationInputKind::Single
        } else {
            PlanClarificationInputKind::Multi
        };
    }

    PlanClarification {
        question,
        input_kind,
        options,
        allow_custom,
    }
}

fn plan_option_from_line(line: &str) -> Option<String> {
    let without_number = strip_numbered_prefix(line);
    let option = without_number
        .strip_prefix("- ")
        .or_else(|| without_number.strip_prefix("* "))
        .or_else(|| without_number.strip_prefix("• "))
        .unwrap_or(without_number)
        .trim();
    if option.is_empty() {
        None
    } else {
        Some(option.to_string())
    }
}

fn split_plan_marker(response: &str) -> (Option<&'static str>, &str) {
    for marker in ["ALIUS_PLAN_READY", "ALIUS_NEED_DETAILS"] {
        if let Some(rest) = response.strip_prefix(marker) {
            return (Some(marker), rest.trim_start());
        }
    }
    (None, response)
}

fn plan_nodes_from_proposal(proposal: &str, goal: &str) -> Vec<PlanNode> {
    let mut nodes = parse_plan_nodes(proposal);
    if nodes.is_empty() {
        let title = proposal
            .lines()
            .find(|line| !line.trim().is_empty())
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .unwrap_or(goal)
            .to_string();
        nodes.push(PlanNode::new("step-1", title, PlanNodeStatus::Pending).with_owner("local"));
    }
    nodes
}

fn parse_plan_nodes(proposal: &str) -> Vec<PlanNode> {
    proposal
        .lines()
        .filter_map(plan_node_title_from_line)
        .enumerate()
        .map(|(index, title)| {
            PlanNode::new(
                format!("step-{}", index + 1),
                title,
                PlanNodeStatus::Pending,
            )
            .with_owner("local")
        })
        .collect()
}

fn plan_node_title_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let without_number = strip_numbered_prefix(trimmed);
    let without_bullet = without_number
        .strip_prefix("- ")
        .or_else(|| without_number.strip_prefix("* "))
        .or_else(|| without_number.strip_prefix("• "))
        .unwrap_or(without_number)
        .trim();
    let without_checkbox = without_bullet
        .strip_prefix("[ ] ")
        .or_else(|| without_bullet.strip_prefix("[x] "))
        .or_else(|| without_bullet.strip_prefix("[X] "))
        .unwrap_or(without_bullet)
        .trim();

    if without_checkbox == trimmed && !starts_with_numbered_prefix(trimmed) {
        return None;
    }

    let title = without_checkbox
        .trim_start_matches(['-', '*', '•'])
        .trim()
        .trim_matches('`')
        .to_string();

    if title.is_empty() {
        None
    } else {
        Some(title)
    }
}

fn strip_numbered_prefix(line: &str) -> &str {
    let mut chars = line.char_indices().peekable();
    let mut saw_digit = false;
    while let Some((_, ch)) = chars.peek().copied() {
        if ch.is_ascii_digit() {
            saw_digit = true;
            chars.next();
        } else {
            break;
        }
    }

    if !saw_digit {
        return line;
    }

    let Some((sep_index, sep)) = chars.peek().copied() else {
        return line;
    };
    if sep != '.' && sep != ')' && sep != '、' {
        return line;
    }
    let rest_start = sep_index + sep.len_utf8();
    line[rest_start..].trim_start()
}

fn starts_with_numbered_prefix(line: &str) -> bool {
    !std::ptr::eq(strip_numbered_prefix(line), line)
}

fn build_plan_step_prompt(goal: &str, plans: &[PlanNode], index: usize) -> String {
    let plan_text = plans
        .iter()
        .enumerate()
        .map(|(idx, node)| format!("{}. {}", idx + 1, node.title))
        .collect::<Vec<_>>()
        .join("\n");
    let current = plans
        .get(index)
        .map(|node| node.title.as_str())
        .unwrap_or(goal);

    format!(
        r#"Execute one approved plan step.

Original user goal:
{goal}

Approved plan:
{plan}

Current step:
{step_number}. {current}

Instructions:
- Execute only the current step.
- Use available tools when needed.
- If a required prerequisite is missing, stop and explain the blocker clearly.
- Return the concrete result of this step and any evidence or files changed.
"#,
        goal = goal.trim(),
        plan = plan_text,
        step_number = index + 1,
        current = current
    )
}

fn workspace_help() -> String {
    [
        t!("workspace.help.title").to_string(),
        format!("  /help              {}", t!("workspace.help.help")),
        format!("  /clear             {}", t!("workspace.help.clear")),
        format!("  /history           {}", t!("workspace.help.history")),
        format!("  /config            {}", t!("workspace.help.config")),
        format!("  /model             {}", t!("workspace.help.model")),
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
            protocol_interface::MessageRole::Tool => t!("workspace.history.tool"),
            protocol_interface::MessageRole::Summary => t!("workspace.history.summary"),
        };
        let preview = helpers::truncate_chars(message.content.trim(), 96);
        out.push_str(&format!("  {:3}. {:<8} {}\n", index + 1, label, preview));
    }
    out.trim_end().to_string()
}

fn render_config_side_panel(
    frame: &mut Frame,
    area: Rect,
    panel: &ConfigSidePanel,
    scroll: &mut PanelScroll,
    focused: bool,
    hovered: bool,
) {
    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" {} ", panel.title))
        .style(crate::tui::theme::base())
        .border_style(crate::tui::theme::border_state(focused, hovered));
    let inner = block.inner(area);
    frame.render_widget(block, area);

    let lines = panel
        .content
        .lines()
        .map(|line| Line::from(line.to_string()))
        .collect::<Vec<_>>();
    let total_visual = helpers::count_visual_lines(&lines, inner.width);
    let max_off = total_visual.saturating_sub(inner.height as usize) as u16;
    scroll.clamp(max_off);

    let paragraph = Paragraph::new(Text::from(lines))
        .wrap(Wrap { trim: false })
        .scroll((scroll.offset, 0));
    frame.render_widget(paragraph, inner);
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

fn tool_display_name(value: &serde_json::Value) -> String {
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool");
    if name == "shell" {
        if let Some(command) = value
            .get("args")
            .and_then(|args| args.get("command"))
            .and_then(serde_json::Value::as_str)
            .map(sanitize_for_tui)
            .filter(|command| !command.is_empty())
        {
            return format!("shell: {command}");
        }
    }
    name.to_string()
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

fn startup_welcome_state(
    session: Option<&ReplSession>,
    initialized: bool,
) -> conversation::WelcomeState {
    let version = option_env!("ALIUS_VERSION")
        .unwrap_or(env!("CARGO_PKG_VERSION"))
        .trim_start_matches('v');
    let mut state = conversation::WelcomeState {
        version: format!("v{version}"),
        ready: initialized,
        soul: initialized.then(|| session.map(ReplSession::soul).unwrap_or_default()),
        model_plan: None,
        model_execute: initialized.then(|| session.map(ReplSession::model).unwrap_or_default()),
        model_review: None,
    };

    if initialized {
        if let Some(snapshot) = std::env::current_dir()
            .ok()
            .and_then(|cwd| runtime_config::load_project_config(&cwd).ok())
        {
            state.model_plan = assigned_welcome_model(&snapshot, ModelAssignmentRole::Plan);
            state.model_execute = assigned_welcome_model(&snapshot, ModelAssignmentRole::Execute)
                .or(state.model_execute);
            state.model_review = assigned_welcome_model(&snapshot, ModelAssignmentRole::Review);
        }
    }

    state
}

fn assigned_welcome_model(
    snapshot: &runtime_config::ProjectConfigSnapshot,
    role: ModelAssignmentRole,
) -> Option<String> {
    let model_id = snapshot.model_assignment.get(role).trim();
    if model_id.is_empty() {
        return None;
    }
    let entry = snapshot
        .providers
        .model_library
        .models
        .iter()
        .find(|entry| entry.enabled && entry.id == model_id)?;
    Some(format!(
        "{}({})",
        welcome_provider_name(&entry.provider),
        entry.model_name
    ))
}

fn welcome_provider_name(provider: &str) -> &'static str {
    match provider {
        "bigmodel" => "BigModel",
        "xiaomi_mimo" => "Xiaomi MiMo",
        "deepseek" => "DeepSeek",
        _ => "Provider",
    }
}

#[derive(Debug, Clone)]
struct PlanDraft {
    goal: String,
    turns: Vec<PlanDraftTurn>,
    proposal: Option<String>,
    nodes: Vec<PlanNode>,
}

#[derive(Debug, Clone)]
struct PlanDraftTurn {
    role: PlanDraftRole,
    content: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlanDraftRole {
    User,
    Assistant,
}

impl PlanDraft {
    fn new(goal: impl Into<String>) -> Self {
        let goal = goal.into();
        Self {
            turns: vec![PlanDraftTurn {
                role: PlanDraftRole::User,
                content: goal.clone(),
            }],
            goal,
            proposal: None,
            nodes: Vec::new(),
        }
    }

    fn add_user_turn(&mut self, content: impl Into<String>) {
        self.turns.push(PlanDraftTurn {
            role: PlanDraftRole::User,
            content: content.into(),
        });
    }

    fn add_assistant_turn(&mut self, content: impl Into<String>) {
        self.turns.push(PlanDraftTurn {
            role: PlanDraftRole::Assistant,
            content: content.into(),
        });
    }

    fn set_proposal(&mut self, proposal: impl Into<String>, nodes: Vec<PlanNode>) {
        self.proposal = Some(proposal.into());
        self.nodes = nodes;
    }

    fn transcript(&self) -> String {
        self.turns
            .iter()
            .map(|turn| {
                let role = match turn.role {
                    PlanDraftRole::User => "User",
                    PlanDraftRole::Assistant => "Alius",
                };
                format!("{role}: {}", turn.content.trim())
            })
            .collect::<Vec<_>>()
            .join("\n\n")
    }
}

// ---------------------------------------------------------------------------
// WorkspaceState
// ---------------------------------------------------------------------------

/// Pending tool confirmation state.
#[derive(Debug, Clone)]
pub struct ToolConfirmationState {
    pub tool_call_id: String,
    pub tool_name: String,
    pub details: String,
    pub run_ref: RunRef,
}

struct WorkspaceState {
    mode: InteractionMode,
    active_tab: MainTab,
    blocks: Vec<ConversationBlock>,
    plans: Vec<PlanNode>,
    agent_team: Option<AgentTeamState>,
    input: InputBuffer,
    input_history: Vec<String>,
    input_history_cursor: Option<usize>,
    input_history_draft: String,
    prompt_input: Option<PromptInputState>,
    pending_tool_confirmation: Option<ToolConfirmationState>,
    config_side_panel: Option<ConfigSidePanel>,
    interaction: InteractionUi,
    config_task: Option<ConfigTask>,
    plan_draft: Option<PlanDraft>,
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
    expanded_blocks: std::collections::HashSet<String>,
    global_expanded: bool,
    block_row_map: std::collections::HashMap<String, (u16, u16)>,
}

impl WorkspaceState {
    #[cfg(test)]
    fn new(initial_missing: Vec<String>) -> Self {
        Self::new_with_welcome(initial_missing, None)
    }

    fn new_for_session(initial_missing: Vec<String>, session: &ReplSession) -> Self {
        let welcome = startup_welcome_state(Some(session), initial_missing.is_empty());
        Self::new_with_welcome(initial_missing, Some(welcome))
    }

    fn new_with_welcome(
        initial_missing: Vec<String>,
        welcome: Option<conversation::WelcomeState>,
    ) -> Self {
        let workspace_status = WorkspaceStatus::load();
        let welcome =
            welcome.unwrap_or_else(|| startup_welcome_state(None, initial_missing.is_empty()));
        let blocks = vec![ConversationBlock::welcome_state(welcome)];

        let _ = initial_missing;

        Self {
            mode: InteractionMode::Plan,
            active_tab: MainTab::Conversation,
            blocks,
            plans: Vec::new(),
            agent_team: None,
            input: InputBuffer::default(),
            input_history: Vec::new(),
            input_history_cursor: None,
            input_history_draft: String::new(),
            prompt_input: None,
            pending_tool_confirmation: None,
            config_side_panel: None,
            interaction: InteractionUi::TextInput,
            config_task: None,
            plan_draft: None,
            pending_goal: None,
            workspace_status,
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
            expanded_blocks: std::collections::HashSet::new(),
            global_expanded: false,
            block_row_map: std::collections::HashMap::new(),
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

        let interaction_height = self.interaction_height(inner.height);
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
                prompt_input: self.prompt_input.as_ref(),
                has_agent_team: self.has_agent_team_tab(),
                command_hint: self.command_hint(models),
                config_section: self
                    .config_task
                    .as_ref()
                    .and_then(ConfigTask::current_section),
                init_nav: self
                    .config_task
                    .as_ref()
                    .and_then(ConfigTask::init_nav_snapshot),
            },
            self.focus_zone == FocusZone::Input,
            self.hover_zone == Some(HoverZone::Interaction),
        );
        status_bar::render(frame, layout[3], &self.workspace_status);
    }

    fn render_main(&mut self, frame: &mut Frame, area: Rect, model: &str) {
        let has_plans = !self.plans.is_empty();
        let has_side_panel = self.config_side_panel.is_some();
        let has_right_panel = has_plans || has_side_panel;
        let constraints = if has_right_panel {
            [Constraint::Percentage(68), Constraint::Percentage(32)]
        } else {
            [Constraint::Percentage(100), Constraint::Percentage(0)]
        };
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area);

        self.layout_rects.plans = if has_right_panel {
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
                self.block_row_map = conversation::render(
                    frame,
                    chunks[0],
                    &self.blocks,
                    model,
                    &tab_title,
                    &mut self.conv_scroll,
                    conv_focused,
                    conv_hovered,
                    &self.expanded_blocks,
                    self.global_expanded,
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
        if let Some(panel) = &self.config_side_panel {
            render_config_side_panel(
                frame,
                chunks[1],
                panel,
                &mut self.plans_scroll,
                self.focus_zone == FocusZone::Plans,
                self.hover_zone == Some(HoverZone::Plans),
            );
        } else if has_plans {
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

    fn has_right_panel(&self) -> bool {
        !self.plans.is_empty() || self.config_side_panel.is_some()
    }

    fn interaction_height(&self, total_height: u16) -> u16 {
        if let Some(prompt_input) = &self.prompt_input {
            return prompt_input.preferred_height(total_height);
        }
        self.interaction.height(total_height)
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

        // Ctrl+O: Toggle global expand/collapse
        if key.code == KeyCode::Char('o') && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_global_expand();
            return WorkspaceAction::None;
        }

        if key.code == KeyCode::Tab && key.modifiers.contains(KeyModifiers::CONTROL) {
            self.toggle_tab();
            return WorkspaceAction::None;
        }

        if self.config_task.is_some()
            && matches!(self.interaction, InteractionUi::TextInput)
            && matches!(key.code, KeyCode::Tab | KeyCode::BackTab)
        {
            let reverse = key.code == KeyCode::BackTab;
            if let Some(task) = self.config_task.as_mut() {
                // /init: BackTab steps the wizard back one stage; Tab is a no-op here
                // (Init kind's switch_tab early-returns). Other kinds use switch_tab.
                if reverse && task.kind() == ConfigTaskKind::Init {
                    if let Some(prompt) = task.init_back() {
                        self.set_config_prompt(prompt);
                    }
                } else {
                    let prompt = task.switch_tab(reverse);
                    self.set_config_prompt(prompt);
                }
            }
            return WorkspaceAction::None;
        }

        if key.code == KeyCode::BackTab {
            if matches!(self.interaction, InteractionUi::TextInput) && self.prompt_input.is_none() {
                self.toggle_mode();
            }
            return WorkspaceAction::None;
        }

        if key.code == KeyCode::Tab
            && key.modifiers.is_empty()
            && self.focus_zone == FocusZone::Input
            && matches!(self.interaction, InteractionUi::TextInput)
            && self.prompt_input.is_none()
            && self.complete_command(models)
        {
            return WorkspaceAction::None;
        }

        // Tab: cycle focus zones (TextInput mode only)
        if key.code == KeyCode::Tab
            && key.modifiers.is_empty()
            && matches!(self.interaction, InteractionUi::TextInput)
            && self.prompt_input.is_none()
        {
            self.focus_zone = match self.focus_zone {
                FocusZone::Input => FocusZone::Conversation,
                FocusZone::Conversation => {
                    if self.has_right_panel() {
                        FocusZone::Plans
                    } else {
                        FocusZone::Input
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

    fn handle_paste(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        match &mut self.interaction {
            InteractionUi::TextInput if self.prompt_input.is_some() => {
                if let Some(prompt_input) = self.prompt_input.as_mut() {
                    prompt_input.paste(text);
                }
            }
            InteractionUi::TextInput => {
                self.reset_input_history_navigation();
                self.input.paste(text);
            }
            InteractionUi::Decision(decision) => {
                decision.paste(text);
            }
        }

        self.focus_zone = FocusZone::Input;
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
            MouseEventKind::Down(_)
                // Handle click on conversation blocks to toggle fold/unfold
                if self.layout_rects.conversation.contains(Position {
                    x: mouse.column,
                    y: mouse.row,
                }) =>
            {
                self.handle_conversation_click(mouse.column, mouse.row);
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
        if self.prompt_input.is_some() {
            return self.handle_prompt_input_key(key);
        }

        match key.code {
            KeyCode::Enter => {
                if key.modifiers.contains(KeyModifiers::CONTROL) || !self.input.is_empty() {
                    let input = self.input.take();
                    self.record_input_history(&input);
                    WorkspaceAction::Submit(input)
                } else {
                    WorkspaceAction::None
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.reset_input_history_navigation();
                WorkspaceAction::None
            }
            KeyCode::Up => {
                self.recall_previous_input();
                WorkspaceAction::None
            }
            KeyCode::Down => {
                self.recall_next_input();
                WorkspaceAction::None
            }
            _ => {
                if Self::key_edits_input(key) {
                    self.reset_input_history_navigation();
                }
                self.input.handle_key(key);
                WorkspaceAction::None
            }
        }
    }

    fn record_input_history(&mut self, input: &str) {
        if !input.is_empty() {
            self.input_history.push(input.to_string());
        }
        self.reset_input_history_navigation();
    }

    fn recall_previous_input(&mut self) {
        if self.input_history.is_empty() {
            return;
        }

        let index = match self.input_history_cursor {
            Some(0) => 0,
            Some(index) => index.saturating_sub(1),
            None => {
                self.input_history_draft = self.input.value().to_string();
                self.input_history.len().saturating_sub(1)
            }
        };

        self.input_history_cursor = Some(index);
        self.input.set_value(self.input_history[index].clone());
    }

    fn recall_next_input(&mut self) {
        let Some(index) = self.input_history_cursor else {
            return;
        };

        if index + 1 < self.input_history.len() {
            let next = index + 1;
            self.input_history_cursor = Some(next);
            self.input.set_value(self.input_history[next].clone());
        } else {
            self.input_history_cursor = None;
            self.input
                .set_value(std::mem::take(&mut self.input_history_draft));
        }
    }

    fn reset_input_history_navigation(&mut self) {
        self.input_history_cursor = None;
        self.input_history_draft.clear();
    }

    fn key_edits_input(key: KeyEvent) -> bool {
        match key.code {
            KeyCode::Char(_) => !key.modifiers.contains(KeyModifiers::CONTROL),
            KeyCode::Backspace | KeyCode::Delete => true,
            _ => false,
        }
    }

    fn handle_prompt_input_key(&mut self, key: KeyEvent) -> WorkspaceAction {
        let action = match self.prompt_input.as_mut() {
            Some(prompt_input) => prompt_input.handle_key(key),
            None => return WorkspaceAction::None,
        };

        match action {
            PromptInputAction::Submit(input) => WorkspaceAction::Submit(input),
            PromptInputAction::Cancel => {
                if self
                    .config_task
                    .as_ref()
                    .map(|task| task.is_dirty())
                    .unwrap_or(false)
                {
                    self.show_config_exit_prompt();
                    WorkspaceAction::None
                } else {
                    WorkspaceAction::Submit("/cancel".to_string())
                }
            }
            PromptInputAction::None => WorkspaceAction::None,
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
        self.reset_input_history_navigation();
        true
    }

    fn command_hint(&self, models: &[String]) -> Option<String> {
        if self.prompt_input.is_some() {
            return None;
        }

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

    fn start_config_task(&mut self, settings: runtime_config::Settings) {
        self.start_config_like_task(ConfigTask::new(settings));
    }

    fn start_init_task(&mut self, settings: runtime_config::Settings) {
        self.start_config_like_task(ConfigTask::init(settings));
    }

    fn start_model_task(&mut self, settings: runtime_config::Settings) {
        self.start_config_like_task(ConfigTask::model_switch(settings));
    }

    fn start_config_like_task(&mut self, task: ConfigTask) {
        let task_kind = task.kind();
        let prompt = task.prompt();
        let overview = if task_kind == ConfigTaskKind::Config {
            Some(task.overview_snapshot())
        } else {
            None
        };
        if self.plan_draft.is_some() {
            self.plan_draft = None;
            self.pending_goal = None;
        }
        // /init is a reset flow — clear the conversation (including welcome).
        if task_kind == ConfigTaskKind::Init {
            self.blocks.clear();
        }
        self.config_task = Some(task);
        self.push_block(ConversationBlock::status(config_task_start_message(
            task_kind,
        )));
        if let Some(snapshot) = overview {
            self.push_block(ConversationBlock::config_overview(snapshot));
        }
        self.apply_config_prompt(prompt);
        self.interaction = InteractionUi::TextInput;
        self.focus_zone = FocusZone::Input;
    }

    fn submit_config_answer(&mut self, session: &mut ReplSession, input: String) -> Result<()> {
        let Some(task) = self.config_task.as_mut() else {
            return Ok(());
        };

        let task_kind = task.kind();
        let save_target = task.save_target();
        let outcome = task.submit(&input);

        match outcome {
            ConfigTaskOutcome::Next { accepted, prompt } => {
                drop(accepted);
                self.refresh_config_overview();
                self.apply_config_prompt(prompt);
            }
            ConfigTaskOutcome::Invalid { message, prompt } => {
                self.push_block(ConversationBlock::error(message));
                self.refresh_config_overview();
                self.apply_config_prompt(prompt);
            }
            ConfigTaskOutcome::Cancelled { message } => {
                drop(message);
                if let Ok(settings) = session.settings.read() {
                    crate::set_locale(&settings.ui.locale);
                }
                self.config_task = None;
                self.config_side_panel = None;
                self.restore_text_input();
                self.push_block(ConversationBlock::status(config_task_cancelled_message(
                    task_kind,
                )));
            }
            ConfigTaskOutcome::Saved {
                settings,
                providers,
                assignment,
                message,
            } => {
                drop(message);
                self.persist_config_apply(
                    &settings,
                    &providers,
                    &assignment,
                    save_target,
                    session,
                )?;
                self.config_task = None;
                self.config_side_panel = None;
                self.restore_text_input();
                self.push_block(ConversationBlock::status(config_task_saved_message(
                    task_kind,
                )));
            }
            ConfigTaskOutcome::Applied {
                settings,
                providers,
                assignment,
                prompt,
            } => {
                self.persist_config_apply(
                    &settings,
                    &providers,
                    &assignment,
                    save_target,
                    session,
                )?;
                self.refresh_config_overview();
                self.apply_config_prompt(prompt);
            }
        }

        Ok(())
    }

    /// Persist a config apply/save: write providers.toml, model.toml, settings,
    /// update session models + locale + runtime bridge. Shared by `Applied` and `Saved`.
    fn persist_config_apply(
        &mut self,
        settings: &runtime_config::Settings,
        providers: &runtime_config::ProviderConfig,
        assignment: &runtime_config::ModelAssignmentConfig,
        save_target: ConfigSaveTarget,
        session: &mut ReplSession,
    ) -> Result<()> {
        save_workspace_providers(providers)?;
        save_workspace_model_assignment(assignment)?;
        session.models = providers
            .model_library
            .models
            .iter()
            .filter(|entry| entry.enabled)
            .map(|entry| entry.model_name.clone())
            .collect();
        match save_target {
            ConfigSaveTarget::Project => settings.save_to_project_config()?,
        }
        let locale = settings.ui.locale.clone();
        *session.settings.write().unwrap() = settings.clone();
        crate::set_locale(&locale);
        session.rebuild_runtime_bridge();
        self.workspace_status = WorkspaceStatus::load();
        self.last_workspace_refresh = Instant::now();
        Ok(())
    }

    fn apply_config_prompt(&mut self, prompt: ConfigPrompt) {
        let ConfigPrompt {
            message,
            input,
            side_panel,
        } = prompt;
        if side_panel.is_none() {
            self.push_block(ConversationBlock::decision(message));
        }
        self.config_side_panel = side_panel;
        self.prompt_input = Some(input);
        self.input.clear();
    }

    fn set_config_prompt(&mut self, prompt: ConfigPrompt) {
        self.config_side_panel = prompt.side_panel;
        self.prompt_input = Some(prompt.input);
        self.input.clear();
    }

    fn show_config_exit_prompt(&mut self) {
        self.prompt_input = None;
        self.input.clear();
        self.push_block(ConversationBlock::decision(
            "Configuration has unsaved changes. Exit without saving?",
        ));
        self.interaction = InteractionUi::Decision(DecisionState::config_exit());
        self.focus_zone = FocusZone::Input;
    }

    fn dismiss_config_exit_prompt(&mut self) {
        if let Some(task) = self.config_task.as_ref() {
            let prompt = task.prompt();
            self.set_config_prompt(prompt);
            self.interaction = InteractionUi::TextInput;
            self.focus_zone = FocusZone::Input;
        } else {
            self.restore_text_input();
        }
    }

    /// Show tool confirmation prompt to user.
    /// Creates a PromptInputState with Approve/Deny choices.
    fn show_tool_confirmation(&mut self, confirmation: ToolConfirmationState) {
        let title = format!("Tool '{}' requires confirmation", confirmation.tool_name);
        let help = format!(
            "Tool: {} | ID: {} | Args: {}",
            confirmation.tool_name, confirmation.tool_call_id, confirmation.details
        );

        let choices = vec![
            PromptChoice::new("Approve", "approve"),
            PromptChoice::new("Deny", "deny"),
        ];

        let prompt_input =
            PromptInputState::new(title, PromptInputKind::SingleSelect, choices, help);

        self.pending_tool_confirmation = Some(confirmation);
        self.prompt_input = Some(prompt_input);
        self.input.clear();
        self.focus_zone = FocusZone::Input;
    }

    /// Handle tool confirmation response.
    fn handle_tool_confirmation_response(
        &mut self,
        response: &str,
    ) -> Option<(RunRef, String, bool)> {
        let approved = match response {
            "approve" => true,
            "deny" => false,
            _ => return None,
        };

        let confirmation = self.pending_tool_confirmation.take()?;
        let run_ref = confirmation.run_ref.clone();
        let tool_call_id = confirmation.tool_call_id.clone();

        // Clear the prompt input
        self.prompt_input = None;
        self.pending_tool_confirmation = None;
        self.restore_text_input();

        // Add confirmation result to conversation
        if approved {
            self.push_block(ConversationBlock::request(format!(
                "✓ Tool '{}' approved",
                confirmation.tool_name
            )));
        } else {
            self.push_block(ConversationBlock::error(format!(
                "✗ Tool '{}' denied",
                confirmation.tool_name
            )));
        }

        Some((run_ref, tool_call_id, approved))
    }

    fn begin_plan_draft(&mut self, goal: &str) {
        self.pending_goal = Some(goal.to_string());
        self.plans.clear();
        self.plan_draft = Some(PlanDraft::new(goal));
        self.record_input_request(goal);
        self.push_block(ConversationBlock::understanding(t!(
            "workspace.plan_draft.started",
            goal = goal
        )));
    }

    fn add_plan_detail(&mut self, detail: &str) {
        if let Some(draft) = self.plan_draft.as_mut() {
            draft.add_user_turn(detail);
        }
    }

    fn start_plan_controller_wait(&mut self) {
        self.prompt_input = None;
        self.input.clear();
        self.interaction = InteractionUi::TextInput;
        self.focus_zone = FocusZone::Input;
        self.push_block(ConversationBlock::execution(""));
    }

    fn apply_plan_controller_response(&mut self, response: String) {
        let Some(goal) = self.plan_draft.as_ref().map(|draft| draft.goal.clone()) else {
            self.restore_text_input();
            return;
        };

        match parse_plan_controller_output(&response, &goal) {
            PlanControllerOutput::NeedDetails(clarification) => {
                if let Some(draft) = self.plan_draft.as_mut() {
                    draft.add_assistant_turn(clarification.transcript_text());
                }
                self.convert_last_execution_or_push(
                    crate::tui::state::ConversationBlockType::Decision,
                    sanitize_for_tui(&clarification.question),
                );
                self.prompt_input = Some(clarification.to_prompt_input());
                self.interaction = InteractionUi::TextInput;
                self.focus_zone = FocusZone::Input;
            }
            PlanControllerOutput::PlanReady(proposal, nodes) => {
                let proposal = if proposal.trim().is_empty() {
                    response
                } else {
                    proposal
                };
                if let Some(draft) = self.plan_draft.as_mut() {
                    draft.add_assistant_turn(proposal.clone());
                    draft.set_proposal(proposal.clone(), nodes);
                }
                self.convert_last_execution_or_push(
                    crate::tui::state::ConversationBlockType::PlanProposal,
                    sanitize_for_tui(&proposal),
                );
                self.push_block(ConversationBlock::decision(t!(
                    "workspace.decision.description"
                )));
                self.interaction = InteractionUi::Decision(DecisionState::plan_approval());
                self.focus_zone = FocusZone::Input;
            }
        }
    }

    fn activate_draft_plan(&mut self) -> bool {
        let Some(draft) = self.plan_draft.take() else {
            return false;
        };
        if draft.nodes.is_empty() {
            return false;
        }

        self.pending_goal = Some(draft.goal);
        self.plans = draft
            .nodes
            .into_iter()
            .map(|mut node| {
                node.status = PlanNodeStatus::Pending;
                node
            })
            .collect();
        self.push_block(ConversationBlock::decision(t!(
            "workspace.plan_draft.approved"
        )));
        self.restore_text_input();
        true
    }

    fn cancel_plan_draft(&mut self) {
        self.plan_draft = None;
        self.pending_goal = None;
        self.plans.clear();
        self.push_block(ConversationBlock::decision(t!(
            "workspace.plan_draft.cancelled"
        )));
        self.restore_text_input();
    }

    fn close_plan(&mut self) {
        self.plans.clear();
        self.pending_goal = None;
        self.plan_draft = None;
        self.restore_text_input();
        if self.focus_zone == FocusZone::Plans {
            self.focus_zone = FocusZone::Input;
        }
    }

    fn next_pending_plan_node_index(&self) -> Option<usize> {
        self.plans
            .iter()
            .position(|node| node.status == PlanNodeStatus::Pending)
    }

    fn plan_step_prompt(&self, index: usize) -> Option<String> {
        let goal = self.pending_goal.as_deref()?;
        Some(build_plan_step_prompt(goal, &self.plans, index))
    }

    fn start_plan_step(&mut self, index: usize) {
        if let Some(node) = self.plans.get_mut(index) {
            node.status = PlanNodeStatus::Running;
        }
        self.push_block(ConversationBlock::decision(t!(
            "workspace.plan_step.started",
            step = index + 1,
            title = self
                .plans
                .get(index)
                .map(|node| node.title.as_str())
                .unwrap_or_default()
        )));
        self.push_block(ConversationBlock::execution(""));
        self.interaction = InteractionUi::TextInput;
    }

    fn finish_plan_step(&mut self, index: usize, response: &str) {
        let content = sanitize_for_tui(response);
        self.convert_last_execution_or_push(
            crate::tui::state::ConversationBlockType::Result,
            content,
        );
        if let Some(node) = self.plans.get_mut(index) {
            node.status = PlanNodeStatus::Completed;
            node.evidence = vec![t!("workspace.model_response_evidence").to_string()];
        }

        if self.next_pending_plan_node_index().is_none() {
            self.push_block(ConversationBlock::result(t!("workspace.plan_completed")));
            self.interaction = InteractionUi::Decision(DecisionState::plan_completion());
            self.focus_zone = FocusZone::Input;
        } else {
            self.restore_text_input();
        }
    }

    fn fail_plan_step(&mut self, index: usize, message: &str) {
        self.convert_last_execution_or_push(
            crate::tui::state::ConversationBlockType::Error,
            sanitize_for_tui(message),
        );
        if let Some(node) = self.plans.get_mut(index) {
            node.status = PlanNodeStatus::Failed;
        }
        self.restore_text_input();
    }

    fn convert_last_execution_or_push(
        &mut self,
        block_type: crate::tui::state::ConversationBlockType,
        content: String,
    ) {
        if let Some(block) = self.blocks.last_mut() {
            if block.is_streaming() || block.is_execution() {
                block.convert_to(block_type, content);
                self.stick_conversation_to_latest();
                return;
            }
        }
        self.push_block(match block_type {
            crate::tui::state::ConversationBlockType::Decision => {
                ConversationBlock::decision(content)
            }
            crate::tui::state::ConversationBlockType::PlanProposal => {
                ConversationBlock::plan_proposal(content)
            }
            crate::tui::state::ConversationBlockType::Result => ConversationBlock::result(content),
            crate::tui::state::ConversationBlockType::Error => ConversationBlock::error(content),
            _ => ConversationBlock::result(content),
        });
    }

    fn start_execution(&mut self, mode: ExecutionMode) {
        self.push_block(ConversationBlock::execution(""));

        if mode == ExecutionMode::Plan {
            if let Some(index) = self.next_pending_plan_node_index() {
                if let Some(node) = self.plans.get_mut(index) {
                    node.status = PlanNodeStatus::Running;
                }
            }
        } else {
            self.plans.clear();
            if self.focus_zone == FocusZone::Plans {
                self.focus_zone = FocusZone::Input;
            }
        }

        self.interaction = InteractionUi::TextInput;
    }

    fn show_execution_interrupt_prompt(&mut self) {
        self.push_block(ConversationBlock::decision(t!(
            "workspace.execution_interrupt.description"
        )));
        self.interaction = InteractionUi::Decision(DecisionState::execution_interrupt());
        self.focus_zone = FocusZone::Input;
    }

    fn dismiss_execution_interrupt_prompt(&mut self) {
        if matches!(
            &self.interaction,
            InteractionUi::Decision(decision) if decision.kind == DecisionKind::ExecutionInterrupt
        ) {
            self.interaction = InteractionUi::TextInput;
        }
    }

    fn interrupt_execution(&mut self) {
        let content = t!("workspace.execution_interrupted").to_string();
        if let Some(block) = self.blocks.last_mut() {
            if block.is_streaming() || block.is_execution() {
                block.convert_to(crate::tui::state::ConversationBlockType::Result, content);
            } else {
                self.push_block(ConversationBlock::result(content));
            }
        } else {
            self.push_block(ConversationBlock::result(content));
        }
        self.stick_conversation_to_latest();

        for node in &mut self.plans {
            if node.status == PlanNodeStatus::Running {
                node.status = PlanNodeStatus::Cancelled;
            }
        }
        self.restore_text_input();
    }

    fn finish_execution(&mut self, response: &str, mode: ExecutionMode) {
        let content = sanitize_for_tui(response);
        if let Some(block) = self.blocks.last_mut() {
            if block.is_streaming() || block.is_execution() {
                block.convert_to(crate::tui::state::ConversationBlockType::Result, content);
            } else {
                self.push_block(ConversationBlock::result(content));
            }
        } else {
            self.push_block(ConversationBlock::result(content));
        }
        self.stick_conversation_to_latest();

        if mode == ExecutionMode::Plan {
            if let Some(index) = self
                .plans
                .iter()
                .position(|node| node.status == PlanNodeStatus::Running)
            {
                if let Some(node) = self.plans.get_mut(index) {
                    node.status = PlanNodeStatus::Completed;
                    node.evidence = vec![t!("workspace.model_response_evidence").to_string()];
                }
            }
            if self.next_pending_plan_node_index().is_none() && !self.plans.is_empty() {
                self.push_block(ConversationBlock::result(t!("workspace.plan_completed")));
                self.interaction = InteractionUi::Decision(DecisionState::plan_completion());
                self.focus_zone = FocusZone::Input;
            } else {
                self.restore_text_input();
            }
        } else {
            self.restore_text_input();
        }
    }

    fn fail_execution(&mut self, message: &str) {
        let content = sanitize_for_tui(message);
        if let Some(block) = self.blocks.last_mut() {
            if block.is_streaming() || block.is_execution() {
                block.convert_to(crate::tui::state::ConversationBlockType::Error, content);
            } else {
                self.push_block(ConversationBlock::error(content));
            }
        } else {
            self.push_block(ConversationBlock::error(content));
        }
        self.stick_conversation_to_latest();
        for node in &mut self.plans {
            if node.status == PlanNodeStatus::Running {
                node.status = PlanNodeStatus::Failed;
            }
        }
        self.restore_text_input();
    }

    /// Append streaming delta text to the last block (or create a new Streaming block).
    fn update_streaming_text(&mut self, delta: &str) {
        if let Some(block) = self.blocks.last_mut() {
            if block.is_streaming() {
                block.append_content(delta);
                self.stick_conversation_to_latest();
                return;
            }
            if block.is_execution() {
                block.convert_to(
                    crate::tui::state::ConversationBlockType::Streaming,
                    delta.to_string(),
                );
                self.stick_conversation_to_latest();
                return;
            }
        }
        self.push_block(ConversationBlock::streaming(delta));
    }

    /// Convert the streaming block to a finalized result block.
    fn finish_streaming(&mut self, full_text: &str, mode: ExecutionMode) {
        if let Some(block) = self.blocks.last_mut() {
            if block.is_streaming() || block.is_execution() {
                block.convert_to(
                    crate::tui::state::ConversationBlockType::Result,
                    sanitize_for_tui(full_text),
                );
            }
        }
        self.stick_conversation_to_latest();
        if mode == ExecutionMode::Plan {
            if let Some(index) = self
                .plans
                .iter()
                .position(|node| node.status == PlanNodeStatus::Running)
            {
                if let Some(node) = self.plans.get_mut(index) {
                    node.status = PlanNodeStatus::Completed;
                    node.evidence = vec![t!("workspace.model_response_evidence").to_string()];
                }
            }
            if self.next_pending_plan_node_index().is_none() && !self.plans.is_empty() {
                self.push_block(ConversationBlock::result(t!("workspace.plan_completed")));
                self.interaction = InteractionUi::Decision(DecisionState::plan_completion());
                self.focus_zone = FocusZone::Input;
            } else {
                self.restore_text_input();
            }
        } else {
            self.restore_text_input();
        }
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
        self.prompt_input = None;
        self.input.clear();
    }

    fn stick_conversation_to_latest(&mut self) {
        self.conv_scroll.auto_scroll = true;
    }

    fn push_block(&mut self, block: ConversationBlock) {
        self.stick_conversation_to_latest();
        self.blocks.push(block);
        let excess = self.blocks.len().saturating_sub(32);
        if excess > 0 {
            self.blocks.drain(0..excess);
        }
    }

    fn record_tool_call_started(&mut self, value: &serde_json::Value) {
        let tool = truncate_chars(&tool_display_name(value), 160);
        let content = t!("workspace.tool.started", tool = tool).to_string();

        if let Some(block) = self.blocks.last_mut() {
            if block.is_execution() && block.content.trim().is_empty() {
                *block = ConversationBlock::status(content);
                self.stick_conversation_to_latest();
                return;
            }
        }

        self.push_block(ConversationBlock::status(content));
    }

    fn record_tool_call_completed(&mut self, value: &serde_json::Value) {
        let tool = truncate_chars(&tool_display_name(value), 160);
        let success = value
            .get("success")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let output = value
            .get("output")
            .and_then(serde_json::Value::as_str)
            .map(sanitize_for_tui)
            .filter(|text| !text.is_empty())
            .map(|text| truncate_chars(&text, 1200));

        let mut content = if success {
            t!("workspace.tool.completed", tool = tool).to_string()
        } else {
            t!("workspace.tool.failed", tool = tool).to_string()
        };
        if let Some(output) = output {
            content.push('\n');
            content.push_str(&output);
        }

        if success {
            self.push_block(ConversationBlock::status(content));
        } else {
            self.push_block(ConversationBlock::error(content));
        }
    }

    fn record_input_request(&mut self, input: impl Into<String>) {
        self.push_block(ConversationBlock::request(input));
    }

    /// Replace the most recent config-overview block with a fresh snapshot,
    /// or push a new one if none exists (e.g. evicted past the block cap).
    fn refresh_config_overview(&mut self) {
        let snapshot = match self.config_task.as_ref() {
            Some(task) if task.kind() == ConfigTaskKind::Config => task.overview_snapshot(),
            _ => return,
        };
        let new_block = ConversationBlock::config_overview(snapshot);
        match self
            .blocks
            .iter()
            .rposition(|b| b.block_type == ConversationBlockType::ConfigOverview)
        {
            Some(i) => self.blocks[i].content = new_block.content,
            None => self.push_block(new_block),
        }
    }

    fn toggle_global_expand(&mut self) {
        if self.global_expanded {
            // Collapse all: clear expanded blocks and set global_expanded to false
            self.expanded_blocks.clear();
            self.global_expanded = false;
        } else {
            // Expand all: set global_expanded to true
            self.global_expanded = true;
        }
    }

    fn handle_conversation_click(&mut self, _col: u16, row: u16) {
        // Map the click row to a block ID
        // Note: row is relative to the terminal, we need to adjust for the conversation area offset
        let inner_row = row.saturating_sub(self.layout_rects.conversation.y + 1); // +1 for border
        let adjusted_row = inner_row.saturating_add(self.conv_scroll.offset);

        // Find which block this row belongs to
        for (block_id, (start, end)) in &self.block_row_map {
            if adjusted_row >= *start && adjusted_row < *end {
                // Check if this block can be folded
                if let Some(block) = self.blocks.iter().find(|b| &b.id == block_id) {
                    let is_empty_execution = block.block_type == ConversationBlockType::Execution
                        && block.content.trim().is_empty();
                    let is_welcome = block.block_type == ConversationBlockType::Welcome;
                    let is_config = block.block_type == ConversationBlockType::ConfigOverview;

                    // Don't fold welcome, config overview, or empty execution blocks
                    if !is_empty_execution && !is_welcome && !is_config {
                        let content_lines: Vec<&str> = block.content.lines().collect();
                        let total_lines = 1 + content_lines.len();

                        if total_lines > MAX_COLLAPSED_LINES {
                            self.toggle_block_expanded(block_id.clone());
                        }
                    }
                }
                break;
            }
        }
    }

    fn toggle_block_expanded(&mut self, block_id: String) {
        if self.expanded_blocks.contains(&block_id) {
            self.expanded_blocks.remove(&block_id);
        } else {
            self.expanded_blocks.insert(block_id);
        }
    }
}

#[cfg(test)]
mod tool_confirmation_tests {
    use super::*;
    use protocol_interface::core::RunRef;

    #[test]
    fn test_tool_confirmation_state_creation() {
        let confirmation = ToolConfirmationState {
            tool_call_id: "test-123".to_string(),
            tool_name: "shell".to_string(),
            details: "ls -la".to_string(),
            run_ref: RunRef::new(),
        };

        assert_eq!(confirmation.tool_call_id, "test-123");
        assert_eq!(confirmation.tool_name, "shell");
        assert_eq!(confirmation.details, "ls -la");
    }

    #[test]
    fn test_show_tool_confirmation_sets_prompt_input() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-1".to_string(),
            tool_name: "write_file".to_string(),
            details: "path: /tmp/test.txt".to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);

        // Should have prompt_input set
        assert!(state.prompt_input.is_some());

        // Should have pending_tool_confirmation set
        assert!(state.pending_tool_confirmation.is_some());
        assert_eq!(
            state.pending_tool_confirmation.as_ref().unwrap().tool_name,
            "write_file"
        );
    }

    #[test]
    fn test_handle_tool_confirmation_response_approve() {
        let mut state = WorkspaceState::new(vec![]);
        let run_ref = RunRef::new();
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-2".to_string(),
            tool_name: "shell".to_string(),
            details: "echo hello".to_string(),
            run_ref: run_ref.clone(),
        };

        state.show_tool_confirmation(confirmation);

        // Simulate approve response
        let result = state.handle_tool_confirmation_response("approve");
        assert!(result.is_some());

        let (returned_run_ref, tool_call_id, approved) = result.unwrap();
        assert_eq!(returned_run_ref, run_ref);
        assert_eq!(tool_call_id, "tc-2");
        assert!(approved);

        // State should be cleared
        assert!(state.prompt_input.is_none());
        assert!(state.pending_tool_confirmation.is_none());
    }

    #[test]
    fn test_handle_tool_confirmation_response_deny() {
        let mut state = WorkspaceState::new(vec![]);
        let run_ref = RunRef::new();
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-3".to_string(),
            tool_name: "edit_file".to_string(),
            details: "find: old, replace: new".to_string(),
            run_ref: run_ref.clone(),
        };

        state.show_tool_confirmation(confirmation);

        // Simulate deny response
        let result = state.handle_tool_confirmation_response("deny");
        assert!(result.is_some());

        let (returned_run_ref, tool_call_id, approved) = result.unwrap();
        assert_eq!(returned_run_ref, run_ref);
        assert_eq!(tool_call_id, "tc-3");
        assert!(!approved);

        // State should be cleared
        assert!(state.prompt_input.is_none());
        assert!(state.pending_tool_confirmation.is_none());
    }

    #[test]
    fn test_handle_tool_confirmation_response_invalid() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-4".to_string(),
            tool_name: "shell".to_string(),
            details: "ls".to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);

        // Invalid response should return None
        let result = state.handle_tool_confirmation_response("invalid");
        assert!(result.is_none());

        // State should remain (confirmation still pending)
        assert!(state.prompt_input.is_some());
        assert!(state.pending_tool_confirmation.is_some());
    }

    #[test]
    fn test_handle_tool_confirmation_no_pending() {
        let mut state = WorkspaceState::new(vec![]);

        // No pending confirmation
        let result = state.handle_tool_confirmation_response("approve");
        assert!(result.is_none());
    }
}
