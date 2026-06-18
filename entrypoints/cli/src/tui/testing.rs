//! Test utilities for the TUI subsystem.
//!
//! Provides shared helpers for TUI state-machine tests: key event
//! construction, text input simulation, execution mode helpers,
//! [`TuiTestHarness`] for workspace state testing, and [`VecEventSource`]
//! for deterministic Core event replay.
//!
//! These helpers are only available in test builds because they access
//! private methods on `WorkspaceState`.

#![allow(dead_code)]

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent};

use super::state::ConversationBlock;
use super::workspace::{PlanPermissionMode, WorkspaceAction, WorkspaceState};

// ── Key helpers ────────────────────────────────────────────────────────

/// Create a `KeyEvent` with no modifiers.
///
/// # Examples
///
/// ```ignore
/// let k = key(KeyCode::Enter);
/// let action = state.handle_key(k, &[]);
/// ```
pub fn key(code: KeyCode) -> KeyEvent {
    KeyEvent::new(code, KeyModifiers::NONE)
}

/// Create a `KeyEvent` with a modifier.
///
/// # Examples
///
/// ```ignore
/// let k = key_with(KeyCode::Tab, KeyModifiers::SHIFT);
/// ```
pub fn key_with(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(code, modifiers)
}

/// Simulate typing text character by character into a `WorkspaceState`.
///
/// Each character is inserted via `handle_key` with `KeyCode::Char`.
pub fn type_text(state: &mut WorkspaceState, value: &str) {
    for ch in value.chars() {
        let _ = state.handle_key(key(KeyCode::Char(ch)), &[]);
    }
}

/// Simulate typing text and pressing Enter, returning the resulting action.
///
/// Returns the `WorkspaceAction` produced by the Enter key press.
pub fn submit_input(state: &mut WorkspaceState, value: &str) -> WorkspaceAction {
    type_text(state, value);
    state.handle_key(key(KeyCode::Enter), &[])
}

/// Handle a key event in execution mode.
///
/// Convenience wrapper that calls `handle_key` and expects the state
/// to be in execution mode.
pub fn handle_execution_key(state: &mut WorkspaceState, key_event: KeyEvent) -> WorkspaceAction {
    state.handle_key(key_event, &[])
}

// ── TuiTestHarness ─────────────────────────────────────────────────────

/// Deterministic harness for TUI state-machine tests.
///
/// Wraps a [`WorkspaceState`] and provides controlled key/mouse injection,
/// terminal-size variants, conversation block inspection, and tool
/// confirmation state management.
///
/// # Examples
///
/// ```ignore
/// let mut harness = TuiTestHarness::new();
/// harness.set_terminal_size(120, 40);
/// let action = harness.press_key(KeyCode::Tab);
/// assert_eq!(harness.blocks().len(), 1); // welcome block
/// ```
pub struct TuiTestHarness {
    state: WorkspaceState,
    models: Vec<String>,
}

impl TuiTestHarness {
    /// Create a new harness with default workspace state.
    pub fn new() -> Self {
        Self {
            state: WorkspaceState::new(vec![]),
            models: vec!["test-model".to_string()],
        }
    }

    /// Create a harness with specific initial missing config keys.
    pub fn with_initial_missing(initial_missing: Vec<String>) -> Self {
        Self {
            state: WorkspaceState::new(initial_missing),
            models: vec!["test-model".to_string()],
        }
    }

    /// Create a harness with specific available models.
    pub fn with_models(models: Vec<String>) -> Self {
        Self {
            state: WorkspaceState::new(vec![]),
            models,
        }
    }

    /// Get a reference to the inner `WorkspaceState`.
    pub fn state(&self) -> &WorkspaceState {
        &self.state
    }

    /// Get a mutable reference to the inner `WorkspaceState`.
    pub fn state_mut(&mut self) -> &mut WorkspaceState {
        &mut self.state
    }

    // ── Key injection ──────────────────────────────────────────────────

    /// Press a single key and return the resulting action.
    pub fn press_key(&mut self, code: KeyCode) -> WorkspaceAction {
        self.state.handle_key(key(code), &self.models)
    }

    /// Press a key with modifiers and return the resulting action.
    pub fn press_key_with(&mut self, code: KeyCode, modifiers: KeyModifiers) -> WorkspaceAction {
        self.state
            .handle_key(key_with(code, modifiers), &self.models)
    }

    /// Press a raw `KeyEvent` and return the resulting action.
    pub fn press_key_event(&mut self, key_event: KeyEvent) -> WorkspaceAction {
        self.state.handle_key(key_event, &self.models)
    }

    /// Type text character by character.
    pub fn type_text(&mut self, value: &str) {
        type_text(&mut self.state, value);
    }

