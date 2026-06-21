//! Native `search_code` tool — workspace-bound code search via rg/grep.

use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;

use async_trait::async_trait;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::process::Command;

use protocol_interface::AliusError;

use crate::permission::PermissionLevel;
use crate::traits::{AliusTool, ToolContext, ToolResult};

const DEFAULT_MAX_RESULTS: usize = 100;
const HARD_MAX_RESULTS: usize = 500;
const SEARCH_TIMEOUT_SECS: u64 = 30;

pub struct SearchCode;

#[derive(Debug, Serialize)]
struct SearchOutput {
    matches: Vec<SearchMatch>,
    truncated: bool,
    backend: &'static str,
}

#[derive(Debug, Serialize)]
struct SearchMatch {
    file: String,
    line: u64,
    column: u64,
    text: String,
    truncated: bool,
}

#[async_trait]
impl AliusTool for SearchCode {
    fn name(&self) -> &'static str {
        "search_code"
    }

    fn description(&self) -> &'static str {
        "Search source code inside the workspace using rg when available, falling back to grep."
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": { "type": "string", "description": "Search pattern" },
                "path": { "type": "string", "description": "Workspace-relative path to search. Default: ." },
                "glob": {
                    "oneOf": [
                        { "type": "string" },
                        { "type": "array", "items": { "type": "string" } }
                    ],
                    "description": "Optional include glob(s), for example '*.rs' or ['*.rs','*.toml']"
                },
                "context": { "type": "integer", "description": "Context lines for rg/grep. Default: 0" },
                "case_sensitive": { "type": "boolean", "description": "Case-sensitive search. Default: true" },
                "max_results": { "type": "integer", "description": "Maximum matches to return. Default: 100; max: 500" }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let query = match args.get("query").and_then(Value::as_str) {
            Some(q) if !q.trim().is_empty() => q.trim().to_string(),
            _ => return Ok(ToolResult::error("error: query is required".to_string())),
        };
        let search_path = args.get("path").and_then(Value::as_str).unwrap_or(".");
        let root = match resolve_existing_path(search_path, &ctx.workspace) {
            Ok(path) => path,
            Err(message) => return Ok(ToolResult::error(format!("error: {message}"))),
        };
        let workspace = match ctx.workspace.canonicalize() {
            Ok(path) => path,
            Err(e) => {
                return Ok(ToolResult::error(format!(
                    "error: workspace not accessible: {e}"
                )))
            }
        };
        let globs = parse_globs(args.get("glob"));
        let context = args
            .get("context")
            .and_then(Value::as_u64)
            .unwrap_or(0)
            .min(20);
        let case_sensitive = args
            .get("case_sensitive")
            .and_then(Value::as_bool)
            .unwrap_or(true);
        let max_results = args
            .get("max_results")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_MAX_RESULTS as u64)
            .max(1)
            .min(HARD_MAX_RESULTS as u64) as usize;

        let result = if command_available("rg").await {
            run_rg(
                &query,
                &root,
                &workspace,
                &globs,
                context,
                case_sensitive,
                max_results,
            )
            .await
        } else {
            run_grep(
                &query,
                &root,
                &workspace,
                &globs,
                context,
                case_sensitive,
                max_results,
            )
            .await
        };

        match result {
            Ok(output) => Ok(ToolResult::success(
                serde_json::to_string_pretty(&output).unwrap_or_else(|_| "{}".to_string()),
            )),
            Err(message) => Ok(ToolResult::error(format!("error: {message}"))),
        }
    }
}

fn resolve_existing_path(path: &str, workspace: &Path) -> Result<PathBuf, String> {
    let requested = Path::new(path);
    if requested.is_absolute() {
        return Err("absolute paths are not allowed; use a workspace-relative path".to_string());
    }
    let workspace = workspace
        .canonicalize()
        .map_err(|e| format!("workspace not accessible: {e}"))?;
    let candidate = workspace.join(requested);
    let canonical = candidate
        .canonicalize()
        .map_err(|e| format!("cannot access '{path}': {e}"))?;
    if !canonical.starts_with(&workspace) {
        return Err(format!("path '{path}' resolves outside workspace"));
    }
    Ok(canonical)
}

fn parse_globs(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) if !s.trim().is_empty() => vec![s.trim().to_string()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToString::to_string)
            .collect(),
        _ => Vec::new(),
    }
}

async fn command_available(command: &str) -> bool {
    let status = Command::new(command)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    matches!(
        tokio::time::timeout(Duration::from_secs(2), status).await,
        Ok(Ok(status)) if status.success()
    )
}

