//! Session Manager — workspace/session/turn/run/trace lifecycle management.

use std::collections::HashMap;
use std::sync::RwLock;

use chrono::Utc;

use protocol_interface::core::*;

/// Manages session, turn, and run lifecycle within a workspace.
pub struct SessionManager {
    workspace_ref: WorkspaceRef,
    sessions: RwLock<HashMap<String, SessionSnapshot>>,
    runs: RwLock<HashMap<String, RunState>>,
}

struct RunState {
    events: Vec<CoreEvent>,
    status: RunStatus,
    /// Pending tool-confirmation oneshot senders (Stage B). Keyed by tool_call_id.
    /// When the user responds (or the run is cancelled), the sender is removed;
    /// dropping it makes the receiver's `await` return Err (treated as denied).
    /// Stores (sender, tool_name, trace_id) for audit purposes on delivery failure.
    confirmation: HashMap<String, (tokio::sync::oneshot::Sender<bool>, String, TraceId)>,
    /// Cancellation token for stopping the run. Checked by loop engine.
    cancel_token: tokio_util::sync::CancellationToken,
}

impl SessionManager {
    pub fn new(workspace_ref: WorkspaceRef) -> Self {
        Self {
            workspace_ref,
            sessions: RwLock::new(HashMap::new()),
            runs: RwLock::new(HashMap::new()),
        }
    }

    /// Return the workspace root path.
    pub fn workspace_root(&self) -> std::path::PathBuf {
        self.workspace_ref.root.clone()
    }

    /// Create a new session in the workspace.
    pub fn create_session(&self) -> SessionSnapshot {
        let session_ref = SessionRef::new();
        let snapshot = SessionSnapshot {
            session_ref: session_ref.clone(),
            workspace_ref: self.workspace_ref.clone(),
            status: SessionStatus::Open,
            runs: Vec::new(),
            updated_at: Utc::now(),
        };

        let mut sessions = self.sessions.write().unwrap();
        sessions.insert(session_ref.as_str().to_string(), snapshot.clone());
        snapshot
    }

    /// Create a turn and run within a session.
    pub fn create_turn(
        &self,
        session_ref: &SessionRef,
    ) -> Result<(TurnRef, RunRef, TraceId), ProtocolError> {
        let mut sessions = self.sessions.write().unwrap();
        let snapshot = sessions
            .get_mut(session_ref.as_str())
            .ok_or_else(|| ProtocolError::SessionNotFound(session_ref.clone()))?;

        if snapshot.status != SessionStatus::Open {
            return Err(ProtocolError::InvalidMessage(
                "session is not open".to_string(),
            ));
        }

        let turn_ref = TurnRef::new();
        let run_ref = RunRef::new();
        let trace_id = TraceId::new();

        let run_summary = RunSummary {
            run_ref: run_ref.clone(),
            trace_id: trace_id.clone(),
            status: RunStatus::Started,
            started_at: Utc::now(),
            finished_at: None,
        };

        snapshot.runs.push(run_summary);
        snapshot.updated_at = Utc::now();
        drop(sessions);

        let mut runs = self.runs.write().unwrap();
        runs.insert(
            run_ref.as_str().to_string(),
            RunState {
                events: Vec::new(),
                status: RunStatus::Started,
                confirmation: HashMap::new(),
                cancel_token: tokio_util::sync::CancellationToken::new(),
            },
        );

        Ok((turn_ref, run_ref, trace_id))
    }

