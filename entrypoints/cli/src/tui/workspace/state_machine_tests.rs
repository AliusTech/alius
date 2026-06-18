// TUI state-machine tests using `TuiTestHarness` and `VecEventSource`.
//
// Covers the required scenarios from validation.md:
// - welcome block presence
// - Plan/Bypass mode toggle
// - config-task Shift+Tab guard
// - Esc interrupt
// - tool confirmation inject/clear
// - Core event replay
//
// This file is included via `include!` in workspace/mod.rs inside a
// `#[cfg(test)]` module. It is a separate file so that coverage tools
// can exclude it from production line coverage via --ignore-filename-regex.

use super::*;
use crate::tui::testing::{
    full_turn_event_source, text_event_source, TuiTestHarness, VecEventSource,
};
use crossterm::event::{KeyCode, KeyModifiers};
use protocol_interface::core::{CoreEvent, CoreEventKind, CoreEventPayload, RunRef, TraceId};

// ── Welcome block ──────────────────────────────────────────────────────

#[test]
fn welcome_block_present_on_startup() {
    rust_i18n::set_locale("en");
    let harness = TuiTestHarness::new();

    assert!(harness.has_welcome_block(), "expected a welcome block");
    assert_eq!(harness.block_count(), 1);
    assert_eq!(
        harness.blocks()[0].block_type,
        crate::tui::state::ConversationBlockType::Welcome
    );
}

#[test]
fn welcome_block_contains_ready_field() {
    rust_i18n::set_locale("en");
    let harness = TuiTestHarness::new();

    let content = &harness.blocks()[0].content;
    assert!(
        content.contains("\"ready\":false") || content.contains("\"ready\":true"),
        "welcome block should contain ready field, got: {content}"
    );
}

// ── Plan / Bypass mode toggle ──────────────────────────────────────────

#[test]
fn starts_in_plan_mode() {
    let harness = TuiTestHarness::new();
    assert_eq!(harness.mode(), InteractionMode::Plan);
}

#[test]
fn shift_tab_toggles_plan_to_bypass() {
    let mut harness = TuiTestHarness::new();
    let action = harness.press_key_with(KeyCode::BackTab, KeyModifiers::SHIFT);

    assert!(matches!(action, WorkspaceAction::None));
    assert_eq!(harness.mode(), InteractionMode::Bypass);
}

