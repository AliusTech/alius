//! Shell Gate — command safety inspection, scope analysis, and authorization.
//!
//! The Shell Gate system prevents dangerous commands from executing by analyzing
//! shell commands before execution, classifying risk levels, checking workspace
//! scope, and enforcing authorization policies.

pub mod authorizer;
pub mod inspector;
pub mod scope;

pub use authorizer::{ShellGateConfig, ShellGateDecision};
pub use inspector::{RiskLevel, ShellInspection};
pub use scope::ScopeAnalysis;

use std::path::PathBuf;

/// A request to execute a shell command, with full context for analysis.
#[derive(Debug, Clone)]
pub struct ShellCommandRequest {
    /// The raw command string.
    pub command: String,
    /// Parsed arguments (best-effort).
    pub args: Vec<String>,
    /// Working directory where the command would execute.
    pub cwd: PathBuf,
    /// Origin of the request.
    pub origin: ShellOrigin,
    /// Workspace root directory.
    pub workspace_root: PathBuf,
}

/// Where the shell command request originated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellOrigin {
    /// Local CLI user (highest trust).
    LocalCli,
    /// Remote A2A protocol (restricted).
    RemoteA2A,
    /// Embedded SDK (restricted).
    Embedded,
}

/// Result of a full Shell Gate analysis.
#[derive(Debug, Clone)]
pub struct ShellGateResult {
    /// The inspection (parsed command, risk level).
    pub inspection: ShellInspection,
    /// The scope analysis (workspace boundaries).
    pub scope: ScopeAnalysis,
    /// The final authorization decision.
    pub decision: ShellGateDecision,
}