    /// Get a session snapshot.
    pub fn get_session(&self, session_ref: &SessionRef) -> Result<SessionSnapshot, ProtocolError> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .get(session_ref.as_str())
            .cloned()
            .ok_or_else(|| ProtocolError::SessionNotFound(session_ref.clone()))
    }

    /// List all sessions in the workspace.
    pub fn list_sessions(&self) -> Vec<SessionSummary> {
        let sessions = self.sessions.read().unwrap();
        sessions
            .values()
            .map(|s| SessionSummary {
                session_ref: s.session_ref.clone(),
                workspace_ref: s.workspace_ref.clone(),
                name: None,
                purpose: SessionPurpose::General,
                updated_at: s.updated_at,
            })
            .collect()
    }

    /// Push an event into a run's event buffer.
    pub fn push_event(&self, run_ref: &RunRef, event: CoreEvent) -> Result<(), ProtocolError> {
        let mut runs = self.runs.write().unwrap();
        let state = runs
            .get_mut(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
        state.events.push(event);
        Ok(())
    }

    /// Get all events for a run.
    pub fn get_events(&self, run_ref: &RunRef) -> Result<Vec<CoreEvent>, ProtocolError> {
        let runs = self.runs.read().unwrap();
        let state = runs
            .get(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
        Ok(state.events.clone())
    }

    /// Get the next sequence number for a run (max existing sequence + 1).
    /// Returns 1 if no events exist yet.
    pub fn next_event_sequence(&self, run_ref: &RunRef) -> u64 {
        let runs = self.runs.read().unwrap();
        runs.get(run_ref.as_str())
            .map(|state| state.events.iter().map(|e| e.sequence).max().unwrap_or(0) + 1)
            .unwrap_or(1)
    }

    /// Close a session, preventing new turns.
    pub fn close(&self, session_ref: &SessionRef) -> Result<(), ProtocolError> {
        let mut sessions = self.sessions.write().unwrap();
        let snapshot = sessions
            .get_mut(session_ref.as_str())
            .ok_or_else(|| ProtocolError::SessionNotFound(session_ref.clone()))?;
        snapshot.status = SessionStatus::Closed;
        snapshot.updated_at = Utc::now();
        Ok(())
    }

    /// Update run status and propagate to session snapshot.
    pub fn update_run_status(
        &self,
        run_ref: &RunRef,
        status: RunStatus,
    ) -> Result<(), ProtocolError> {
        {
            let mut runs = self.runs.write().unwrap();
            let state = runs
                .get_mut(run_ref.as_str())
                .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
            state.status = status.clone();
        }

        // Update the run in the session snapshot too
        let sessions = self
            .sessions
            .read()
            .unwrap()
            .keys()
            .cloned()
            .collect::<Vec<_>>();
        drop(sessions);

        let mut sessions = self.sessions.write().unwrap();
        for snapshot in sessions.values_mut() {
            for run in &mut snapshot.runs {
                if run.run_ref == *run_ref {
                    run.status = status.clone();
                    if run.finished_at.is_none()
                        && matches!(
                            run.status,
                            RunStatus::Completed | RunStatus::Failed | RunStatus::Cancelled
                        )
                    {
                        run.finished_at = Some(Utc::now());
                    }
                    break;
                }
            }
        }

        Ok(())
    }

    /// Get the current run status.
    pub fn get_run_status(&self, run_ref: &RunRef) -> Result<RunStatus, ProtocolError> {
        let runs = self.runs.read().unwrap();
        runs.get(run_ref.as_str())
            .map(|s| s.status.clone())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))
    }

    /// Store a oneshot sender for a pending tool confirmation (Stage B).
    /// The loop engine awaits the matching receiver; the TUI delivers the
    /// user's response via `deliver_confirmation`.
    /// Also stores tool_name and trace_id for audit logging on delivery failure.
    pub fn store_confirmation_sender(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        tool_name: &str,
        trace_id: &TraceId,
        sender: tokio::sync::oneshot::Sender<bool>,
    ) -> Result<(), ProtocolError> {
        let mut runs = self.runs.write().unwrap();
        let state = runs
            .get_mut(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
        state.confirmation.insert(
            tool_call_id.to_string(),
            (sender, tool_name.to_string(), trace_id.clone()),
        );
        Ok(())
    }

    /// Deliver the user's yes/no to a pending tool confirmation (Stage B).
    /// Sends on the stored oneshot and removes it; the awaiting loop resumes.
    /// Only restores status to `Running` if the run is still in
    /// `WaitingForApproval` — cancelled or terminal states are preserved.
    /// The status check + update is atomic (under the same write lock) to
    /// prevent a race with `cancel_run`.
    ///
    /// Returns Ok(tool_name) on successful delivery.
    /// Returns Err((ProtocolError, tool_name)) if:
    ///   - Run not found
    ///   - No pending confirmation for tool_call_id
    ///   - Receiver dropped (sender.send fails)
    ///
    /// For no-pending and run-not-found cases, tool_name is "unknown" (sentinel).
    pub fn deliver_confirmation(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        approved: bool,
    ) -> Result<String, (ProtocolError, String)> {
        let mut runs = self.runs.write().unwrap();
        let state = runs.get_mut(run_ref.as_str()).ok_or_else(|| {
            (
                ProtocolError::RunNotFound(run_ref.clone()),
                "unknown".to_string(),
            )
        })?;
        match state.confirmation.remove(tool_call_id) {
            Some((sender, tool_name, _trace_id)) => {
                // Check if receiver is still alive
                if sender.send(approved).is_err() {
                    // Receiver dropped - this is a delivery failure
                    return Err((
                        ProtocolError::Internal(format!(
                            "confirmation receiver dropped for tool_call_id {tool_call_id}"
                        )),
                        tool_name,
                    ));
                }

                // Atomic: check + update under the same lock.
                // Only restore to Running if still in WaitingForApproval.
                if approved && state.status == RunStatus::WaitingForApproval {
                    state.status = RunStatus::Running;
                    // Propagate to session snapshot while still holding lock.
                    let mut sessions = self.sessions.write().unwrap();
                    for snapshot in sessions.values_mut() {
                        for run in &mut snapshot.runs {
                            if run.run_ref == *run_ref {
                                run.status = RunStatus::Running;
                                break;
                            }
                        }
                    }
                }
                Ok(tool_name)
            }
            None => Err((
                ProtocolError::Internal(format!(
                    "no pending confirmation for tool_call_id {tool_call_id}"
                )),
                "unknown".to_string(),
            )),
        }
    }

    /// Drop all pending confirmation senders for a run (Stage B).
    /// Called on Cancel — receivers get Err and treat it as denied.
    pub fn cancel_pending_confirmations(&self, run_ref: &RunRef) {
        let mut runs = self.runs.write().unwrap();
        if let Some(state) = runs.get_mut(run_ref.as_str()) {
            state.confirmation.clear();
        }
    }

    /// Cancel a run by triggering its cancellation token.
    /// This stops the loop execution and updates status to Cancelled.
    /// Emits a cancellation event if events exist.
    pub fn cancel_run(&self, run_ref: &RunRef) -> Result<(), ProtocolError> {
        let runs = self.runs.read().unwrap();
        let state = runs
            .get(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;

        state.cancel_token.cancel();

        // Extract trace_id and next sequence from existing events
        let (trace_id, next_seq) = if let Some(first) = state.events.first() {
            let max_seq = state.events.iter().map(|e| e.sequence).max().unwrap_or(0);
            (Some(first.trace_id.clone()), max_seq + 1)
        } else {
            (None, 1)
        };
        drop(runs);

        self.update_run_status(run_ref, RunStatus::Cancelled)?;
        self.cancel_pending_confirmations(run_ref);

        // Persist cancellation event if we have a trace_id
        if let Some(trace_id) = trace_id {
            let cancel_event = CoreEvent::new(
                run_ref.clone(),
                trace_id,
                next_seq,
                CoreEventKind::RunCancelled,
                CoreEventPayload::Json {
                    value: serde_json::json!({
                        "reason": "user_requested"
                    }),
                },
            );
            let _ = self.push_event(run_ref, cancel_event);
        }

        Ok(())
    }

    /// Check if a run has been cancelled.
    pub fn is_cancelled(&self, run_ref: &RunRef) -> bool {
        let runs = self.runs.read().unwrap();
        runs.get(run_ref.as_str())
            .map(|s| s.cancel_token.is_cancelled())
            .unwrap_or(false)
    }

    /// Get a clone of the cancel token for a run.
    /// Used to pass the token to the loop engine.
    pub fn get_cancel_token(
        &self,
        run_ref: &RunRef,
    ) -> Result<tokio_util::sync::CancellationToken, ProtocolError> {
        let runs = self.runs.read().unwrap();
        let state = runs
            .get(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
        Ok(state.cancel_token.clone())
    }

    /// Handle automatic status transitions based on event kinds.
    /// Called after pushing an event to maintain status consistency.
    /// Does not transition if already in a terminal state (Cancelled).
    pub fn handle_event_status_transition(
        &self,
        run_ref: &RunRef,
        event: &CoreEvent,
    ) -> Result<(), ProtocolError> {
        // Check if run is already in a terminal state
        let current_status = {
            let runs = self.runs.read().unwrap();
            runs.get(run_ref.as_str())
                .map(|s| s.status.clone())
                .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?
        };

        // Don't transition from terminal states (Cancelled, Completed, Failed)
        if matches!(
            current_status,
            RunStatus::Cancelled | RunStatus::Completed | RunStatus::Failed
        ) {
            return Ok(());
        }

        match (&event.kind, &event.payload) {
            (CoreEventKind::FinalResult, CoreEventPayload::Final { success: true, .. }) => {
                self.update_run_status(run_ref, RunStatus::Completed)?;
            }
            (CoreEventKind::FinalResult, CoreEventPayload::Final { success: false, .. }) => {
                self.update_run_status(run_ref, RunStatus::Failed)?;
            }
            (CoreEventKind::ErrorRaised, _) => {
                self.update_run_status(run_ref, RunStatus::Failed)?;
            }
            (CoreEventKind::ToolConfirmationRequired, _) => {
                self.update_run_status(run_ref, RunStatus::WaitingForApproval)?;
            }
            _ => {}
        }
        Ok(())
    }

    /// Get all run refs across all sessions.
    pub fn all_run_refs(&self) -> Vec<(RunRef, Option<SessionRef>)> {
        let sessions = self.sessions.read().unwrap();
        let runs = self.runs.read().unwrap();
        let mut result = Vec::new();
        for snapshot in sessions.values() {
            for run_summary in &snapshot.runs {
                if runs.contains_key(run_summary.run_ref.as_str()) {
                    result.push((
                        run_summary.run_ref.clone(),
                        Some(snapshot.session_ref.clone()),
                    ));
                }
            }
        }
        result
    }

    /// Get run refs for a specific session.
    pub fn run_refs_for_session(
        &self,
        session_ref: &SessionRef,
    ) -> Result<Vec<RunRef>, ProtocolError> {
        let sessions = self.sessions.read().unwrap();
        let snapshot = sessions
            .get(session_ref.as_str())
            .ok_or_else(|| ProtocolError::SessionNotFound(session_ref.clone()))?;
        Ok(snapshot.runs.iter().map(|r| r.run_ref.clone()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use protocol_interface::core::WorkspaceRef;

    fn mgr_with_run() -> (SessionManager, RunRef) {
        let mgr = SessionManager::new(WorkspaceRef::new("/tmp"));
        let session = mgr.create_session();
        let (_turn, run_ref, _trace) = mgr.create_turn(&session.session_ref).unwrap();
        (mgr, run_ref)
    }

    #[tokio::test]
    async fn confirmation_store_deliver_roundtrip() {
        let (mgr, run_ref) = mgr_with_run();

        // Simulate entering WaitingForApproval state.
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "call-1",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx,
        )
        .unwrap();

        // Deliver the user's approval → status restored to Running.
        mgr.deliver_confirmation(&run_ref, "call-1", true).unwrap();
        let approved = rx.await.unwrap();
        assert!(approved);
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Running);

        // A second deliver for the same id fails (sender already consumed).
        assert!(mgr.deliver_confirmation(&run_ref, "call-1", true).is_err());
    }

    #[tokio::test]
    async fn confirmation_cancel_drops_sender() {
        let (mgr, run_ref) = mgr_with_run();

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "call-2",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx,
        )
        .unwrap();

        // Cancel drops the sender → receiver gets Err → treated as denied.
        mgr.cancel_pending_confirmations(&run_ref);
        let res = rx.await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn deliver_confirmation_does_not_restore_cancelled() {
        let (mgr, run_ref) = mgr_with_run();

        // Simulate the full cancel flow: store sender → cancel run.
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "call-3",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx,
        )
        .unwrap();
        mgr.cancel_run(&run_ref).unwrap();

        // Sender was dropped by cancel → rx returns Err.
        assert!(rx.await.is_err());

        // Status must remain Cancelled, not restored to Running.
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Cancelled);
    }

    #[tokio::test]
    async fn deliver_confirmation_does_not_restore_after_user_deny_via_cancel() {
        let (mgr, run_ref) = mgr_with_run();

        // Enter WaitingForApproval.
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "call-4",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx,
        )
        .unwrap();

        // Simulate cancel while waiting.
        mgr.cancel_run(&run_ref).unwrap();

        // Sender dropped → rx Err.
        assert!(rx.await.is_err());

        // Status must be Cancelled.
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Cancelled);
    }

    #[tokio::test]
    async fn cancel_run_clears_all_pending_confirmations() {
        let (mgr, run_ref) = mgr_with_run();

        let (tx1, _rx1) = tokio::sync::oneshot::channel::<bool>();
        let (tx2, _rx2) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "c1",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx1,
        )
        .unwrap();
        mgr.store_confirmation_sender(
            &run_ref,
            "c2",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx2,
        )
        .unwrap();

        mgr.cancel_run(&run_ref).unwrap();

        // Both senders dropped → receivers get Err.
        assert!(_rx1.await.is_err());
        assert!(_rx2.await.is_err());
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Cancelled);
    }

    #[test]
    fn get_run_status_returns_current() {
        let (mgr, run_ref) = mgr_with_run();
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Started);

        mgr.update_run_status(&run_ref, RunStatus::Running).unwrap();
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Running);

        mgr.update_run_status(&run_ref, RunStatus::Failed).unwrap();
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Failed);
    }

    #[tokio::test]
    async fn deliver_approved_does_not_overwrite_cancelled() {
        // Simulate: cancel sets Cancelled, then deliver_confirmation is called
        // with approved=true. The sender was dropped by cancel, so deliver
        // returns Err. Status must remain Cancelled.
        let (mgr, run_ref) = mgr_with_run();
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let (tx, _rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "tc-1",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx,
        )
        .unwrap();

        // Cancel clears senders and sets Cancelled.
        mgr.cancel_run(&run_ref).unwrap();
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Cancelled);

        // deliver_confirmation with approved=true: sender gone → Err.
        assert!(mgr.deliver_confirmation(&run_ref, "tc-1", true).is_err());
        // Status must still be Cancelled.
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Cancelled);
    }

    #[tokio::test]
    async fn deliver_approved_does_not_overwrite_failed() {
        // Simulate: an error sets Failed while confirmation is pending,
        // then deliver_confirmation is called with approved=true.
        // Status must remain Failed (not restored to Running).
        let (mgr, run_ref) = mgr_with_run();
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(
            &run_ref,
            "tc-2",
            "test_tool",
            &protocol_interface::TraceId::new(),
            tx,
        )
        .unwrap();

        // An error sets Failed.
        mgr.update_run_status(&run_ref, RunStatus::Failed).unwrap();

        // Sender is still in the map (not cleared by cancel).
        // deliver_confirmation with approved=true: sender consumed, status
        // is Failed (not WaitingForApproval) → must NOT restore to Running.
        mgr.deliver_confirmation(&run_ref, "tc-2", true).unwrap();
        assert!(rx.await.unwrap()); // approved signal received
        assert_eq!(mgr.get_run_status(&run_ref).unwrap(), RunStatus::Failed);
    }

    // ===== P3-3 Delivery Failure Audit Tests =====

    #[tokio::test]
    async fn delivery_failed_receiver_dropped_returns_error() {
        // Scenario: receiver is dropped before sender.send()
        // This simulates a delivery failure where the confirmation
        // can never be delivered.
        let (mgr, run_ref) = mgr_with_run();
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let trace_id = protocol_interface::TraceId::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(&run_ref, "tc-fail", "shell", &trace_id, tx)
            .unwrap();

        // Drop the receiver to simulate delivery failure
        drop(rx);

        // deliver_confirmation should return Err with tool_name
        let result = mgr.deliver_confirmation(&run_ref, "tc-fail", true);
        assert!(result.is_err());
        let (err, tool_name) = result.unwrap_err();
        assert!(err.to_string().contains("receiver dropped"));
        assert_eq!(tool_name, "shell");
    }

    #[tokio::test]
    async fn delivery_failed_no_pending_confirmation_returns_unknown_sentinel() {
        // Scenario: no pending confirmation for the tool_call_id
        // tool_name should be "unknown" sentinel (not empty)
        let (mgr, run_ref) = mgr_with_run();

        let result = mgr.deliver_confirmation(&run_ref, "nonexistent", true);
        assert!(result.is_err());
        let (err, tool_name) = result.unwrap_err();
        assert!(err.to_string().contains("no pending confirmation"));
        // tool_name uses "unknown" sentinel when no pending exists
        assert_eq!(tool_name, "unknown");
        assert!(!tool_name.is_empty());
    }

    #[tokio::test]
    async fn delivery_failed_preserves_tool_name_from_pending() {
        // Verify that delivery failure from receiver-dropped preserves tool_name
        let (mgr, run_ref) = mgr_with_run();
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let trace_id = protocol_interface::TraceId::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(&run_ref, "tc-meta", "write_file", &trace_id, tx)
            .unwrap();

        // Drop receiver to trigger delivery failure
        drop(rx);

        let result = mgr.deliver_confirmation(&run_ref, "tc-meta", false);
        assert!(result.is_err());
        let (_err, tool_name) = result.unwrap_err();

        // Verify non-empty tool_name for audit
        assert_eq!(tool_name, "write_file");
        assert!(!tool_name.is_empty());
    }

    #[tokio::test]
    async fn delivery_success_returns_tool_name() {
        // Verify successful delivery returns proper metadata
        let (mgr, run_ref) = mgr_with_run();
        mgr.update_run_status(&run_ref, RunStatus::WaitingForApproval)
            .unwrap();

        let trace_id = protocol_interface::TraceId::new();
        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(&run_ref, "tc-ok", "edit_file", &trace_id, tx)
            .unwrap();

        let result = mgr.deliver_confirmation(&run_ref, "tc-ok", true);
        assert!(result.is_ok());
        let tool_name = result.unwrap();
        assert_eq!(tool_name, "edit_file");

        // Verify receiver got the approval signal
        assert!(rx.await.unwrap());
    }
}