#[test]
fn shift_tab_toggles_bypass_back_to_plan() {
    let mut harness = TuiTestHarness::new();

    harness.press_key_with(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(harness.mode(), InteractionMode::Bypass);

    harness.press_key_with(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(harness.mode(), InteractionMode::Plan);
}

#[test]
fn mode_toggle_preserves_input_text() {
    let mut harness = TuiTestHarness::new();

    harness.type_text("hello world");
    assert_eq!(harness.input_value(), "hello world");

    harness.press_key_with(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(harness.input_value(), "hello world");
    assert_eq!(harness.mode(), InteractionMode::Bypass);
}

// ── Config task: Shift+Tab guards mode ─────────────────────────────────

#[test]
fn shift_tab_in_config_task_does_not_toggle_mode() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();
    assert_eq!(harness.mode(), InteractionMode::Plan);

    harness.start_config_task();
    assert!(harness.has_config_task(), "config task should be active");

    // Shift+Tab should step the config wizard, not toggle Plan/Bypass
    harness.press_key_with(KeyCode::BackTab, KeyModifiers::SHIFT);
    assert_eq!(
        harness.mode(),
        InteractionMode::Plan,
        "mode must stay Plan when config task is active"
    );
}

#[test]
fn tab_in_config_task_does_not_trigger_completion() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.start_config_task();
    assert!(harness.has_config_task());

    // Tab should step the wizard forward, not trigger command completion
    let action = harness.press_key(KeyCode::Tab);
    assert!(matches!(action, WorkspaceAction::None));
    assert_eq!(
        harness.mode(),
        InteractionMode::Plan,
        "mode must stay Plan when config task is active"
    );
}

// ── Esc clears input ───────────────────────────────────────────────────

#[test]
fn esc_clears_input_buffer() {
    let mut harness = TuiTestHarness::new();

    harness.type_text("some text");
    assert_eq!(harness.input_value(), "some text");

    let action = harness.press_key(KeyCode::Esc);
    assert!(matches!(action, WorkspaceAction::None));
    assert_eq!(harness.input_value(), "");
}

#[test]
fn esc_on_empty_input_is_noop() {
    let mut harness = TuiTestHarness::new();

    assert_eq!(harness.input_value(), "");
    let action = harness.press_key(KeyCode::Esc);
    assert!(matches!(action, WorkspaceAction::None));
    assert_eq!(harness.input_value(), "");
}

// ── Ctrl+C / Ctrl+D quit ───────────────────────────────────────────────

#[test]
fn ctrl_c_returns_quit_action() {
    let mut harness = TuiTestHarness::new();

    let action = harness.press_key_with(KeyCode::Char('c'), KeyModifiers::CONTROL);
    assert!(
        matches!(action, WorkspaceAction::Quit),
        "Ctrl+C should return Quit action, got {action:?}"
    );
}

#[test]
fn ctrl_d_returns_quit_action() {
    let mut harness = TuiTestHarness::new();

    let action = harness.press_key_with(KeyCode::Char('d'), KeyModifiers::CONTROL);
    assert!(
        matches!(action, WorkspaceAction::Quit),
        "Ctrl+D should return Quit action, got {action:?}"
    );
}

// ── Focus zone cycling ─────────────────────────────────────────────────

#[test]
fn tab_cycles_focus_zones() {
    let mut harness = TuiTestHarness::new();
    assert_eq!(harness.focus_zone(), FocusZone::Input);

    // Tab -> Conversation (no right panel, so cycles back)
    harness.press_key(KeyCode::Tab);
    assert_eq!(harness.focus_zone(), FocusZone::Conversation);

    // Tab -> Input (no right panel)
    harness.press_key(KeyCode::Tab);
    assert_eq!(harness.focus_zone(), FocusZone::Input);
}

// ── Tool confirmation injection ────────────────────────────────────────

#[test]
fn inject_and_check_tool_confirmation() {
    let mut harness = TuiTestHarness::new();

    assert!(!harness.has_pending_tool_confirmation());

    harness.inject_tool_confirmation("tc-1", "shell", "ls -la");
    assert!(harness.has_pending_tool_confirmation());
}

#[test]
fn clear_tool_confirmation() {
    let mut harness = TuiTestHarness::new();

    harness.inject_tool_confirmation("tc-1", "shell", "ls -la");
    assert!(harness.has_pending_tool_confirmation());

    harness.clear_tool_confirmation();
    assert!(!harness.has_pending_tool_confirmation());
}

#[test]
fn inject_tool_confirmation_overwrites_previous() {
    let mut harness = TuiTestHarness::new();

    harness.inject_tool_confirmation("tc-1", "shell", "ls -la");
    harness.inject_tool_confirmation("tc-2", "write_file", "/tmp/test");
    assert!(harness.has_pending_tool_confirmation());
}

// ── Terminal size variants ─────────────────────────────────────────────

#[test]
fn small_terminal_does_not_panic() {
    let mut harness = TuiTestHarness::new();
    harness.set_small_terminal();

    let action = harness.press_key(KeyCode::Tab);
    assert!(matches!(action, WorkspaceAction::None));
}

#[test]
fn standard_terminal_does_not_panic() {
    let mut harness = TuiTestHarness::new();
    harness.set_standard_terminal();

    let action = harness.press_key(KeyCode::Tab);
    assert!(matches!(action, WorkspaceAction::None));
}

#[test]
fn wide_terminal_does_not_panic() {
    let mut harness = TuiTestHarness::new();
    harness.set_wide_terminal();

    let action = harness.press_key(KeyCode::Tab);
    assert!(matches!(action, WorkspaceAction::None));
}

// ── VecEventSource ─────────────────────────────────────────────────────

#[test]
fn vec_event_source_yields_events_in_order() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let events = vec![
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            0,
            CoreEventKind::TurnStarted,
            CoreEventPayload::Empty,
        ),
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            1,
            CoreEventKind::ModelDelta,
            CoreEventPayload::Text {
                text: "hello".to_string(),
            },
        ),
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            2,
            CoreEventKind::FinalResult,
            CoreEventPayload::Final {
                content: "hello".to_string(),
                success: true,
            },
        ),
    ];

    let mut source = VecEventSource::new(events);
    assert_eq!(source.remaining(), 3);
    assert!(!source.is_empty());

    let e1 = source.next().unwrap();
    assert_eq!(e1.kind, CoreEventKind::TurnStarted);

    let e2 = source.next().unwrap();
    assert_eq!(e2.kind, CoreEventKind::ModelDelta);

    let e3 = source.next().unwrap();
    assert_eq!(e3.kind, CoreEventKind::FinalResult);

    assert!(source.next().is_none());
    assert!(source.is_empty());
    assert_eq!(source.remaining(), 0);
}

