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

// Re-export for testing module access
pub(crate) use events::{ExecutionMode, PlanPermissionMode, WorkspaceAction};

use std::io::{self, Write};
use std::time::{Duration, Instant};

use anyhow::Result;
use base64::Engine as _;
use crossterm::event::{
    self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers, ModifierKeyCode, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::event::{DisableMouseCapture, EnableMouseCapture};
use crossterm::execute;
use protocol_interface::core::{CoreEventKind, CoreEventPayload, LoopPolicy, RunRef, RuntimeMode};
use ratatui::prelude::*;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use runtime_config::ModelAssignmentRole;
use rust_i18n::t;

use crate::repl::{
    completion, init_required_message, missing_runtime_requirements,
    model_assignment_readiness_issues, model_assignment_required_message, ReplSession,
};
use crate::tui::state::{
    AgentHeader, AgentTeamState, ConversationBlock, ConversationBlockType, InteractionMode,
    MainTab, PlanNode, PlanNodeStatus, WorkspaceStatus,
};
use crate::tui::TuiApp;

use config_task::{
    ConfigPrompt, ConfigSaveTarget, ConfigSidePanel, ConfigTask, ConfigTaskKind, ConfigTaskOutcome,
};
use events::{CommandOutcome, DecisionKind};
use helpers::{sanitize_for_tui, truncate_chars};
use interaction::{
    DecisionState, InputBuffer, InteractionState, InteractionUi, PromptChoice, PromptInputAction,
    PromptInputKind, PromptInputState,
};

const WORKSPACE_POLL_MS: u64 = 100;
const GIT_REFRESH_SECS: u64 = 2;
const COPY_FEEDBACK_SECS: u64 = 2;

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

    /// Write a `.alius/config/` tree under the current dir with the given
    /// providers/model-library/assignment TOML bodies (empty string = skip).
    fn write_project_config(providers_toml: &str, model_toml: &str) {
        let config_dir = std::path::Path::new(".alius/config");
        std::fs::create_dir_all(config_dir).unwrap();
        if !providers_toml.is_empty() {
            std::fs::write(config_dir.join("providers.toml"), providers_toml).unwrap();
        }
        if !model_toml.is_empty() {
            std::fs::write(config_dir.join("model.toml"), model_toml).unwrap();
        }
    }

    #[test]
    fn assigned_welcome_model_falls_back_to_tier_when_library_misses() {
        // Execute is set via the tier but NOT present in the model library.
        // Before the fix this returned None ("Not configured").
        let (_dir, _guard) = enter_temp_cwd();
        write_project_config(
            r#"[router]
strategy = "tiered"
default_tier = "medium"
fallback_tier = "medium"

[tiers.light]
description = "Plan"
provider = "bigmodel"
model = ""

[tiers.medium]
description = "Execute"
provider = "bigmodel"
model = "gpt-4o"

[tiers.high]
description = "Review"
provider = "bigmodel"
model = ""

[providers.bigmodel]
enabled = true
kind = "openai-compatible"
base_url = "https://open.bigmodel.cn/api/coding/paas/v4"
api_key_env = "BIGMODEL_API_KEY"

[model_library]
models = []
"#,
            r#"schema_version = "0.1"
[assignment]
plan = ""
execute = ""
review = ""
"#,
        );

        let cwd = std::env::current_dir().unwrap();
        let snapshot = runtime_config::load_project_config(&cwd).unwrap();

        // Library miss + tier set -> fallback returns the tier model.
        assert_eq!(
            assigned_welcome_model(&snapshot, ModelAssignmentRole::Execute).as_deref(),
            Some("BigModel(gpt-4o)")
        );
        // Plan/Review tiers empty -> still None (genuinely unconfigured).
        assert!(assigned_welcome_model(&snapshot, ModelAssignmentRole::Plan).is_none());
        assert!(assigned_welcome_model(&snapshot, ModelAssignmentRole::Review).is_none());
    }

    #[test]
    fn assigned_welcome_model_prefers_library_match_over_tier() {
        let (_dir, _guard) = enter_temp_cwd();
        write_project_config(
            r#"[router]
strategy = "tiered"
default_tier = "medium"
fallback_tier = "medium"

[tiers.light]
description = "Plan"
provider = "bigmodel"
model = ""

[tiers.medium]
description = "Execute"
provider = "bigmodel"
model = ""

[tiers.high]
description = "Review"
provider = "deepseek"
model = "stale-name"

[providers.bigmodel]
enabled = true
kind = "openai-compatible"
base_url = "https://open.bigmodel.cn/api/coding/paas/v4"
api_key_env = "BIGMODEL_API_KEY"

[[model_library.models]]
id = "bigmodel-glm-4.5"
display_name = "glm-4.5"
provider = "bigmodel"
base_url = "https://open.bigmodel.cn/api/coding/paas/v4"
model_name = "glm-4.5"
reasoning_note = "Standard Reasoning"
enabled = true
"#,
            r#"schema_version = "0.1"
[assignment]
plan = ""
execute = ""
review = "bigmodel-glm-4.5"
"#,
        );

        let cwd = std::env::current_dir().unwrap();
        let snapshot = runtime_config::load_project_config(&cwd).unwrap();

        // Assignment id matches a library entry -> use it (not the tier).
        assert_eq!(
            assigned_welcome_model(&snapshot, ModelAssignmentRole::Review).as_deref(),
            Some("BigModel(glm-4.5)")
        );
    }

    #[test]
    fn assigned_welcome_model_returns_none_when_everything_empty() {
        let (_dir, _guard) = enter_temp_cwd();
        write_project_config(
            r#"[router]
strategy = "tiered"
default_tier = "medium"
fallback_tier = "medium"

[tiers.light]
description = "Plan"
provider = "bigmodel"
model = ""

[tiers.medium]
description = "Execute"
provider = "bigmodel"
model = ""

[tiers.high]
description = "Review"
provider = "bigmodel"
model = ""

[providers.bigmodel]
enabled = true
kind = "openai-compatible"
base_url = "https://open.bigmodel.cn/api/coding/paas/v4"
api_key_env = "BIGMODEL_API_KEY"

[model_library]
models = []
"#,
            r#"schema_version = "0.1"
[assignment]
plan = ""
execute = ""
review = ""
"#,
        );

        let cwd = std::env::current_dir().unwrap();
        let snapshot = runtime_config::load_project_config(&cwd).unwrap();

        for role in ModelAssignmentRole::all() {
            assert!(assigned_welcome_model(&snapshot, role).is_none());
        }
    }

    #[test]
    fn selection_extracts_conversation_text() {
        let mut state = WorkspaceState::new(Vec::new());
        state.layout_rects.conversation = Rect::new(0, 0, 40, 8);
        state.conv_plain_lines = vec![PlainTextLine {
            text: "hello world".to_string(),
            row: 0,
        }];

        let selected = state.selected_text_for_selection(&TextSelection {
            panel: PanelType::Conversation,
            start: Position { x: 1, y: 1 },
            end: Position { x: 6, y: 1 },
        });

        assert_eq!(selected.as_deref(), Some("hello"));
    }

    #[test]
    fn selection_extracts_reversed_multiline_text() {
        let mut state = WorkspaceState::new(Vec::new());
        state.layout_rects.conversation = Rect::new(0, 0, 40, 8);
        state.conv_plain_lines = vec![
            PlainTextLine {
                text: "alpha".to_string(),
                row: 0,
            },
            PlainTextLine {
                text: "beta".to_string(),
                row: 1,
            },
        ];

        let selected = state.selected_text_for_selection(&TextSelection {
            panel: PanelType::Conversation,
            start: Position { x: 5, y: 2 },
            end: Position { x: 3, y: 1 },
        });

        assert_eq!(selected.as_deref(), Some("pha\nbeta"));
    }

    #[test]
    fn backtab_toggles_plan_to_chat_mode() {
        let mut state = WorkspaceState::new(Vec::new());
        assert_eq!(state.mode, InteractionMode::Plan);

        let action = state.handle_key(key(KeyCode::BackTab), &[]);
        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.mode, InteractionMode::Chat);
    }

    #[test]
    fn backtab_cycles_chat_bypass_plan_modes() {
        let mut state = WorkspaceState::new(Vec::new());

        state.handle_key(key(KeyCode::BackTab), &[]);
        assert_eq!(state.mode, InteractionMode::Chat);

        state.handle_key(key(KeyCode::BackTab), &[]);
        assert_eq!(state.mode, InteractionMode::Bypass);

        let action = state.handle_key(key(KeyCode::BackTab), &[]);
        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.mode, InteractionMode::Plan);
    }

    #[test]
    fn mode_toggle_preserves_input_text() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "hello world");
        assert_eq!(state.input.value(), "hello world");

        state.handle_key(key(KeyCode::BackTab), &[]);
        assert_eq!(state.input.value(), "hello world");
        assert_eq!(state.mode, InteractionMode::Chat);
    }

    #[test]
    fn execution_mode_maps_to_matching_runtime_mode() {
        assert_eq!(
            runtime_mode_for_execution(ExecutionMode::Chat),
            RuntimeMode::Chat
        );
        assert_eq!(
            runtime_mode_for_execution(ExecutionMode::Plan),
            RuntimeMode::Plan
        );
        assert_eq!(
            runtime_mode_for_execution(ExecutionMode::Bypass),
            RuntimeMode::Bypass
        );
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
        assert!(state.blocks.iter().any(|block| block
            .title
            .as_deref()
            .is_some_and(|t| t.contains("shell: git clone"))));

        state.record_tool_call_completed(&completed);
        let block = state.blocks.last().expect("tool completion block");
        assert!(block
            .title
            .as_deref()
            .is_some_and(|t| t.contains("shell: git clone")));
        assert!(block.content.contains("cloned"));
    }

    #[test]
    fn run_local_service_tool_result_summarizes_verified_url() {
        let mut state = WorkspaceState::new(Vec::new());
        let completed = serde_json::json!({
            "id": "call-1",
            "name": "run_local_service",
            "args": {
                "command": "npm run dev"
            },
            "success": true,
            "output": serde_json::json!({
                "tool": "run_local_service",
                "ready": true,
                "url": "http://127.0.0.1:5173",
                "service_id": null,
                "pid": 123,
                "kept_running": false,
                "stopped": true,
                "logs_tail": ["stdout: Local: http://127.0.0.1:5173"]
            }).to_string()
        });

        state.record_tool_call_completed(&completed);
        let block = state.blocks.last().expect("tool completion block");

        assert!(block
            .content
            .contains("本地服务 URL: http://127.0.0.1:5173"));
        assert!(block.content.contains("服务状态: 已验证并停止"));
        assert!(block.content.contains("日志摘要:"));
        assert!(!block.content.contains("\"tool\":\"run_local_service\""));
    }

    #[test]
    fn plan_step_prompt_requires_search_tests_and_local_service_evidence() {
        let plans = vec![PlanNode {
            id: "p1".to_string(),
            title: "Fix local frontend bug".to_string(),
            status: PlanNodeStatus::Pending,
            description: None,
            acceptance_criteria: Vec::new(),
            evidence: Vec::new(),
            owner: None,
        }];

        let prompt = build_plan_step_prompt("fix bug", &plans, 0);

        assert!(prompt.contains("search_code/read_file/list_dir"));
        assert!(prompt.contains("tests, checks, or build command"));
        assert!(prompt.contains("run_local_service"));
        assert!(prompt.contains("verified local service URL"));
        assert!(prompt.contains("whether that service was stopped"));
    }

    #[test]
    fn tab_cycles_ambiguous_root_command_matches() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/m");

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "/mode");
        assert_eq!(state.focus_zone, FocusZone::Input);

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "/model");
        assert_eq!(state.focus_zone, FocusZone::Input);
    }

    #[test]
    fn tab_cycles_ambiguous_subcommand_matches() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/session l");

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "/session list");
        assert_eq!(state.focus_zone, FocusZone::Input);

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), "/session load");
        assert_eq!(state.focus_zone, FocusZone::Input);
    }

    #[test]
    fn tab_does_not_complete_command_with_leading_space() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, " /he");

        let action = state.handle_key(key(KeyCode::Tab), &[]);

        assert!(matches!(action, WorkspaceAction::None));
        assert_eq!(state.input.value(), " /he");
        assert_eq!(state.focus_zone, FocusZone::Conversation);
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
    fn readiness_redirect_opens_model_pool_assignment() {
        rust_i18n::set_locale("en");
        let mut settings = runtime_config::Settings::default();
        settings.llm.model = "gpt-4o".to_string();
        settings.llm.api_key = Some("sk-test".to_string());
        settings.soul.role = runtime_config::SoulRole::new("default".to_string());
        let issues = vec![runtime_config::ModelAssignmentReadinessIssue {
            role: runtime_config::ModelAssignmentRole::Plan,
            model_id: None,
            kind: runtime_config::ModelAssignmentReadinessIssueKind::NotConfigured,
        }];
        let mut state = WorkspaceState::new(Vec::new());
        state.begin_plan_draft("build something");

        state.start_model_task_for_readiness(settings, &issues);

        assert!(state.plan_draft.is_none());
        assert!(state.config_task.is_some());
        let prompt = state.prompt_input.as_ref().unwrap();
        assert_eq!(prompt.scope_title.as_deref(), Some("Model Pool Management"));
        assert_eq!(prompt.title, "Model Assignment");
        assert!(prompt
            .choices
            .iter()
            .any(|choice| choice.label.contains("Plan Model")));
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

    #[test]
    fn command_matched_ignores_partial_root_command() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/he");

        assert!(state.command_hint(&[]).unwrap().contains("/help"));
        assert!(!state.command_matched());
    }

    #[test]
    fn command_matched_accepts_exact_root_command() {
        for command in [
            "/help", "/model", "/mode", "/session", "/review", "/memory", "/trace", "/confirm",
        ] {
            let mut state = WorkspaceState::new(Vec::new());
            type_text(&mut state, command);

            assert!(state.command_matched(), "{command} should be matched");
        }
    }

    #[test]
    fn command_matched_rejects_leading_space() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, " /help");

        assert!(state.command_hint(&[]).is_none());
        assert!(!state.command_matched());
    }

    #[test]
    fn command_matched_requires_exact_subcommand() {
        let mut state = WorkspaceState::new(Vec::new());
        type_text(&mut state, "/session l");
        assert!(!state.command_matched());

        state.input.clear();
        type_text(&mut state, "/session list");
        assert!(state.command_matched());

        state.input.clear();
        type_text(&mut state, "/mode p");
        assert!(!state.command_matched());

        state.input.clear();
        type_text(&mut state, "/mode plan");
        assert!(state.command_matched());
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
            match bridge.respond_confirmation(&run_ref, &tool_call_id, approved) {
                Ok(()) => {
                    state.complete_tool_confirmation(approved);
                }
                Err(e) => {
                    state.fail_tool_confirmation(e.to_string());
                    let _ = bridge.cancel(
                        &run_ref,
                        Some("tool confirmation delivery failed".to_string()),
                    );
                }
            }
        } else {
            state.fail_tool_confirmation("bridge unavailable".to_string());
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

    if !ensure_model_assignment_ready_or_open_model_pool(session, state) {
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
        InteractionMode::Chat => {
            state.record_input_request(trimmed);
            execute_goal(
                app,
                session,
                state,
                trimmed.to_string(),
                ExecutionMode::Chat,
            )
            .await?;
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

fn ensure_model_assignment_ready_or_open_model_pool(
    session: &ReplSession,
    state: &mut WorkspaceState,
) -> bool {
    let issues = model_assignment_readiness_issues(&session.workspace_root);
    if issues.is_empty() {
        return true;
    }

    let current_settings = session.settings.read().unwrap().clone();
    state.start_model_task_for_readiness(current_settings, &issues);
    false
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

    if key.code == KeyCode::BackTab && state.has_active_plan_execution() {
        state.toggle_plan_permission_mode();
        return ExecutionInputAction::None;
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
    if !ensure_model_assignment_ready_or_open_model_pool(session, state) {
        return Ok(());
    }

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

        let rt_mode = runtime_mode_for_execution(mode);

        match bridge.start_streaming(&prompt, rt_mode) {
            Ok((run_ref, mut event_rx)) => {
                // Stage B: Store run_ref for tool confirmation responses
                state.current_run_ref = Some(run_ref.clone());

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
                                    if let Some((run_ref, tool_call_id, approved)) =
                                        state.handle_tool_confirmation_response(&response)
                                    {
                                        match bridge.respond_confirmation(
                                            &run_ref,
                                            &tool_call_id,
                                            approved,
                                        ) {
                                            Ok(()) => {
                                                // Success: clear confirmation state
                                                state.complete_tool_confirmation(approved);
                                            }
                                            Err(e) => {
                                                // Fail-closed: show error, cancel run
                                                state.fail_tool_confirmation(e.to_string());
                                                let _ = bridge.cancel(
                                                    &run_ref,
                                                    Some(
                                                        "tool confirmation delivery failed"
                                                            .to_string(),
                                                    ),
                                                );
                                                interrupted = true;
                                                done = true;
                                            }
                                        }
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
    if !ensure_model_assignment_ready_or_open_model_pool(session, state) {
        return Ok(());
    }

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

fn runtime_mode_for_execution(mode: ExecutionMode) -> RuntimeMode {
    match mode {
        ExecutionMode::Chat => RuntimeMode::Chat,
        ExecutionMode::Plan => RuntimeMode::Plan,
        ExecutionMode::Bypass => RuntimeMode::Bypass,
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
    if !ensure_model_assignment_ready_or_open_model_pool(session, state) {
        return Ok(false);
    }

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

    let policy = state.plan_permission_mode.loop_policy();
    let (run_ref, mut event_rx) =
        match bridge.start_streaming_with_policy(&prompt, RuntimeMode::Plan, policy) {
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
                        if let Some((run_ref, tool_call_id, approved)) =
                            state.handle_tool_confirmation_response(&response)
                        {
                            match bridge.respond_confirmation(&run_ref, &tool_call_id, approved) {
                                Ok(()) => {
                                    state.complete_tool_confirmation(approved);
                                }
                                Err(e) => {
                                    state.fail_tool_confirmation(e.to_string());
                                    let _ = bridge.cancel(
                                        &run_ref,
                                        Some("tool confirmation delivery failed".to_string()),
                                    );
                                    interrupted = true;
                                    done = true;
                                }
                            }
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
    let assignment_issues = model_assignment_readiness_issues(&session.workspace_root);
    if !assignment_issues.is_empty() {
        return Err(anyhow::anyhow!(
            "{}",
            model_assignment_required_message(&assignment_issues)
        ));
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
- Inspect existing backend/frontend code first with search_code/read_file/list_dir when code context is needed.
- Use available tools to implement or verify the step; do not guess when local evidence can be gathered.
- After implementation or bug fixes, run the relevant tests, checks, or build command.
- If this step changes or fixes a locally runnable app/API, call run_local_service to verify the local service URL.
- If a required prerequisite is missing, stop and explain the blocker clearly.
- Return the concrete result, commands run, test/build outcome, verified local service URL when applicable, whether that service was stopped, and any files changed.
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
        .style(crate::tui::theme::base())
        .wrap(Wrap { trim: false })
        .scroll((scroll.offset, 0));
    // Clear stale glyphs before repaint (see conversation.rs for rationale).
    frame.render_widget(ratatui::widgets::Clear, inner);
    frame.render_widget(paragraph, inner);
}

// ---------------------------------------------------------------------------
// FocusZone, HoverZone, PanelScroll, LayoutRects
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FocusZone {
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

fn format_tool_completion_output(value: &serde_json::Value, raw_output: &str) -> String {
    let name = value
        .get("name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("tool");
    if name == "run_local_service" {
        if let Some(summary) = format_local_service_result(raw_output) {
            return summary;
        }
    }
    sanitize_for_tui(raw_output)
}

fn format_local_service_result(raw_output: &str) -> Option<String> {
    let output: serde_json::Value = serde_json::from_str(raw_output).ok()?;
    if output.get("tool").and_then(serde_json::Value::as_str) != Some("run_local_service") {
        return None;
    }
    let url = output
        .get("url")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("");
    if url.is_empty() {
        return None;
    }
    let kept_running = output
        .get("kept_running")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let stopped = output
        .get("stopped")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let mut lines = vec![format!("本地服务 URL: {url}")];
    if kept_running {
        lines.push("服务状态: 仍在运行".to_string());
        if let Some(service_id) = output.get("service_id").and_then(serde_json::Value::as_str) {
            lines.push(format!("Service ID: {service_id}"));
        }
    } else if stopped {
        lines.push("服务状态: 已验证并停止".to_string());
    } else {
        lines.push("服务状态: 已验证，停止状态未知".to_string());
    }
    if let Some(logs) = output
        .get("logs_tail")
        .and_then(serde_json::Value::as_array)
    {
        let mut visible_logs = logs
            .iter()
            .filter_map(serde_json::Value::as_str)
            .filter(|line| !line.trim().is_empty())
            .rev()
            .take(5)
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        visible_logs.reverse();
        if !visible_logs.is_empty() {
            lines.push("日志摘要:".to_string());
            lines.extend(visible_logs);
        }
    }
    Some(lines.join("\n"))
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
        // All three roles derive from the project config snapshot (model.toml
        // assignment + tiers), NOT from runtime ReplSession settings — so the
        // welcome page reflects what is persisted to disk, not a value that
        // may have been overridden at runtime via --model / env vars.
        model_plan: None,
        model_execute: None,
        model_review: None,
    };

    if initialized {
        if let Some(snapshot) = std::env::current_dir()
            .ok()
            .and_then(|cwd| runtime_config::load_project_config(&cwd).ok())
        {
            state.model_plan = assigned_welcome_model(&snapshot, ModelAssignmentRole::Plan);
            state.model_execute =
                assigned_welcome_model(&snapshot, ModelAssignmentRole::Execute);
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
    // Prefer an exact model-library match (the canonical source).
    if !model_id.is_empty() {
        if let Some(entry) = snapshot
            .providers
            .model_library
            .models
            .iter()
            .find(|entry| entry.enabled && entry.id == model_id)
        {
            return Some(format!(
                "{}({})",
                welcome_provider_name(&entry.provider),
                entry.model_name
            ));
        }
    }
    // Fallback: derive from the compatibility tier (light=Plan, medium=Execute,
    // high=Review). Covers legacy configs and models assigned before they were
    // added to the model library, so the welcome page no longer shows
    // "Not configured" for roles the user has actually set up.
    let tier = match role {
        ModelAssignmentRole::Plan => &snapshot.providers.tiers.light,
        ModelAssignmentRole::Execute => &snapshot.providers.tiers.medium,
        ModelAssignmentRole::Review => &snapshot.providers.tiers.high,
    };
    let model_name = tier.model.trim();
    if model_name.is_empty() {
        return None;
    }
    Some(format!(
        "{}({})",
        welcome_provider_name(&tier.provider),
        model_name
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

impl PlanPermissionMode {
    fn title(self) -> String {
        match self {
            Self::AcceptEdits => t!("workspace.mode.plan_accept_edits").to_string(),
            Self::BypassPermissions => t!("workspace.mode.plan_bypass_permissions").to_string(),
        }
    }

    fn loop_policy(self) -> LoopPolicy {
        match self {
            Self::AcceptEdits => LoopPolicy::plan_accept_edits(),
            Self::BypassPermissions => LoopPolicy::plan(),
        }
    }
}

/// Which panel a text selection targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PanelType {
    Conversation,
    Plans,
    Interaction,
}

/// An active mouse text selection.
#[derive(Debug, Clone)]
struct TextSelection {
    panel: PanelType,
    /// Selection anchor (where mouse was pressed).
    start: Position,
    /// Current mouse position (updated on drag).
    end: Position,
}

#[derive(Debug, Clone)]
struct CopyFeedback {
    message: String,
    is_error: bool,
    expires_at: Instant,
}

/// A plain-text line for selection extraction.
#[derive(Debug, Clone)]
struct PlainTextLine {
    text: String,
    /// Row index in the panel's inner area (0-based, before scroll).
    row: u16,
}

#[derive(Debug, Clone)]
struct CommandCompletionCycle {
    start: usize,
    matches: Vec<String>,
    selected: usize,
}

pub(crate) struct WorkspaceState {
    mode: InteractionMode,
    plan_permission_mode: PlanPermissionMode,
    active_tab: MainTab,
    blocks: Vec<ConversationBlock>,
    plans: Vec<PlanNode>,
    agent_team: Option<AgentTeamState>,
    input: InputBuffer,
    input_history: Vec<String>,
    input_history_cursor: Option<usize>,
    input_history_draft: String,
    command_completion_cycle: Option<CommandCompletionCycle>,
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
    /// Current run reference for tool confirmation (Stage B).
    current_run_ref: Option<protocol_interface::core::RunRef>,
    /// Active text selection (if any).
    selection: Option<TextSelection>,
    copy_feedback: Option<CopyFeedback>,
    /// Plain-text lines for the conversation panel (rebuilt each frame).
    conv_plain_lines: Vec<PlainTextLine>,
    /// Plain-text lines for the plans panel (rebuilt each frame).
    plans_plain_lines: Vec<PlainTextLine>,
    /// Plain-text lines for the interaction panel (rebuilt each frame).
    interaction_plain_lines: Vec<PlainTextLine>,
}

impl WorkspaceState {
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn new(initial_missing: Vec<String>) -> Self {
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
            plan_permission_mode: PlanPermissionMode::BypassPermissions,
            active_tab: MainTab::Conversation,
            blocks,
            plans: Vec::new(),
            agent_team: None,
            input: InputBuffer::default(),
            input_history: Vec::new(),
            input_history_cursor: None,
            input_history_draft: String::new(),
            command_completion_cycle: None,
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
            current_run_ref: None,
            selection: None,
            copy_feedback: None,
            conv_plain_lines: Vec::new(),
            plans_plain_lines: Vec::new(),
            interaction_plain_lines: Vec::new(),
        }
    }

    /// Get a reference to the conversation blocks.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn blocks(&self) -> &[ConversationBlock] {
        &self.blocks
    }

    /// Get the current interaction mode.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn mode(&self) -> InteractionMode {
        self.mode
    }

    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn plan_permission_mode(&self) -> PlanPermissionMode {
        self.plan_permission_mode
    }

    /// Activate a minimal plan execution for state-machine tests.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn activate_plan_execution_for_test(&mut self) {
        self.pending_goal = Some("test goal".to_string());
        self.plan_permission_mode = PlanPermissionMode::BypassPermissions;
        self.plans = vec![PlanNode::new(
            "step-1",
            "Test plan step",
            PlanNodeStatus::Pending,
        )];
    }

    /// Get the current focus zone.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn focus_zone(&self) -> FocusZone {
        self.focus_zone
    }

    /// Check if quit was requested.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn quit_requested(&self) -> bool {
        self.quit_requested
    }

    /// Get the current input buffer value.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn input_value(&self) -> &str {
        self.input.value()
    }

    /// Inject a mouse event for testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn handle_mouse_event(&mut self, mouse: MouseEvent) {
        self.handle_mouse(mouse);
    }

    /// Set terminal dimensions for layout testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn set_terminal_size(&mut self, width: u16, height: u16) {
        use ratatui::layout::Rect;
        self.layout_rects = LayoutRects {
            conversation: Rect::new(1, 1, width.saturating_sub(2), height.saturating_sub(4)),
            agent_team: Rect::new(0, 0, 0, 0),
            plans: Rect::new(0, 0, 0, 0),
            interaction: Rect::new(1, height.saturating_sub(3), width.saturating_sub(2), 2),
        };
    }

    /// Inject a pending tool confirmation state.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn inject_tool_confirmation(
        &mut self,
        tool_call_id: String,
        tool_name: String,
        details: String,
        run_ref: protocol_interface::core::RunRef,
    ) {
        self.pending_tool_confirmation = Some(ToolConfirmationState {
            tool_call_id,
            tool_name,
            details,
            run_ref,
        });
    }

    /// Clear any pending tool confirmation.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn clear_tool_confirmation(&mut self) {
        self.pending_tool_confirmation = None;
    }

    /// Check if a tool confirmation is pending.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn has_pending_tool_confirmation(&self) -> bool {
        self.pending_tool_confirmation.is_some()
    }

    /// Start a config task for testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn start_config_task_for_test(&mut self) {
        self.start_config_task(runtime_config::Settings::default());
    }

    /// Check if a config task is active.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn has_config_task(&self) -> bool {
        self.config_task.is_some()
    }

    /// Push a conversation block for testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn push_block_for_test(&mut self, block: ConversationBlock) {
        self.push_block(block);
    }

    /// Update streaming text for testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn update_streaming_text_for_test(&mut self, delta: &str) {
        self.update_streaming_text(delta);
    }

    /// Start execution mode for testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn start_execution_for_test(&mut self, mode: ExecutionMode) {
        self.start_execution(mode);
    }

    /// Toggle global expand/collapse for testing.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn toggle_global_expand_for_test(&mut self) {
        self.toggle_global_expand();
    }

    /// Check if globally expanded.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn is_globally_expanded(&self) -> bool {
        self.global_expanded
    }

    /// Get the number of expanded blocks.
    #[cfg(any(test, feature = "testing"))]
    pub(crate) fn expanded_block_count(&self) -> usize {
        self.expanded_blocks.len()
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
                mode_title_override: self.plan_mode_title_override(),
                active_tab: self.active_tab,
                input: &self.input,
                prompt_input: self.prompt_input.as_ref(),
                has_agent_team: self.has_agent_team_tab(),
                command_hint: self.command_hint(models),
                command_matched: self.command_matched(),
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
        self.render_selection_highlight(frame);
        let copy_feedback = self.visible_copy_feedback();
        status_bar::render(
            frame,
            layout[3],
            &self.workspace_status,
            copy_feedback
                .as_ref()
                .map(|(message, is_error)| status_bar::StatusFeedback {
                    message,
                    is_error: *is_error,
                }),
        );
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
                    &mut self.conv_plain_lines,
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
                &mut self.plans_plain_lines,
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

    pub(crate) fn handle_key(&mut self, key: KeyEvent, models: &[String]) -> WorkspaceAction {
        if self.quit_requested {
            return WorkspaceAction::Quit;
        }

        if key.modifiers.contains(KeyModifiers::CONTROL)
            && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
        {
            if matches!(self.interaction, InteractionUi::TextInput) {
                self.show_quit_confirm_prompt();
            }
            return WorkspaceAction::None;
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
                self.reset_command_completion_cycle();
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
            MouseEventKind::Down(MouseButton::Left) => {
                let pos = Position {
                    x: mouse.column,
                    y: mouse.row,
                };
                // Start a new selection (or clear previous one).
                let panel = self.panel_for_position(pos);
                // For interaction panel, cache the input text as plain lines.
                if panel == PanelType::Interaction {
                    self.interaction_plain_lines.clear();
                    let text = self.input.value();
                    for (i, line) in text.lines().enumerate() {
                        self.interaction_plain_lines.push(PlainTextLine {
                            text: line.to_string(),
                            row: i as u16,
                        });
                    }
                    if self.interaction_plain_lines.is_empty() {
                        self.interaction_plain_lines.push(PlainTextLine {
                            text: String::new(),
                            row: 0,
                        });
                    }
                }
                self.selection = Some(TextSelection {
                    panel,
                    start: pos,
                    end: pos,
                });
            }
            MouseEventKind::Drag(MouseButton::Left) => {
                if let Some(sel) = &mut self.selection {
                    sel.end = Position {
                        x: mouse.column,
                        y: mouse.row,
                    };
                }
            }
            MouseEventKind::Up(MouseButton::Left) => {
                if let Some(sel) = self.selection.take() {
                    if sel.start != sel.end {
                        self.copy_selection_to_clipboard(&sel);
                    }
                }
            }
            _ => {}
        }
    }

    /// Determine which panel a position belongs to.
    fn panel_for_position(&self, pos: Position) -> PanelType {
        if self.layout_rects.conversation.contains(pos) {
            PanelType::Conversation
        } else if self.layout_rects.plans.contains(pos) {
            PanelType::Plans
        } else {
            PanelType::Interaction
        }
    }

    fn selected_text_for_selection(&self, sel: &TextSelection) -> Option<String> {
        let (panel_rect, plain_lines, scroll_offset) = match sel.panel {
            PanelType::Conversation => (
                self.layout_rects.conversation,
                &self.conv_plain_lines,
                self.conv_scroll.offset,
            ),
            PanelType::Plans => (
                self.layout_rects.plans,
                &self.plans_plain_lines,
                self.plans_scroll.offset,
            ),
            PanelType::Interaction => (
                self.layout_rects.interaction,
                &self.interaction_plain_lines,
                0u16,
            ),
        };

        // Convert terminal coordinates to panel-local row indices.
        let inner = Rect::new(
            panel_rect.x + 1, // skip border
            panel_rect.y + 1,
            panel_rect.width.saturating_sub(2),
            panel_rect.height.saturating_sub(2),
        );

        let start_row = sel.start.y.saturating_sub(inner.y) + scroll_offset;
        let end_row = sel.end.y.saturating_sub(inner.y) + scroll_offset;
        let (lo_row, lo_col, hi_row, hi_col) =
            if start_row < end_row || (start_row == end_row && sel.start.x <= sel.end.x) {
                (
                    start_row,
                    sel.start.x.saturating_sub(inner.x),
                    end_row,
                    sel.end.x.saturating_sub(inner.x),
                )
            } else {
                (
                    end_row,
                    sel.end.x.saturating_sub(inner.x),
                    start_row,
                    sel.start.x.saturating_sub(inner.x),
                )
            };

        let mut selected = String::new();
        for pl in plain_lines {
            if pl.row < lo_row || pl.row > hi_row {
                continue;
            }
            let line_text = &pl.text;
            let (start_byte, end_byte) = if lo_row == hi_row {
                // Single line: extract between columns.
                let s = helpers::col_to_byte_offset(line_text, lo_col);
                let e = helpers::col_to_byte_offset(line_text, hi_col);
                (s, e)
            } else if pl.row == lo_row {
                // First line: from lo_col to end.
                (
                    helpers::col_to_byte_offset(line_text, lo_col),
                    line_text.len(),
                )
            } else if pl.row == hi_row {
                // Last line: from start to hi_col.
                (0, helpers::col_to_byte_offset(line_text, hi_col))
            } else {
                // Middle line: entire line.
                (0, line_text.len())
            };
            if start_byte < end_byte {
                if !selected.is_empty() {
                    selected.push('\n');
                }
                selected.push_str(&line_text[start_byte..end_byte]);
            }
        }

        if selected.is_empty() {
            None
        } else {
            Some(selected)
        }
    }

    fn render_selection_highlight(&self, frame: &mut Frame) {
        let Some(sel) = &self.selection else {
            return;
        };
        let (panel_rect, plain_lines, scroll_offset) = match sel.panel {
            PanelType::Conversation => (
                self.layout_rects.conversation,
                &self.conv_plain_lines,
                self.conv_scroll.offset,
            ),
            PanelType::Plans => (
                self.layout_rects.plans,
                &self.plans_plain_lines,
                self.plans_scroll.offset,
            ),
            PanelType::Interaction => (
                self.layout_rects.interaction,
                &self.interaction_plain_lines,
                0u16,
            ),
        };
        if panel_rect.width <= 2 || panel_rect.height <= 2 {
            return;
        }

        let inner = Rect::new(
            panel_rect.x + 1,
            panel_rect.y + 1,
            panel_rect.width.saturating_sub(2),
            panel_rect.height.saturating_sub(2),
        );
        let start_row = sel.start.y.saturating_sub(inner.y) + scroll_offset;
        let end_row = sel.end.y.saturating_sub(inner.y) + scroll_offset;
        let (lo_row, lo_col, hi_row, hi_col) =
            if start_row < end_row || (start_row == end_row && sel.start.x <= sel.end.x) {
                (
                    start_row,
                    sel.start.x.saturating_sub(inner.x),
                    end_row,
                    sel.end.x.saturating_sub(inner.x),
                )
            } else {
                (
                    end_row,
                    sel.end.x.saturating_sub(inner.x),
                    start_row,
                    sel.start.x.saturating_sub(inner.x),
                )
            };

        for line in plain_lines {
            if line.row < lo_row || line.row > hi_row {
                continue;
            }
            if line.row < scroll_offset {
                continue;
            }
            let visible_row = line.row - scroll_offset;
            if visible_row >= inner.height {
                continue;
            }

            let start_col = if line.row == lo_row { lo_col } else { 0 }.min(inner.width);
            let end_col = if line.row == hi_row {
                hi_col
            } else {
                inner.width
            }
            .min(inner.width);
            if end_col <= start_col {
                continue;
            }

            let start_byte = helpers::col_to_byte_offset(&line.text, start_col);
            let end_byte = helpers::col_to_byte_offset(&line.text, end_col);
            let mut selected = if start_byte < end_byte {
                line.text[start_byte..end_byte].to_string()
            } else {
                String::new()
            };
            let selection_width = end_col - start_col;
            let selected_width = helpers::char_len(&selected) as u16;
            if selected_width < selection_width {
                selected.push_str(&" ".repeat((selection_width - selected_width) as usize));
            }

            frame.render_widget(
                Paragraph::new(selected).style(crate::tui::theme::selected()),
                Rect::new(inner.x + start_col, inner.y + visible_row, selection_width, 1),
            );
        }
    }

    /// Extract selected text and copy it to the system clipboard.
    fn copy_selection_to_clipboard(&mut self, sel: &TextSelection) {
        let Some(selected) = self.selected_text_for_selection(sel) else {
            return;
        };

        match copy_text_to_clipboard(&selected) {
            Ok(()) => self.show_copy_feedback(t!("workspace.copy.copied").to_string(), false),
            Err(error) => self.show_copy_feedback(
                t!("workspace.copy.failed", error = error).to_string(),
                true,
            ),
        }
    }

    fn show_copy_feedback(&mut self, message: String, is_error: bool) {
        self.copy_feedback = Some(CopyFeedback {
            message,
            is_error,
            expires_at: Instant::now() + Duration::from_secs(COPY_FEEDBACK_SECS),
        });
    }

    fn visible_copy_feedback(&mut self) -> Option<(String, bool)> {
        let expired = self
            .copy_feedback
            .as_ref()
            .is_some_and(|feedback| Instant::now() >= feedback.expires_at);
        if expired {
            self.copy_feedback = None;
        }
        self.copy_feedback
            .as_ref()
            .map(|feedback| (feedback.message.clone(), feedback.is_error))
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
                    self.reset_command_completion_cycle();
                    WorkspaceAction::Submit(input)
                } else {
                    WorkspaceAction::None
                }
            }
            KeyCode::Esc => {
                self.input.clear();
                self.reset_input_history_navigation();
                self.reset_command_completion_cycle();
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
                    self.reset_command_completion_cycle();
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
        self.reset_command_completion_cycle();
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
        self.reset_command_completion_cycle();
    }

    fn reset_input_history_navigation(&mut self) {
        self.input_history_cursor = None;
        self.input_history_draft.clear();
    }

    fn reset_command_completion_cycle(&mut self) {
        self.command_completion_cycle = None;
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
        if !self.input.value().starts_with('/') {
            self.reset_command_completion_cycle();
            return false;
        }

        if self.cycle_command_completion() {
            self.reset_input_history_navigation();
            return true;
        }

        let Some(result) = completion::complete(self.input.value(), self.input.cursor(), models)
        else {
            self.reset_command_completion_cycle();
            return false;
        };

        if result.matches.len() == 1 {
            self.input
                .replace_range(result.start, result.end, &result.matches[0].replacement);
            self.reset_command_completion_cycle();
        } else {
            let replacements = result
                .matches
                .iter()
                .map(|item| item.replacement.clone())
                .collect::<Vec<_>>();
            let selected = 0;
            self.input
                .replace_range(result.start, result.end, &replacements[selected]);
            self.command_completion_cycle = Some(CommandCompletionCycle {
                start: result.start,
                matches: replacements,
                selected,
            });
        }

        self.reset_input_history_navigation();
        true
    }

    fn cycle_command_completion(&mut self) -> bool {
        let Some(cycle) = self.command_completion_cycle.clone() else {
            return false;
        };

        let cursor = self.input.cursor();
        if cursor < cycle.start || cycle.matches.is_empty() {
            self.reset_command_completion_cycle();
            return false;
        }

        let current = self
            .input
            .value()
            .chars()
            .skip(cycle.start)
            .take(cursor.saturating_sub(cycle.start))
            .collect::<String>();
        if cycle.matches.get(cycle.selected) != Some(&current) {
            self.reset_command_completion_cycle();
            return false;
        }

        let selected = (cycle.selected + 1) % cycle.matches.len();
        let replacement = cycle.matches[selected].clone();
        self.input.replace_range(cycle.start, cursor, &replacement);
        self.command_completion_cycle = Some(CommandCompletionCycle { selected, ..cycle });
        true
    }

    /// Whether the current input is a matched slash command.
    fn command_matched(&self) -> bool {
        if self.prompt_input.is_some() {
            return false;
        }
        if !matches!(self.interaction, InteractionUi::TextInput)
            || !self.input.value().starts_with('/')
        {
            return false;
        }
        completion::exact_command_match(self.input.value(), self.input.cursor())
    }

    fn command_hint(&self, models: &[String]) -> Option<String> {
        if self.prompt_input.is_some() {
            return None;
        }

        if !matches!(self.interaction, InteractionUi::TextInput)
            || !self.input.value().starts_with('/')
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
            InteractionMode::Plan => InteractionMode::Chat,
            InteractionMode::Chat => InteractionMode::Bypass,
            InteractionMode::Bypass => InteractionMode::Plan,
        };
        // 不再清空输入框 — 模式切换时保留用户输入内容
    }

    fn has_active_plan_execution(&self) -> bool {
        self.pending_goal.is_some() && !self.plans.is_empty()
    }

    fn toggle_plan_permission_mode(&mut self) {
        self.plan_permission_mode = self.plan_permission_mode.toggle();
        self.push_block(ConversationBlock::status(t!(
            "workspace.plan_permission.switched",
            mode = self.plan_permission_mode.title()
        )));
    }

    fn plan_mode_title_override(&self) -> Option<String> {
        if self.has_active_plan_execution() {
            Some(self.plan_permission_mode.title())
        } else {
            None
        }
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

    fn start_model_task_for_readiness(
        &mut self,
        settings: runtime_config::Settings,
        issues: &[runtime_config::ModelAssignmentReadinessIssue],
    ) {
        self.start_config_like_task(ConfigTask::model_switch_for_readiness(settings, issues));
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
                    crate::tui::theme::set_theme(&settings.ui.theme);
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
        let theme = settings.ui.theme.clone();
        *session.settings.write().unwrap() = settings.clone();
        crate::set_locale(&locale);
        crate::tui::theme::set_theme(&theme);
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

    fn show_quit_confirm_prompt(&mut self) {
        self.prompt_input = None;
        self.input.clear();
        self.push_block(ConversationBlock::decision(t!(
            "workspace.quit_confirm.description"
        )));
        self.interaction = InteractionUi::Decision(DecisionState::quit_confirm());
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
    /// Creates a PromptInputState with Approve/Deny choices and formatted details.
    fn show_tool_confirmation(&mut self, confirmation: ToolConfirmationState) {
        let title = format!("Tool '{}' requires confirmation", confirmation.tool_name);
        let formatted_details = Self::format_tool_details(
            &confirmation.tool_name,
            &confirmation.tool_call_id,
            &confirmation.details,
        );
        let help = format!(
            "Tool: {} | ID: {} | Args: {}",
            confirmation.tool_name,
            confirmation.tool_call_id,
            Self::truncate_details(&confirmation.details, 80)
        );

        let choices = vec![
            PromptChoice::new("Approve", "approve"),
            PromptChoice::new("Deny", "deny"),
        ];

        let prompt_input =
            PromptInputState::new(title, PromptInputKind::SingleSelect, choices, help);

        // Show a detailed block in the conversation for context
        self.push_block(ConversationBlock::request(formatted_details));

        self.pending_tool_confirmation = Some(confirmation);
        self.prompt_input = Some(prompt_input);
        self.input.clear();
        self.focus_zone = FocusZone::Input;
    }

    /// Format tool details for display to user.
    fn format_tool_details(tool_name: &str, tool_call_id: &str, details: &str) -> String {
        let args_display = Self::format_tool_args(details);
        format!(
            "Tool confirmation required\n  Tool: {}\n  Call ID: {}\n  Args: {}",
            tool_name, tool_call_id, args_display
        )
    }

    /// Format tool arguments for readable display.
    fn format_tool_args(details: &str) -> String {
        // Try to parse as JSON for pretty display
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(details) {
            if let Some(obj) = value.as_object() {
                // Format as key=value pairs for readability
                let formatted: Vec<String> = obj
                    .iter()
                    .map(|(k, v)| {
                        let val_str = match v {
                            serde_json::Value::String(s) => {
                                format!("\"{}\"", Self::truncate_details(s, 50))
                            }
                            serde_json::Value::Number(n) => n.to_string(),
                            serde_json::Value::Bool(b) => b.to_string(),
                            _ => Self::truncate_details(&v.to_string(), 50),
                        };
                        format!("{}={}", k, val_str)
                    })
                    .collect();
                return formatted.join(", ");
            }
        }
        // Fallback: just show truncated raw details
        Self::truncate_details(details, 120)
    }

    /// Truncate a string to `max_len` characters (not bytes).
    /// Safe for multi-byte UTF-8 content (Chinese, emoji, etc.).
    fn truncate_details(s: &str, max_len: usize) -> String {
        if s.chars().count() <= max_len {
            s.to_string()
        } else {
            let truncated: String = s.chars().take(max_len).collect();
            format!("{}...", truncated)
        }
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

        let confirmation = self.pending_tool_confirmation.as_ref()?;
        let run_ref = confirmation.run_ref.clone();
        let tool_call_id = confirmation.tool_call_id.clone();

        Some((run_ref, tool_call_id, approved))
    }

    /// Complete a successful tool confirmation: clear state and show result.
    fn complete_tool_confirmation(&mut self, approved: bool) {
        if let Some(confirmation) = self.pending_tool_confirmation.take() {
            self.prompt_input = None;
            self.restore_text_input();

            if approved {
                self.push_block(ConversationBlock::request(format!(
                    "✓ Tool '{}' approved (ID: {})",
                    confirmation.tool_name, confirmation.tool_call_id
                )));
            } else {
                self.push_block(ConversationBlock::error(format!(
                    "✗ Tool '{}' denied (ID: {}) — tool will not execute",
                    confirmation.tool_name, confirmation.tool_call_id
                )));
            }
        }
    }

    /// Handle a failed confirmation response: show error and fail-closed.
    fn fail_tool_confirmation(&mut self, error: String) {
        if let Some(confirmation) = self.pending_tool_confirmation.take() {
            self.prompt_input = None;
            self.restore_text_input();

            self.push_block(ConversationBlock::error(format!(
                "✗ Tool '{}' confirmation failed (ID: {}): {}\n  The tool will not execute for safety.",
                confirmation.tool_name, confirmation.tool_call_id, error
            )));
        }
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
        self.plan_permission_mode = PlanPermissionMode::BypassPermissions;
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
        let title = t!("workspace.tool.started", tool = tool);
        // Raw tool name (e.g. "shell", "read_file") drives the per-tool icon.
        let raw_name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(|s| s.to_string());

        if let Some(block) = self.blocks.last_mut() {
            if block.is_execution() && block.content.trim().is_empty() {
                block.title = Some(title.to_string());
                block.block_type = ConversationBlockType::Execution;
                block.tool_name = raw_name;
                self.stick_conversation_to_latest();
                return;
            }
        }

        let mut block = ConversationBlock::execution("");
        block.title = Some(title.to_string());
        block.tool_name = raw_name;
        self.push_block(block);
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
            .map(|text| format_tool_completion_output(value, text))
            .filter(|text| !text.is_empty())
            .map(|text| truncate_chars(&text, 1200));
        let raw_name = value
            .get("name")
            .and_then(serde_json::Value::as_str)
            .map(|s| s.to_string());

        let title = if success {
            t!("workspace.tool.completed", tool = tool)
        } else {
            t!("workspace.tool.failed", tool = tool)
        };

        let content = output.unwrap_or_default();

        if success {
            let mut block = ConversationBlock::result(&content);
            block.title = Some(title.to_string());
            block.tool_name = raw_name;
            self.push_block(block);
        } else {
            let mut block = ConversationBlock::error(&content);
            block.title = Some(title.to_string());
            block.tool_name = raw_name;
            self.push_block(block);
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
            self.expanded_blocks.clear();
            self.global_expanded = false;
        } else {
            self.global_expanded = true;
        }
    }
}

fn copy_text_to_clipboard(text: &str) -> std::result::Result<(), String> {
    let system_result = arboard::Clipboard::new()
        .map_err(|error| error.to_string())
        .and_then(|mut clipboard| {
            clipboard
                .set_text(text.to_string())
                .map_err(|error| error.to_string())
        });

    match system_result {
        Ok(()) => Ok(()),
        Err(system_error) => copy_text_to_terminal_clipboard(text).map_err(|terminal_error| {
            format!(
                "system clipboard unavailable ({system_error}); terminal clipboard failed ({terminal_error})"
            )
        }),
    }
}

fn copy_text_to_terminal_clipboard(text: &str) -> io::Result<()> {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    let mut stdout = io::stdout();
    write!(stdout, "\x1b]52;c;{encoded}\x07")?;
    stdout.flush()
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

        // Extract confirmation data
        let result = state.handle_tool_confirmation_response("approve");
        assert!(result.is_some());

        let (returned_run_ref, tool_call_id, approved) = result.unwrap();
        assert_eq!(returned_run_ref, run_ref);
        assert_eq!(tool_call_id, "tc-2");
        assert!(approved);

        // Confirm: state should NOT be cleared yet (pending bridge response)
        assert!(state.prompt_input.is_some());
        assert!(state.pending_tool_confirmation.is_some());

        // Complete the confirmation
        state.complete_tool_confirmation(approved);
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

        // Extract confirmation data
        let result = state.handle_tool_confirmation_response("deny");
        assert!(result.is_some());

        let (returned_run_ref, tool_call_id, approved) = result.unwrap();
        assert_eq!(returned_run_ref, run_ref);
        assert_eq!(tool_call_id, "tc-3");
        assert!(!approved);

        // Confirm: state should NOT be cleared yet
        assert!(state.prompt_input.is_some());
        assert!(state.pending_tool_confirmation.is_some());

        // Complete the confirmation
        state.complete_tool_confirmation(approved);
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

    #[test]
    fn test_fail_tool_confirmation_clears_state() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-5".to_string(),
            tool_name: "shell".to_string(),
            details: "ls".to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);
        assert!(state.prompt_input.is_some());
        assert!(state.pending_tool_confirmation.is_some());

        // Simulate failure
        state.fail_tool_confirmation("delivery failed".to_string());

        // State should be cleared
        assert!(state.prompt_input.is_none());
        assert!(state.pending_tool_confirmation.is_none());
    }

    #[test]
    fn test_show_tool_confirmation_displays_tool_name_and_id() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-12345".to_string(),
            tool_name: "shell".to_string(),
            details: serde_json::json!({"command": "ls -la /tmp"}).to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);

        // Verify prompt_input is set with correct title
        let prompt = state.prompt_input.as_ref().unwrap();
        assert!(prompt.title.contains("shell"));
        assert!(prompt.title.contains("requires confirmation"));

        // Verify help text contains tool name and ID
        assert!(prompt.help.contains("shell"));
        assert!(prompt.help.contains("tc-12345"));
    }

    #[test]
    fn test_show_tool_confirmation_formats_json_args() {
        let mut state = WorkspaceState::new(vec![]);
        let details = serde_json::json!({
            "path": "/tmp/test.txt",
            "content": "hello world",
            "overwrite": true
        })
        .to_string();
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-args".to_string(),
            tool_name: "write_file".to_string(),
            details,
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);

        // Should have a conversation block with formatted args
        let blocks = &state.blocks;
        let last_block = blocks.last().unwrap();
        let content = &last_block.content;
        assert!(content.contains("write_file"));
        assert!(content.contains("tc-args"));
        assert!(content.contains("path="));
        assert!(content.contains("/tmp/test.txt"));
    }

    #[test]
    fn test_complete_tool_confirmation_shows_tool_call_id_for_approve() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-approve-test".to_string(),
            tool_name: "shell".to_string(),
            details: "echo test".to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);
        state.complete_tool_confirmation(true);

        // Verify the approve block shows tool_call_id
        let blocks = &state.blocks;
        let last_block = blocks.last().unwrap();
        assert!(last_block.content.contains("✓"));
        assert!(last_block.content.contains("shell"));
        assert!(last_block.content.contains("tc-approve-test"));
    }

    #[test]
    fn test_complete_tool_confirmation_shows_tool_call_id_for_deny() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-deny-test".to_string(),
            tool_name: "edit_file".to_string(),
            details: "find: old, replace: new".to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);
        state.complete_tool_confirmation(false);

        // Verify the deny block shows tool_call_id and "will not execute"
        let blocks = &state.blocks;
        let last_block = blocks.last().unwrap();
        assert!(last_block.content.contains("✗"));
        assert!(last_block.content.contains("edit_file"));
        assert!(last_block.content.contains("tc-deny-test"));
        assert!(last_block.content.contains("will not execute"));
    }

    #[test]
    fn test_fail_tool_confirmation_shields_error_details() {
        let mut state = WorkspaceState::new(vec![]);
        let confirmation = ToolConfirmationState {
            tool_call_id: "tc-fail".to_string(),
            tool_name: "shell".to_string(),
            details: "rm -rf /".to_string(),
            run_ref: RunRef::new(),
        };

        state.show_tool_confirmation(confirmation);
        state.fail_tool_confirmation("internal channel error".to_string());

        // Verify error message is user-friendly, not raw internal details
        let blocks = &state.blocks;
        let last_block = blocks.last().unwrap();
        assert!(last_block.content.contains("✗"));
        assert!(last_block.content.contains("shell"));
        assert!(last_block.content.contains("tc-fail"));
        assert!(last_block.content.contains("internal channel error"));
        assert!(last_block.content.contains("will not execute"));
        // Should NOT expose raw stack traces
        assert!(!last_block.content.contains("backtrace"));
        assert!(!last_block.content.contains("thread"));
    }

    #[test]
    fn test_format_tool_args_pretty_prints_json() {
        let details = serde_json::json!({
            "command": "ls -la",
            "timeout": 30
        })
        .to_string();
        let formatted = WorkspaceState::format_tool_args(&details);
        assert!(formatted.contains("command=\"ls -la\""));
        assert!(formatted.contains("timeout=30"));
    }

    #[test]
    fn test_format_tool_args_handles_non_json() {
        let details = "raw string details without JSON";
        let formatted = WorkspaceState::format_tool_args(details);
        assert_eq!(formatted, details);
    }

    #[test]
    fn test_truncate_details_short_string() {
        let result = WorkspaceState::truncate_details("short", 100);
        assert_eq!(result, "short");
    }

    #[test]
    fn test_truncate_details_long_string() {
        let long = "a".repeat(200);
        let result = WorkspaceState::truncate_details(&long, 100);
        assert_eq!(result.chars().count(), 103); // 100 + "..."
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_details_chinese() {
        // Chinese characters are 3 bytes each
        let chinese = "你好世界测试工具确认".to_string(); // 10 chars, 30 bytes
        let result = WorkspaceState::truncate_details(&chinese, 7);
        assert_eq!(result, "你好世界测试工...");
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_details_emoji() {
        // Emoji are multi-byte
        let emoji = "🔧🛠️✅❌".to_string();
        let result = WorkspaceState::truncate_details(&emoji, 2);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 5); // 2 chars + "..."
    }

    #[test]
    fn test_truncate_details_mixed_content() {
        let mixed = "path: /tmp/文件.txt".to_string(); // mixed ASCII + Chinese
        let result = WorkspaceState::truncate_details(&mixed, 10);
        assert!(result.ends_with("..."));
        assert!(result.chars().count() <= 13); // 10 chars + "..."
    }
}

/// TUI state-machine tests using `TuiTestHarness` and `VecEventSource`.
///
/// Included from a separate file so coverage tools can exclude it from
/// production line coverage via --ignore-filename-regex.
#[cfg(test)]
mod tui_state_machine_tests {
    include!("state_machine_tests.rs");
}
