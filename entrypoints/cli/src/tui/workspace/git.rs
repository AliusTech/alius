use std::process::Command;

use rust_i18n::t;

use super::helpers::compact_path;
use crate::tui::state::WorkspaceStatus;

impl WorkspaceStatus {
    pub fn load() -> Self {
        let cwd = std::env::current_dir()
            .map(|path| compact_path(&path.display().to_string()))
            .unwrap_or_else(|_| "~".to_string());
        let git_available = git_is_available();

        let root = git_output(&["rev-parse", "--show-toplevel"]);
        let Some(root) = root else {
            return Self {
                cwd,
                repo: None,
                branch: None,
                staged: 0,
                modified: 0,
                untracked: 0,
                clean: false,
                git_available,
            };
        };

        let repo = std::path::Path::new(root.trim())
            .file_name()
            .map(|name| name.to_string_lossy().to_string());
        let branch = git_output(&["branch", "--show-current"])
            .map(|branch| {
                let branch = branch.trim();
                if branch.is_empty() {
                    t!("workspace.status_bar.detached").to_string()
                } else {
                    branch.to_string()
                }
            })
            .or_else(|| git_output(&["rev-parse", "--short", "HEAD"]).map(|s| s.trim().into()));

        let status = git_output(&["status", "--porcelain=v1"]);
        let Some(status) = status else {
            return Self {
                cwd,
                repo,
                branch,
                staged: 0,
                modified: 0,
                untracked: 0,
                clean: false,
                git_available: false,
            };
        };

        let (staged, modified, untracked) = parse_git_status(&status);
        Self {
            cwd,
            repo,
            branch,
            staged,
            modified,
            untracked,
            clean: staged == 0 && modified == 0 && untracked == 0,
            git_available: true,
        }
    }

    pub fn display(&self, max_width: usize) -> String {
        use super::helpers::truncate_chars;

        let repo = self.repo.as_deref().unwrap_or("none");
        let branch = self.branch.as_deref().unwrap_or("-");
        let git_status = if !self.git_available && self.repo.is_some() {
            t!("workspace.status_bar.git_unavailable").to_string()
        } else if self.repo.is_none() {
            String::new()
        } else if self.clean {
            t!("workspace.status_bar.clean").to_string()
        } else {
            let mut parts = Vec::new();
            if self.staged > 0 {
                parts.push(format!("+{}", self.staged));
            }
            if self.modified > 0 {
                parts.push(format!("~{}", self.modified));
            }
            if self.untracked > 0 {
                parts.push(format!("?{}", self.untracked));
            }
            parts.join(" ")
        };

        let mut text = t!(
            "workspace.status_bar.cwd",
            cwd = &self.cwd,
            repo = repo,
            branch = branch
        )
        .to_string();
        if !git_status.is_empty() {
            text.push_str(" │ ");
            text.push_str(&git_status);
        }
        truncate_chars(&text, max_width)
    }
}

fn git_output(args: &[&str]) -> Option<String> {
    let output = Command::new("git").args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).to_string())
}

fn git_is_available() -> bool {
    Command::new("git")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false)
}

fn parse_git_status(status: &str) -> (u32, u32, u32) {
    let mut staged = 0;
    let mut modified = 0;
    let mut untracked = 0;

    for line in status.lines() {
        let mut chars = line.chars();
        let x = chars.next().unwrap_or(' ');
        let y = chars.next().unwrap_or(' ');

        if x == '?' && y == '?' {
            untracked += 1;
            continue;
        }
        if x != ' ' {
            staged += 1;
        }
        if y != ' ' {
            modified += 1;
        }
    }

    (staged, modified, untracked)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_status_counts_porcelain_columns() {
        let status = "M  staged.rs\n M modified.rs\nMM both.rs\n?? new.rs\n";

        assert_eq!(parse_git_status(status), (2, 2, 1));
    }
}