    /// Type text and press Enter, returning the resulting action.
    pub fn submit_input(&mut self, value: &str) -> WorkspaceAction {
        submit_input(&mut self.state, value)
    }

    // ── Mouse injection ────────────────────────────────────────────────

    /// Inject a mouse event.
    pub fn send_mouse(&mut self, mouse: MouseEvent) {
        self.state.handle_mouse_event(mouse);
    }

    // ── Terminal size variants ─────────────────────────────────────────

    /// Set terminal dimensions for layout testing.
    pub fn set_terminal_size(&mut self, width: u16, height: u16) {
        self.state.set_terminal_size(width, height);
    }

    /// Set a small terminal size (below the minimum threshold).
    pub fn set_small_terminal(&mut self) {
        self.set_terminal_size(30, 5);
    }

    /// Set a standard terminal size.
    pub fn set_standard_terminal(&mut self) {
        self.set_terminal_size(120, 40);
    }

    /// Set a wide terminal size.
    pub fn set_wide_terminal(&mut self) {
        self.set_terminal_size(200, 50);
    }

    // ── Block inspection ───────────────────────────────────────────────

    /// Get the conversation blocks.
    pub fn blocks(&self) -> &[ConversationBlock] {
        self.state.blocks()
    }

    /// Get the number of conversation blocks.
    pub fn block_count(&self) -> usize {
        self.state.blocks().len()
    }

    /// Find a block by its type name (e.g. "Welcome", "Request", "PlanProposal").
    pub fn find_block_by_type(&self, type_name: &str) -> Option<&ConversationBlock> {
        self.state
            .blocks()
            .iter()
            .find(|b| format!("{:?}", b.block_type) == type_name)
    }

    /// Check if a welcome block is present.
    pub fn has_welcome_block(&self) -> bool {
        self.state
            .blocks()
            .iter()
            .any(|b| matches!(b.block_type, super::state::ConversationBlockType::Welcome))
    }

    // ── Tool confirmation state ────────────────────────────────────────

    /// Inject a pending tool confirmation.
    pub fn inject_tool_confirmation(&mut self, tool_call_id: &str, tool_name: &str, details: &str) {
        let run_ref = protocol_interface::core::RunRef::new();
        self.state.inject_tool_confirmation(
            tool_call_id.to_string(),
            tool_name.to_string(),
            details.to_string(),
            run_ref,
        );
    }

    /// Clear any pending tool confirmation.
    pub fn clear_tool_confirmation(&mut self) {
        self.state.clear_tool_confirmation();
    }

    /// Check if a tool confirmation is pending.
    pub fn has_pending_tool_confirmation(&self) -> bool {
        self.state.has_pending_tool_confirmation()
    }

    // ── State queries ──────────────────────────────────────────────────

    /// Get the current interaction mode.
    pub fn mode(&self) -> super::state::InteractionMode {
        self.state.mode()
    }

    /// Get the current Plan execution permission strategy.
    pub fn plan_permission_mode(&self) -> PlanPermissionMode {
        self.state.plan_permission_mode()
    }

    /// Get the current focus zone.
    pub fn focus_zone(&self) -> super::workspace::FocusZone {
        self.state.focus_zone()
    }

    /// Check if quit was requested.
    pub fn quit_requested(&self) -> bool {
        self.state.quit_requested()
    }

    /// Get the current input buffer value.
    pub fn input_value(&self) -> &str {
        self.state.input_value()
    }

    // ── Config task ────────────────────────────────────────────────────

    /// Start a config task (simulates `/config` command).
    pub fn start_config_task(&mut self) {
        self.state.start_config_task_for_test();
    }

    /// Check if a config task is active.
    pub fn has_config_task(&self) -> bool {
        self.state.has_config_task()
    }

    // ── Block manipulation ─────────────────────────────────────────────

    /// Push a conversation block.
    pub fn push_block(&mut self, block: super::state::ConversationBlock) {
        self.state.push_block_for_test(block);
    }

    /// Update streaming text (appends to last streaming block).
    pub fn update_streaming_text(&mut self, delta: &str) {
        self.state.update_streaming_text_for_test(delta);
    }

    // ── Execution mode ─────────────────────────────────────────────────

    /// Start execution mode.
    pub fn start_execution(&mut self, mode: super::workspace::ExecutionMode) {
        self.state.start_execution_for_test(mode);
    }

    /// Activate a minimal Plan execution.
    pub fn activate_plan_execution(&mut self) {
        self.state.activate_plan_execution_for_test();
    }

    // ── Folding ────────────────────────────────────────────────────────