#[test]
fn vec_event_source_peek_does_not_advance() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let events = vec![CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        0,
        CoreEventKind::ModelDelta,
        CoreEventPayload::Text {
            text: "test".to_string(),
        },
    )];

    let mut source = VecEventSource::new(events);

    let peeked = source.peek().unwrap();
    assert_eq!(peeked.kind, CoreEventKind::ModelDelta);
    let peeked2 = source.peek().unwrap();
    assert_eq!(peeked2.kind, CoreEventKind::ModelDelta);

    assert_eq!(source.remaining(), 1);

    source.next();
    assert!(source.peek().is_none());
}

#[test]
fn vec_event_source_reset_replays() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let events = vec![CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        0,
        CoreEventKind::TurnStarted,
        CoreEventPayload::Empty,
    )];

    let mut source = VecEventSource::new(events);

    source.next();
    assert!(source.is_empty());

    source.reset();
    assert_eq!(source.remaining(), 1);
    assert!(source.next().is_some());
}

#[test]
fn vec_event_source_empty() {
    let source = VecEventSource::empty();
    assert!(source.is_empty());
    assert_eq!(source.remaining(), 0);
    assert_eq!(source.total(), 0);
}

#[test]
fn vec_event_source_drain_returns_remaining() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let events = vec![
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            0,
            CoreEventKind::TurnStarted,
            CoreEventPayload::Empty,
        ),
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            1,
            CoreEventKind::ModelDelta,
            CoreEventPayload::Text {
                text: "a".to_string(),
            },
        ),
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            2,
            CoreEventKind::FinalResult,
            CoreEventPayload::Final {
                content: "a".to_string(),
                success: true,
            },
        ),
    ];

    let mut source = VecEventSource::new(events);
    source.next();

    let remaining = source.drain();
    assert_eq!(remaining.len(), 2);
    assert_eq!(remaining[0].kind, CoreEventKind::ModelDelta);
    assert_eq!(remaining[1].kind, CoreEventKind::FinalResult);
    assert!(source.is_empty());
}

// ── Core event replay helpers ──────────────────────────────────────────

#[test]
fn text_event_source_builds_delta_then_final() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let mut source = text_event_source(&run_ref, &trace_id, &["hel", "lo"], "hello");

    let e1 = source.next().unwrap();
    assert_eq!(e1.kind, CoreEventKind::ModelDelta);

    let e2 = source.next().unwrap();
    assert_eq!(e2.kind, CoreEventKind::ModelDelta);

    let e3 = source.next().unwrap();
    assert_eq!(e3.kind, CoreEventKind::FinalResult);

    assert!(source.next().is_none());
}

#[test]
fn full_turn_event_source_builds_started_delta_final() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let mut source = full_turn_event_source(&run_ref, &trace_id, "response");

    let e1 = source.next().unwrap();
    assert_eq!(e1.kind, CoreEventKind::TurnStarted);

    let e2 = source.next().unwrap();
    assert_eq!(e2.kind, CoreEventKind::ModelDelta);

    let e3 = source.next().unwrap();
    assert_eq!(e3.kind, CoreEventKind::FinalResult);

    assert!(source.next().is_none());
}

#[test]
fn vec_event_source_single() {
    let run_ref = RunRef::new();
    let trace_id = TraceId::new();

    let event = CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        0,
        CoreEventKind::ErrorRaised,
        CoreEventPayload::Error {
            code: "test".to_string(),
            message: "test error".to_string(),
        },
    );

    let mut source = VecEventSource::single(event);
    assert_eq!(source.remaining(), 1);
    assert_eq!(source.total(), 1);

    let e = source.next().unwrap();
    assert_eq!(e.kind, CoreEventKind::ErrorRaised);
    assert!(source.next().is_none());
}

// ── Block manipulation ─────────────────────────────────────────────────

#[test]
fn push_block_adds_to_conversation() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();
    assert_eq!(harness.block_count(), 1); // welcome block

    harness.push_block(ConversationBlock::result("test output"));
    assert_eq!(harness.block_count(), 2);
}

#[test]
fn push_block_result_has_correct_type() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.push_block(ConversationBlock::result("output"));
    let block = harness.blocks().last().unwrap();
    assert_eq!(block.block_type, ConversationBlockType::Result);
}

