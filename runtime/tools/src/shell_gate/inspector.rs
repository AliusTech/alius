//! Command parsing and risk classification.

use super::ShellCommandRequest;

/// Risk level assigned to a shell command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

/// Parsed inspection of a shell command.
#[derive(Debug, Clone)]
pub struct ShellInspection {
    /// Original command string.
    pub raw_command: String,
    /// Base command (first token).
    pub base_command: String,
    /// Parsed arguments.
    pub args: Vec<String>,
    /// Detected output redirections.
    pub redirects: Vec<String>,
    /// Whether the command uses pipes.
    pub has_pipe: bool,
    /// Assessed risk level.
    pub risk_level: RiskLevel,
}

/// Commands that are always denied regardless of arguments.
const DENYLIST_COMMANDS: &[&str] = &["mkfs", "dd", "fork-bomb"];

/// Commands that are denied when combined with dangerous flags.
const CRITICAL_COMMANDS: &[&str] = &["rm", "chmod", "chown"];

/// Commands that elevate risk to Critical.
const ELEVATION_COMMANDS: &[&str] = &["sudo", "su", "doas", "pkexec"];

/// Commands that are always Low risk.
const LOW_RISK_COMMANDS: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "find", "wc", "sort", "uniq", "diff", "echo", "pwd",
    "whoami", "which", "type", "stat", "file", "tree", "rg", "ag", "fd", "bat",
];

/// Parse a shell command string into a ShellInspection.
pub fn parse_command(request: &ShellCommandRequest) -> ShellInspection {
    let raw = &request.command;
    let tokens = shell_tokenize(raw);

    let base_command = tokens.first().cloned().unwrap_or_default();
    let args: Vec<String> = tokens.iter().skip(1).cloned().collect();

    let redirects: Vec<String> = args
        .iter()
        .filter(|a| a.starts_with('>') || a.starts_with('<') || *a == ">>")
        .cloned()
        .collect();

    let has_pipe = raw.contains('|');

    let risk_level = classify_risk(&base_command, &args, raw);

    ShellInspection {
        raw_command: raw.clone(),
        base_command,
        args,
        redirects,
        has_pipe,
        risk_level,
    }
}

/// Classify the risk level of a command based on its base command and arguments.
pub fn classify_risk(base_command: &str, args: &[String], raw: &str) -> RiskLevel {
    let cmd_lower = base_command.to_lowercase();

    // Hard denylist — always Critical.
    if DENYLIST_COMMANDS
        .iter()
        .any(|&d| cmd_lower == d || cmd_lower.starts_with(&format!("{}.", d)))
    {
        return RiskLevel::Critical;
    }

    // Elevation commands — always Critical.
    if ELEVATION_COMMANDS.contains(&cmd_lower.as_str()) {
        return RiskLevel::Critical;
    }

    // rm with recursive force patterns — Critical.
    if cmd_lower == "rm" {
        let raw_lower = raw.to_lowercase();
        // Parse flags from individual args to correctly detect combined flags like -rf.
        let flags: String = args
            .iter()
            .filter(|a| a.starts_with('-') && !a.starts_with("--"))
            .map(|a| a.trim_start_matches('-'))
            .flat_map(|s| s.chars())
            .collect();
        let has_recursive =
            flags.contains('r') || flags.contains('R') || raw_lower.contains("--recursive");
        let has_force = flags.contains('f') || raw_lower.contains("--force");

        // rm -rf targeting root, home, dot, or star — always Critical
        if has_recursive && has_force {
            let targets_root =
                raw == "rm -rf /" || raw.ends_with("rm -rf /") || raw.contains("rm -rf / ");
            let targets_home =
                raw == "rm -rf ~" || raw.contains("rm -rf ~") || raw.contains("rm -rf ~/");
            let targets_dot = raw == "rm -rf ." || raw.ends_with("rm -rf .");
            let targets_star = raw == "rm -rf *" || raw.ends_with("rm -rf *");

            if targets_root || targets_home || targets_dot || targets_star {
                return RiskLevel::Critical;
            }
            return RiskLevel::High;
        }

        // rm -r without force is still high risk if targeting broad paths
        if has_recursive {
            return RiskLevel::High;
        }

        return RiskLevel::Medium;
    }

    // chmod/chown on system paths
    if CRITICAL_COMMANDS.contains(&cmd_lower.as_str()) {
        let args_str = args.join(" ");
        if args_str.contains("/etc") || args_str.contains("/usr") || args_str.contains("/sys") {
            return RiskLevel::Critical;
        }
        return RiskLevel::Medium;
    }

    if cmd_lower == "git" {
        return classify_git_risk(args);
    }

    // Known low-risk commands
    if LOW_RISK_COMMANDS.contains(&cmd_lower.as_str()) {
        return RiskLevel::Low;
    }

    // Write-type commands
    let write_commands = ["cp", "mv", "mkdir", "touch", "ln", "install"];
    if write_commands.contains(&cmd_lower.as_str()) {
        return RiskLevel::Medium;
    }

    // Network commands
    let net_commands = ["curl", "wget", "nc", "ncat", "ssh", "scp", "rsync"];
    if net_commands.contains(&cmd_lower.as_str()) {
        return RiskLevel::Medium;
    }

    // Default to Medium for unrecognized commands
    RiskLevel::Medium
}

