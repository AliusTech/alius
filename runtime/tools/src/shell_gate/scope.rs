//! Workspace scope analysis for shell commands.

use std::path::{Component, Path, PathBuf};

use super::inspector::shell_tokenize;
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
    let args = scope_args(request);

    // Extract path candidates from args
    for candidate in path_candidates(&args) {
        if let Some(path) = resolve_candidate_path(&candidate, &request.cwd) {
            if !is_inside_workspace(&path, &request.workspace_root) {
                external_paths.push(path);
            }
        }
    }

    // Also check for redirections in the raw command string
    // This catches cases like "echo test > /tmp/out" where args=["test"]
    for candidate in extract_redirections_from_command(&request.command) {
        if let Some(path) = resolve_candidate_path(&candidate, &request.cwd) {
            if !is_inside_workspace(&path, &request.workspace_root) {
                external_paths.push(path);
            }
        }
    }

    external_paths.sort();
    external_paths.dedup();

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
    let canonical_workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| normalize_lexical(workspace));
    let canonical_path = path
        .canonicalize()
        .unwrap_or_else(|_| normalize_lexical(path));

    canonical_path == canonical_workspace || canonical_path.starts_with(&canonical_workspace)
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

/// Extract redirection targets from a raw command string.
/// This catches "echo test > /tmp/out" patterns where the redirection
/// is not included in the parsed args.
fn extract_redirections_from_command(command: &str) -> Vec<String> {
    let mut targets = Vec::new();
    let tokens = shell_tokenize(command);
    let mut expect_target = false;

    for token in &tokens {
        if expect_target {
            targets.push(token.clone());
            expect_target = false;
            continue;
        }

        if is_redirection_operator(token) {
            expect_target = true;
            continue;
        }

        if let Some(target) = inline_redirection_target(token) {
            targets.push(target);
        }
    }

    targets
}

fn scope_args(request: &ShellCommandRequest) -> Vec<String> {
    if request.args.is_empty() {
        shell_tokenize(&request.command)
            .into_iter()
            .skip(1)
            .collect()
    } else {
        request.args.clone()
    }
}

fn path_candidates(args: &[String]) -> Vec<String> {
    let mut candidates = Vec::new();
    let mut expect_redirection_target = false;

    for arg in args {
        if expect_redirection_target {
            candidates.push(arg.clone());
            expect_redirection_target = false;
            continue;
        }

        if is_redirection_operator(arg) {
            expect_redirection_target = true;
            continue;
        }

        if let Some(target) = inline_redirection_target(arg) {
            candidates.push(target);
            continue;
        }

        if let Some(value) = option_value(arg) {
            if looks_like_path(value) {
                candidates.push(value.to_string());
            }
            continue;
        }

        if looks_like_path(arg) {
            candidates.push(arg.clone());
        }
    }

    candidates
}

fn is_redirection_operator(arg: &str) -> bool {
    matches!(arg, ">" | ">>" | "<" | "<<" | "2>" | "2>>" | "1>" | "1>>")
}

fn inline_redirection_target(arg: &str) -> Option<String> {
    let trimmed = clean_token(arg);
    let without_fd = trimmed.trim_start_matches(|ch: char| ch.is_ascii_digit());
    for op in [">>", ">", "<<", "<"] {
        if let Some(target) = without_fd.strip_prefix(op) {
            let target = clean_token(target);
            if !target.is_empty() {
                return Some(target.to_string());
            }
        }
    }
    None
}

fn option_value(arg: &str) -> Option<&str> {
    arg.strip_prefix('-')?;
    arg.split_once('=').map(|(_, value)| value)
}

fn looks_like_path(arg: &str) -> bool {
    let cleaned = clean_token(arg);
    if cleaned.is_empty()
        || cleaned == "-"
        || cleaned.starts_with('$')
        || cleaned.starts_with('-')
        || cleaned.contains("://")
        || matches!(cleaned, "|" | "||" | "&&" | ";")
    {
        return false;
    }

    cleaned.starts_with('/')
        || cleaned.starts_with('~')
        || cleaned.starts_with('.')
        || cleaned.contains('/')
        || cleaned.contains('\\')
}

