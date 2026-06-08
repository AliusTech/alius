//! Workspace scope analysis for shell commands.

use std::path::{Path, PathBuf};

use super::ShellCommandRequest;

/// Result of scope analysis on a shell command.
#[derive(Debug, Clone)]
pub struct ScopeAnalysis {
    /// Whether the command's working directory is inside the workspace.
    pub cwd_inside_workspace: bool,
    /// Paths referenced in the command that are outside the workspace.
    pub external_paths: Vec<PathBuf>,
    /// Whether a symlink escape was detected.
    pub symlink_escape: bool,
}

/// Analyze whether a shell command operates within workspace boundaries.
pub fn analyze_scope(request: &ShellCommandRequest) -> ScopeAnalysis {
    let cwd_inside = is_inside_workspace(&request.cwd, &request.workspace_root);

    let mut external_paths = Vec::new();

    // Check arguments for paths outside workspace.
    for arg in &request.args {
        let path = PathBuf::from(arg);
        if path.is_absolute() && !is_inside_workspace(&path, &request.workspace_root) {
            external_paths.push(path);
        }
    }

    // Check command string for redirections to external paths.
    for part in request.command.split_whitespace() {
        if part.contains('>') || part.contains('<') {
            let path_str = part
                .trim_start_matches('>')
                .trim_start_matches('<')
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            if !path_str.is_empty() {
                let path = PathBuf::from(path_str);
                if path.is_absolute() && !is_inside_workspace(&path, &request.workspace_root) {
                    external_paths.push(path);
                }
            }
        }
    }

    // Check for symlink escapes.
    let symlink_escape = check_symlink_escape(&request.cwd, &request.workspace_root);

    ScopeAnalysis {
        cwd_inside_workspace: cwd_inside,
        external_paths,
        symlink_escape,
    }
}

/// Check whether a path falls inside the workspace root.
pub fn is_inside_workspace(path: &Path, workspace: &Path) -> bool {
    match (path.canonicalize(), workspace.canonicalize()) {
        (Ok(canonical_path), Ok(canonical_workspace)) => {
            canonical_path.starts_with(&canonical_workspace)
        }
        _ => {
            // Fallback: string comparison if canonicalize fails.
            let path_str = path.to_string_lossy();
            let ws_str = workspace.to_string_lossy();
            path_str.starts_with(ws_str.as_ref())
        }
    }
}

/// Check for symlink escape from workspace.
fn check_symlink_escape(cwd: &Path, workspace: &Path) -> bool {
    if let (Ok(canonical_cwd), Ok(canonical_ws)) = (cwd.canonicalize(), workspace.canonicalize()) {
        // If cwd resolves outside workspace, a symlink escape exists.
        !canonical_cwd.starts_with(&canonical_ws)
    } else {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_request(cwd: &str, workspace: &str) -> ShellCommandRequest {
        ShellCommandRequest {
            command: "echo hello".to_string(),
            args: vec![],
            cwd: PathBuf::from(cwd),
            origin: super::super::ShellOrigin::LocalCli,
            workspace_root: PathBuf::from(workspace),
        }
    }

    #[test]
    fn test_cwd_inside_workspace() {
        let req = make_request("/workspace/src", "/workspace");
        let analysis = analyze_scope(&req);
        assert!(analysis.cwd_inside_workspace);
    }

    #[test]
    fn test_cwd_outside_workspace() {
        let req = make_request("/etc", "/workspace");
        let analysis = analyze_scope(&req);
        assert!(!analysis.cwd_inside_workspace);
    }

    #[test]
    fn test_external_path_in_args() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "cp /etc/passwd /workspace/backup".to_string();
        req.args = vec!["/etc/passwd".to_string(), "/workspace/backup".to_string()];
        let analysis = analyze_scope(&req);
        assert!(!analysis.external_paths.is_empty());
    }
}