fn classify_git_risk(args: &[String]) -> RiskLevel {
    let Some(subcommand) = git_subcommand(args) else {
        return RiskLevel::Low;
    };

    match subcommand {
        "status" | "log" | "diff" | "show" | "branch" => RiskLevel::Low,
        "clean" => RiskLevel::High,
        "reset" if args.iter().any(|arg| arg == "--hard") => RiskLevel::High,
        "clone" | "fetch" | "pull" | "submodule" => RiskLevel::Medium,
        "push" | "checkout" | "switch" | "merge" | "rebase" | "reset" | "restore" | "add"
        | "commit" => RiskLevel::Medium,
        _ => RiskLevel::Medium,
    }
}

fn git_subcommand(args: &[String]) -> Option<&str> {
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "-C" | "--git-dir" | "--work-tree" => {
                index += 2;
            }
            arg if arg.starts_with("--git-dir=") || arg.starts_with("--work-tree=") => {
                index += 1;
            }
            arg if arg.starts_with('-') => {
                index += 1;
            }
            arg => return Some(arg),
        }
    }
    None
}

/// Return best-effort command arguments for authorization and scope analysis.
pub fn command_args(command: &str) -> Vec<String> {
    shell_tokenize(command).into_iter().skip(1).collect()
}

/// Simple shell tokenizer — handles quoted strings and basic escaping.
pub(crate) fn shell_tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    let mut escape = false;

    for ch in input.chars() {
        if escape {
            current.push(ch);
            escape = false;
            continue;
        }
        match ch {
            '\\' => {
                escape = true;
            }
            '"' | '\'' if quote == Some(ch) => {
                quote = None;
            }
            '"' | '\'' if quote.is_none() => {
                quote = Some(ch);
            }
            ' ' | '\t' if quote.is_none() => {
                if !current.is_empty() {
                    tokens.push(std::mem::take(&mut current));
                }
            }
            _ => {
                current.push(ch);
            }
        }
    }
    if escape {
        current.push('\\');
    }
    if !current.is_empty() {
        tokens.push(current);
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn make_request(cmd: &str) -> ShellCommandRequest {
        ShellCommandRequest {
            command: cmd.to_string(),
            args: vec![],
            cwd: PathBuf::from("/workspace"),
            origin: super::super::ShellOrigin::LocalCli,
            workspace_root: PathBuf::from("/workspace"),
        }
    }

    #[test]
    fn test_deny_rm_rf_root() {
        let req = make_request("rm -rf /");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_deny_rm_rf_home() {
        let req = make_request("rm -rf ~");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_deny_rm_rf_dot() {
        let req = make_request("rm -rf .");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_deny_rm_rf_star() {
        let req = make_request("rm -rf *");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_deny_sudo() {
        let req = make_request("sudo apt install foo");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_approve_git_status() {
        let req = make_request("git status");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_git_clone_is_medium_risk() {
        let req = make_request("git clone https://github.com/lc345/repo.git");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_git_global_option_then_status_is_low_risk() {
        let req = make_request("git -C repo status");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_git_reset_hard_is_high_risk() {
        let req = make_request("git reset --hard HEAD");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_approve_ls() {
        let req = make_request("ls -la /workspace/src");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_approve_cat() {
        let req = make_request("cat file.txt");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_pipe_chain_detected() {
        let req = make_request("cat file | grep foo | wc -l");
        let inspection = parse_command(&req);
        assert!(inspection.has_pipe);
    }

    #[test]
    fn test_redirection_detected() {
        let req = make_request("echo hello > file.txt");
        let inspection = parse_command(&req);
        assert!(!inspection.redirects.is_empty() || inspection.raw_command.contains('>'));
    }

    #[test]
    fn test_mkfs_critical() {
        let req = make_request("mkfs.ext4 /dev/sda1");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_rm_rf_specific_file_high() {
        let req = make_request("rm -rf /workspace/build");
        let inspection = parse_command(&req);
        assert_eq!(inspection.risk_level, RiskLevel::High);
    }
}