fn resolve_candidate_path(candidate: &str, cwd: &Path) -> Option<PathBuf> {
    let cleaned = clean_token(candidate);
    if cleaned.is_empty() || cleaned.contains("://") {
        return None;
    }

    let expanded = expand_home(cleaned);
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        cwd.join(expanded)
    };
    Some(normalize_lexical(&absolute))
}

fn clean_token(token: &str) -> &str {
    token
        .trim()
        .trim_matches('"')
        .trim_matches('\'')
        .trim_end_matches(';')
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME")
        .or_else(|| std::env::var_os("USERPROFILE"))
        .map(PathBuf::from)
}

fn normalize_lexical(path: &Path) -> PathBuf {
    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new(std::path::MAIN_SEPARATOR_STR)),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(part) => normalized.push(part),
        }
    }
    normalized
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

    #[test]
    fn test_external_path_from_command_when_args_empty() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "cat /etc/passwd".to_string();
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/etc/passwd")]);
    }

    #[test]
    fn test_relative_parent_escape_is_external() {
        let mut req = make_request("/workspace/project", "/workspace/project");
        req.command = "cat ../secrets.txt".to_string();
        let analysis = analyze_scope(&req);
        assert_eq!(
            analysis.external_paths,
            vec![PathBuf::from("/workspace/secrets.txt")]
        );
    }

    #[test]
    fn test_split_redirection_target_is_external() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "echo hello > /tmp/out.txt".to_string();
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/tmp/out.txt")]);
    }

    #[test]
    fn test_inline_redirection_target_is_external() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "echo hello 2>/tmp/error.txt".to_string();
        let analysis = analyze_scope(&req);
        assert_eq!(
            analysis.external_paths,
            vec![PathBuf::from("/tmp/error.txt")]
        );
    }

    #[test]
    fn test_workspace_prefix_collision_is_not_inside() {
        assert!(!is_inside_workspace(
            Path::new("/workspace2/file.txt"),
            Path::new("/workspace")
        ));
    }

    #[test]
    fn test_etc_passwd_is_external() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "cat /etc/passwd".to_string();
        req.args = vec!["/etc/passwd".to_string()];
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/etc/passwd")]);
    }

    #[test]
    fn test_parent_escape_outside_is_external() {
        let mut req = make_request("/workspace/subdir", "/workspace");
        req.command = "cat ../outside/file.txt".to_string();
        req.args = vec!["../outside/file.txt".to_string()];
        let _analysis = analyze_scope(&req);
        // ../outside from /workspace/subdir resolves to /workspace/outside
        // which is inside workspace, so this should be empty
        // Let's test a deeper escape
        req.cwd = PathBuf::from("/workspace");
        req.command = "cat ../outside/file.txt".to_string();
        req.args = vec!["../outside/file.txt".to_string()];
        let analysis = analyze_scope(&req);
        // ../outside from /workspace resolves to /outside which is external
        assert!(!analysis.external_paths.is_empty());
    }

    #[test]
    fn test_output_flag_with_external_path() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "gcc main.c --output=/tmp/out".to_string();
        req.args = vec!["main.c".to_string(), "--output=/tmp/out".to_string()];
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/tmp/out")]);
    }

    #[test]
    fn test_redirection_operator_with_tmp() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "echo test > /tmp/out".to_string();
        req.args = vec!["test".to_string()];
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/tmp/out")]);
    }

    #[test]
    fn test_stderr_redirection_to_tmp() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "command 2>/tmp/err".to_string();
        req.args = vec![];
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/tmp/err")]);
    }

    #[test]
    fn test_cwd_absolute_path_outside_workspace() {
        let req = make_request("/tmp", "/workspace");
        let analysis = analyze_scope(&req);
        assert!(!analysis.cwd_inside_workspace);
    }

    #[test]
    fn test_cwd_parent_escape_outside_workspace() {
        let mut req = make_request("/workspace", "/workspace");
        req.cwd = PathBuf::from("/workspace/../outside");
        let analysis = analyze_scope(&req);
        assert!(!analysis.cwd_inside_workspace);
    }

    #[test]
    fn test_args_override_empty_args_fallback() {
        let mut req = make_request("/workspace", "/workspace");
        req.command = "cat /etc/passwd".to_string();
        req.args = vec![]; // Empty args should fallback to parsing command
        let analysis = analyze_scope(&req);
        assert_eq!(analysis.external_paths, vec![PathBuf::from("/etc/passwd")]);
    }
}