async fn run_rg(
    query: &str,
    root: &Path,
    workspace: &Path,
    globs: &[String],
    context: u64,
    case_sensitive: bool,
    max_results: usize,
) -> Result<SearchOutput, String> {
    let mut cmd = Command::new("rg");
    cmd.arg("--json")
        .arg("--color")
        .arg("never")
        .arg("--line-number")
        .arg("--column");
    if context > 0 {
        cmd.arg("-C").arg(context.to_string());
    }
    if !case_sensitive {
        cmd.arg("--ignore-case");
    }
    for glob in globs {
        cmd.arg("--glob").arg(glob);
    }
    cmd.arg("--").arg(query).arg(root);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = wait_for_output(cmd).await?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(format!(
            "rg failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut matches = Vec::new();
    let mut truncated = false;
    for line in stdout.lines() {
        if let Some(search_match) = parse_rg_json_line(line, workspace) {
            if matches.len() >= max_results {
                truncated = true;
                break;
            }
            matches.push(search_match);
        }
    }

    Ok(SearchOutput {
        matches,
        truncated,
        backend: "rg",
    })
}

fn parse_rg_json_line(line: &str, workspace: &Path) -> Option<SearchMatch> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("type").and_then(Value::as_str)? != "match" {
        return None;
    }
    let data = value.get("data")?;
    let path = data.get("path")?.get("text")?.as_str().map(PathBuf::from)?;
    let file = relative_display_path(&path, workspace);
    let line_number = data.get("line_number")?.as_u64()?;
    let text = data
        .get("lines")?
        .get("text")?
        .as_str()
        .unwrap_or("")
        .trim_end_matches(['\r', '\n'])
        .to_string();
    let column = data
        .get("submatches")
        .and_then(Value::as_array)
        .and_then(|items| items.first())
        .and_then(|item| item.get("start"))
        .and_then(Value::as_u64)
        .map(|zero_based| zero_based + 1)
        .unwrap_or(1);

    Some(SearchMatch {
        file,
        line: line_number,
        column,
        text,
        truncated: false,
    })
}

async fn run_grep(
    query: &str,
    root: &Path,
    workspace: &Path,
    globs: &[String],
    context: u64,
    case_sensitive: bool,
    max_results: usize,
) -> Result<SearchOutput, String> {
    let mut cmd = Command::new("grep");
    cmd.arg("-R")
        .arg("-I")
        .arg("-n")
        .arg("--binary-files=without-match");
    if context > 0 {
        cmd.arg("-C").arg(context.to_string());
    }
    if !case_sensitive {
        cmd.arg("-i");
    }
    for glob in globs {
        cmd.arg(format!("--include={glob}"));
    }
    cmd.arg("--").arg(query).arg(root);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let output = wait_for_output(cmd).await?;
    if !output.status.success() && output.status.code() != Some(1) {
        return Err(format!(
            "grep failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut matches = Vec::new();
    let mut truncated = false;
    for line in stdout.lines() {
        if let Some(search_match) = parse_grep_line(line, workspace) {
            if matches.len() >= max_results {
                truncated = true;
                break;
            }
            matches.push(search_match);
        }
    }

    Ok(SearchOutput {
        matches,
        truncated,
        backend: "grep",
    })
}

async fn wait_for_output(mut cmd: Command) -> Result<std::process::Output, String> {
    let child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn search backend: {e}"))?;
    match tokio::time::timeout(
        Duration::from_secs(SEARCH_TIMEOUT_SECS),
        child.wait_with_output(),
    )
    .await
    {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(format!("search backend wait failed: {e}")),
        Err(_) => Err(format!("search timed out after {SEARCH_TIMEOUT_SECS}s")),
    }
}

fn parse_grep_line(line: &str, workspace: &Path) -> Option<SearchMatch> {
    let (file, rest) = split_grep_field(line)?;
    let (line_no, text) = split_grep_field(rest)?;
    let line = line_no.parse().ok()?;
    Some(SearchMatch {
        file: relative_display_path(Path::new(file), workspace),
        line,
        column: 1,
        text: text.to_string(),
        truncated: false,
    })
}

fn split_grep_field(value: &str) -> Option<(&str, &str)> {
    value
        .split_once(':')
        .or_else(|| value.split_once('-'))
        .filter(|(left, _)| !left.is_empty())
}

fn relative_display_path(path: &Path, workspace: &Path) -> String {
    let candidate = if path.is_absolute() {
        path.to_path_buf()
    } else {
        workspace.join(path)
    };
    candidate
        .strip_prefix(workspace)
        .unwrap_or(&candidate)
        .to_string_lossy()
        .trim_start_matches(std::path::MAIN_SEPARATOR)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;

    fn ctx(workspace: &Path) -> ToolContext {
        ToolContext::new(
            workspace.to_path_buf(),
            "test-session".to_string(),
            protocol_interface::RuntimeMode::Chat,
        )
    }

    #[tokio::test]
    async fn search_code_finds_workspace_match() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join("main.rs"),
            "fn main() {\n    println!(\"alius\");\n}\n",
        )
        .unwrap();

        let result = SearchCode
            .execute(json!({"query": "alius", "path": "."}), ctx(tmp.path()))
            .await
            .unwrap();

        assert!(result.success);
        let output: Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output["matches"][0]["file"], "main.rs");
        assert_eq!(output["matches"][0]["line"], 2);
    }

    #[tokio::test]
    async fn search_code_rejects_absolute_path() {
        let tmp = TempDir::new().unwrap();
        let result = SearchCode
            .execute(
                json!({"query": "root", "path": "/etc/passwd"}),
                ctx(tmp.path()),
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.output.contains("absolute paths are not allowed"));
    }

    #[tokio::test]
    async fn search_code_rejects_parent_escape() {
        let tmp = TempDir::new().unwrap();
        let result = SearchCode
            .execute(json!({"query": "x", "path": ".."}), ctx(tmp.path()))
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.output.contains("resolves outside workspace"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn search_code_rejects_symlink_escape() {
        let workspace = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let link = workspace.path().join("outside-link");
        std::os::unix::fs::symlink(outside.path(), &link).unwrap();

        let result = SearchCode
            .execute(
                json!({"query": "x", "path": "outside-link"}),
                ctx(workspace.path()),
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.output.contains("resolves outside workspace"));
    }

    #[tokio::test]
    async fn search_code_truncates_results() {
        let tmp = TempDir::new().unwrap();
        std::fs::write(tmp.path().join("a.txt"), "needle\nneedle\nneedle\n").unwrap();

        let result = SearchCode
            .execute(
                json!({"query": "needle", "path": ".", "max_results": 2}),
                ctx(tmp.path()),
            )
            .await
            .unwrap();
        let output: Value = serde_json::from_str(&result.output).unwrap();

        assert_eq!(output["matches"].as_array().unwrap().len(), 2);
        assert_eq!(output["truncated"], true);
    }
}