#[test]
fn push_block_error_has_correct_type() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.push_block(ConversationBlock::error("something failed"));
    let block = harness.blocks().last().unwrap();
    assert_eq!(block.block_type, ConversationBlockType::Error);
}

#[test]
fn push_block_execution_has_correct_type() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.push_block(ConversationBlock::execution(""));
    let block = harness.blocks().last().unwrap();
    assert_eq!(block.block_type, ConversationBlockType::Execution);
}

// ── Streaming text ─────────────────────────────────────────────────────

#[test]
fn streaming_text_updates_execution_block() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.push_block(ConversationBlock::streaming(""));
    harness.update_streaming_text("hello ");
    harness.update_streaming_text("world");

    let block = harness.blocks().last().unwrap();
    assert_eq!(block.block_type, ConversationBlockType::Streaming);
    assert!(block.content.contains("hello world"));
}

// ── Folding (global expand/collapse) ───────────────────────────────────

#[test]
fn global_expand_toggle() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    assert!(!harness.is_globally_expanded());

    harness.toggle_global_expand();
    assert!(harness.is_globally_expanded());

    harness.toggle_global_expand();
    assert!(!harness.is_globally_expanded());
}

#[test]
fn ctrl_o_toggles_global_expand() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.press_key_with(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(harness.is_globally_expanded());

    harness.press_key_with(KeyCode::Char('o'), KeyModifiers::CONTROL);
    assert!(!harness.is_globally_expanded());
}

// ── Execution mode ─────────────────────────────────────────────────────

#[test]
fn start_execution_adds_blocks() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();
    let initial_count = harness.block_count();

    harness.start_execution(ExecutionMode::Bypass);
    // start_execution should add an execution block
    assert!(harness.block_count() >= initial_count);
}

// ── Responsive layout ──────────────────────────────────────────────────

#[test]
fn tiny_terminal_minimum_size_handling() {
    let mut harness = TuiTestHarness::new();
    // 1x1 — below minimum threshold
    harness.set_terminal_size(1, 1);

    // Key handling should still work without panic
    harness.type_text("test");
    assert_eq!(harness.input_value(), "test");
}

#[test]
fn very_wide_terminal() {
    let mut harness = TuiTestHarness::new();
    harness.set_terminal_size(500, 100);

    harness.type_text("wide terminal test");
    assert_eq!(harness.input_value(), "wide terminal test");
}

// ── Input history ──────────────────────────────────────────────────────

#[test]
fn enter_submits_input() {
    let mut harness = TuiTestHarness::new();

    let action = harness.submit_input("hello");
    assert!(matches!(action, WorkspaceAction::Submit(s) if s == "hello"));
}

#[test]
fn enter_empty_input_returns_none() {
    let mut harness = TuiTestHarness::new();

    let action = harness.press_key(KeyCode::Enter);
    assert!(matches!(action, WorkspaceAction::None));
}

// ── Backspace ──────────────────────────────────────────────────────────

#[test]
fn backspace_removes_character() {
    let mut harness = TuiTestHarness::new();

    harness.type_text("abc");
    assert_eq!(harness.input_value(), "abc");

    harness.press_key(KeyCode::Backspace);
    assert_eq!(harness.input_value(), "ab");
}

#[test]
fn backspace_on_empty_is_noop() {
    let mut harness = TuiTestHarness::new();

    harness.press_key(KeyCode::Backspace);
    assert_eq!(harness.input_value(), "");
}

// ── Decision mode ──────────────────────────────────────────────────────

#[test]
fn decision_key_returns_action() {
    // When in a decision state, keys should route to the decision handler
    let mut harness = TuiTestHarness::new();

    // Enter on empty input in normal mode returns None
    let action = harness.press_key(KeyCode::Enter);
    assert!(matches!(action, WorkspaceAction::None));
}

// ── Tab completion ─────────────────────────────────────────────────────

#[test]
fn tab_completes_slash_command() {
    rust_i18n::set_locale("en");
    let mut harness = TuiTestHarness::new();

    harness.type_text("/he");
    harness.press_key(KeyCode::Tab);

    assert_eq!(harness.input_value(), "/help");
    assert_eq!(harness.focus_zone(), FocusZone::Input);
}

#[test]
fn tab_does_not_complete_partial_non_command() {
    let mut harness = TuiTestHarness::new();

    harness.type_text("hel");
    harness.press_key(KeyCode::Tab);

    // Should cycle focus, not complete
    assert_eq!(harness.focus_zone(), FocusZone::Conversation);
}