    /// Toggle global expand/collapse.
    pub fn toggle_global_expand(&mut self) {
        self.state.toggle_global_expand_for_test();
    }

    /// Check if globally expanded.
    pub fn is_globally_expanded(&self) -> bool {
        self.state.is_globally_expanded()
    }

    /// Get the number of expanded blocks.
    pub fn expanded_block_count(&self) -> usize {
        self.state.expanded_block_count()
    }
}

impl Default for TuiTestHarness {
    fn default() -> Self {
        Self::new()
    }
}

// ── VecEventSource ─────────────────────────────────────────────────────

/// Ordered event replay source for deterministic testing.
///
/// Stores a sequence of [`CoreEvent`]s and yields them one at a time
/// in order. Used for testing event handling without background
/// networking or real runtime streams.
///
/// # Examples
///
/// ```ignore
/// let mut source = VecEventSource::new(vec![event1, event2, event3]);
/// assert_eq!(source.remaining(), 3);
/// let e = source.next().unwrap();
/// assert_eq!(source.remaining(), 2);
/// assert!(source.is_empty());
/// ```
pub struct VecEventSource {
    events: Vec<protocol_interface::core::CoreEvent>,
    index: usize,
}

impl VecEventSource {
    /// Create a new source from an ordered list of events.
    pub fn new(events: Vec<protocol_interface::core::CoreEvent>) -> Self {
        Self { events, index: 0 }
    }

    /// Create an empty event source.
    pub fn empty() -> Self {
        Self {
            events: Vec::new(),
            index: 0,
        }
    }

    /// Create a source from a single event.
    pub fn single(event: protocol_interface::core::CoreEvent) -> Self {
        Self {
            events: vec![event],
            index: 0,
        }
    }

    /// Get the next event, or `None` if exhausted.
    pub fn next(&mut self) -> Option<&protocol_interface::core::CoreEvent> {
        if self.index < self.events.len() {
            let event = &self.events[self.index];
            self.index += 1;
            Some(event)
        } else {
            None
        }
    }

    /// Peek at the next event without advancing.
    pub fn peek(&self) -> Option<&protocol_interface::core::CoreEvent> {
        self.events.get(self.index)
    }

    /// Number of events remaining.
    pub fn remaining(&self) -> usize {
        self.events.len().saturating_sub(self.index)
    }

    /// Check if all events have been consumed.
    pub fn is_empty(&self) -> bool {
        self.remaining() == 0
    }

    /// Total number of events (including already consumed).
    pub fn total(&self) -> usize {
        self.events.len()
    }

    /// Reset the cursor to the beginning for replay.
    pub fn reset(&mut self) {
        self.index = 0;
    }

    /// Consume all remaining events into a Vec.
    pub fn drain(&mut self) -> Vec<protocol_interface::core::CoreEvent> {
        let remaining: Vec<_> = self.events[self.index..].to_vec();
        self.index = self.events.len();
        remaining
    }
}

// ── VecEventSource builder helpers ─────────────────────────────────────

/// Build a `VecEventSource` from a sequence of text deltas.
///
/// Creates `ModelDelta` events followed by a `FinalResult` event.
pub fn text_event_source(
    run_ref: &protocol_interface::core::RunRef,
    trace_id: &protocol_interface::core::TraceId,
    deltas: &[&str],
    final_text: &str,
) -> VecEventSource {
    use protocol_interface::core::{CoreEvent, CoreEventKind, CoreEventPayload};

    let mut events = Vec::new();
    for (i, text) in deltas.iter().enumerate() {
        events.push(CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            i as u64,
            CoreEventKind::ModelDelta,
            CoreEventPayload::Text {
                text: text.to_string(),
            },
        ));
    }
    events.push(CoreEvent::new(
        run_ref.clone(),
        trace_id.clone(),
        deltas.len() as u64,
        CoreEventKind::FinalResult,
        CoreEventPayload::Final {
            content: final_text.to_string(),
            success: true,
        },
    ));
    VecEventSource::new(events)
}

/// Build a `VecEventSource` with a turn-started, deltas, and turn-completed sequence.
pub fn full_turn_event_source(
    run_ref: &protocol_interface::core::RunRef,
    trace_id: &protocol_interface::core::TraceId,
    response_text: &str,
) -> VecEventSource {
    use protocol_interface::core::{CoreEvent, CoreEventKind, CoreEventPayload};

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
                text: response_text.to_string(),
            },
        ),
        CoreEvent::new(
            run_ref.clone(),
            trace_id.clone(),
            2,
            CoreEventKind::FinalResult,
            CoreEventPayload::Final {
                content: response_text.to_string(),
                success: true,
            },
        ),
    ];
    VecEventSource::new(events)
}
