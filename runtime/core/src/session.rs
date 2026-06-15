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
    confirmation: HashMap<String, tokio::sync::oneshot::Sender<bool>>,
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

    /// Store a oneshot sender for a pending tool confirmation (Stage B).
    /// The loop engine awaits the matching receiver; the TUI delivers the
    /// user's response via `deliver_confirmation`.
    pub fn store_confirmation_sender(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        sender: tokio::sync::oneshot::Sender<bool>,
    ) -> Result<(), ProtocolError> {
        let mut runs = self.runs.write().unwrap();
        let state = runs
            .get_mut(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
        state.confirmation.insert(tool_call_id.to_string(), sender);
        Ok(())
    }

    /// Deliver the user's yes/no to a pending tool confirmation (Stage B).
    /// Sends on the stored oneshot and removes it; the awaiting loop resumes.
    pub fn deliver_confirmation(
        &self,
        run_ref: &RunRef,
        tool_call_id: &str,
        approved: bool,
    ) -> Result<(), ProtocolError> {
        let mut runs = self.runs.write().unwrap();
        let state = runs
            .get_mut(run_ref.as_str())
            .ok_or_else(|| ProtocolError::RunNotFound(run_ref.clone()))?;
        match state.confirmation.remove(tool_call_id) {
            Some(sender) => {
                let _ = sender.send(approved);
                Ok(())
            }
            None => Err(ProtocolError::Internal(format!(
                "no pending confirmation for tool_call_id {tool_call_id}"
            ))),
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

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(&run_ref, "call-1", tx)
            .unwrap();

        // Deliver the user's approval.
        mgr.deliver_confirmation(&run_ref, "call-1", true).unwrap();
        let approved = rx.await.unwrap();
        assert!(approved);

        // A second deliver for the same id fails (sender already consumed).
        assert!(mgr.deliver_confirmation(&run_ref, "call-1", true).is_err());
    }

    #[tokio::test]
    async fn confirmation_cancel_drops_sender() {
        let (mgr, run_ref) = mgr_with_run();

        let (tx, rx) = tokio::sync::oneshot::channel::<bool>();
        mgr.store_confirmation_sender(&run_ref, "call-2", tx)
            .unwrap();

        // Cancel drops the sender → receiver gets Err → treated as denied.
        mgr.cancel_pending_confirmations(&run_ref);
        let res = rx.await;
        assert!(res.is_err());
    }
}
