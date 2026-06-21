//! Native local service verification tools.

use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use serde_json::{json, Value};
use tokio::io::{AsyncBufReadExt, AsyncRead, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use protocol_interface::{AliusError, RuntimeMode};

use crate::permission::PermissionLevel;
use crate::shell_gate::authorizer::{authorize, ShellGateConfig, ShellGateDecision};
use crate::shell_gate::inspector::command_args;
use crate::shell_gate::{ShellCommandRequest, ShellOrigin};
use crate::traits::{AliusTool, ToolContext, ToolResult};

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const MAX_TIMEOUT_SECS: u64 = 300;
const LOG_CAPACITY: usize = 200;
const LOG_TAIL: usize = 20;

static NEXT_SERVICE_ID: AtomicU64 = AtomicU64::new(1);
static SERVICE_MANAGER: std::sync::OnceLock<Mutex<ServiceManager>> = std::sync::OnceLock::new();

pub struct RunLocalService;
pub struct LocalServiceStatus;
pub struct StopLocalService;

struct ManagedService {
    child: Child,
    pid: Option<u32>,
    command: String,
    cwd: PathBuf,
    url: String,
    logs: Arc<Mutex<VecDeque<String>>>,
    started_at: Instant,
}

#[derive(Default)]
struct ServiceManager {
    services: HashMap<String, ManagedService>,
}

#[async_trait]
impl AliusTool for RunLocalService {
    fn name(&self) -> &'static str {
        "run_local_service"
    }

    fn description(&self) -> &'static str {
        "Start a workspace-local long-running service, wait until a loopback URL is reachable, and return the verified local URL. By default the service is stopped before returning."
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn preview_confirmation(&self, args: &Value, mode: RuntimeMode) -> bool {
        if mode != RuntimeMode::Plan {
            return false;
        }
        let command = args.get("command").and_then(Value::as_str).unwrap_or("");
        if command.trim().is_empty() {
            return false;
        }
        let req = ShellCommandRequest {
            command: command.to_string(),
            args: command_args(command),
            cwd: PathBuf::new(),
            origin: ShellOrigin::LocalCli,
            workspace_root: PathBuf::new(),
        };
        let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
        matches!(decision, ShellGateDecision::ApprovalRequired { .. })
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": { "type": "string", "description": "Command that starts the local service" },
                "cwd": { "type": "string", "description": "Workspace-relative working directory. Default: workspace root" },
                "expected_url": { "type": "string", "description": "Expected loopback URL to poll, for example http://127.0.0.1:3000" },
                "port": { "type": "integer", "description": "Loopback port to poll when expected_url is not provided" },
                "readiness_path": { "type": "string", "description": "Path used with port, default: /" },
                "timeout_secs": { "type": "integer", "description": "Readiness timeout. Default: 30; max: 300" },
                "keep_running": { "type": "boolean", "description": "Keep service running after verification. Default: false" }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, ctx: ToolContext) -> Result<ToolResult, AliusError> {
        match run_local_service(args, ctx).await {
            Ok(output) => Ok(ToolResult::success(output)),
            Err(message) => Ok(ToolResult::error(format!("error: {message}"))),
        }
    }
}

#[async_trait]
impl AliusTool for LocalServiceStatus {
    fn name(&self) -> &'static str {
        "local_service_status"
    }

    fn description(&self) -> &'static str {
        "Inspect a previously kept-running local service started by run_local_service."
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Read
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service_id": { "type": "string" }
            },
            "required": ["service_id"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let service_id = match args.get("service_id").and_then(Value::as_str) {
            Some(id) if !id.trim().is_empty() => id.trim(),
            _ => {
                return Ok(ToolResult::error(
                    "error: service_id is required".to_string(),
                ))
            }
        };

        match service_status(service_id).await {
            Ok(output) => Ok(ToolResult::success(output)),
            Err(message) => Ok(ToolResult::error(format!("error: {message}"))),
        }
    }
}

#[async_trait]
impl AliusTool for StopLocalService {
    fn name(&self) -> &'static str {
        "stop_local_service"
    }

    fn description(&self) -> &'static str {
        "Stop a local service that was kept running by run_local_service."
    }

    fn required_permission(&self) -> PermissionLevel {
        PermissionLevel::Execute
    }

    fn input_schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "service_id": { "type": "string" }
            },
            "required": ["service_id"]
        })
    }

    async fn execute(&self, args: Value, _ctx: ToolContext) -> Result<ToolResult, AliusError> {
        let service_id = match args.get("service_id").and_then(Value::as_str) {
            Some(id) if !id.trim().is_empty() => id.trim(),
            _ => {
                return Ok(ToolResult::error(
                    "error: service_id is required".to_string(),
                ))
            }
        };

        match stop_service(service_id).await {
            Ok(output) => Ok(ToolResult::success(output)),
            Err(message) => Ok(ToolResult::error(format!("error: {message}"))),
        }
    }
}

async fn run_local_service(args: Value, ctx: ToolContext) -> Result<String, String> {
    let command = args
        .get("command")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|command| !command.is_empty())
        .ok_or_else(|| "command is required".to_string())?
        .to_string();
    let cwd = resolve_cwd(args.get("cwd").and_then(Value::as_str), &ctx.workspace)?;
    authorize_service_command(&command, &cwd, &ctx.workspace)?;

    let timeout_secs = args
        .get("timeout_secs")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_SECS)
        .clamp(1, MAX_TIMEOUT_SECS);
    let keep_running = args
        .get("keep_running")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let readiness_path = args
        .get("readiness_path")
        .and_then(Value::as_str)
        .unwrap_or("/");
    let mut candidate_url = initial_readiness_url(&args, readiness_path)?;

    let mut cmd = build_command(&command);
    cmd.current_dir(&cwd);
    cmd.stdin(Stdio::null());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| format!("failed to spawn local service: {e}"))?;
    let pid = child.id();
    let logs = Arc::new(Mutex::new(VecDeque::new()));
    if let Some(stdout) = child.stdout.take() {
        spawn_log_reader(stdout, logs.clone(), "stdout");
    }
    if let Some(stderr) = child.stderr.take() {
        spawn_log_reader(stderr, logs.clone(), "stderr");
    }

    let client = reqwest::Client::new();
    let deadline = Instant::now() + Duration::from_secs(timeout_secs);
    let mut verified_url = None;

    while Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                let tail = logs_tail(&logs).await;
                return Err(format!(
                    "local service exited before readiness with status {status}; logs: {}",
                    tail.join("\n")
                ));
            }
            Ok(None) => {}
            Err(e) => return Err(format!("failed to inspect local service: {e}")),
        }

        if candidate_url.is_none() {
            let tail = logs_tail(&logs).await;
            candidate_url = extract_local_url(&tail.join("\n"));
        }

        if let Some(url) = candidate_url.as_deref() {
            if url_ready(&client, url).await {
                verified_url = Some(url.to_string());
                break;
            }
        }

        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    let url = match verified_url {
        Some(url) => normalize_display_url(&url),
        None => {
            let tail = logs_tail(&logs).await;
            let _ = child.kill().await;
            return Err(format!(
                "local service did not become ready within {timeout_secs}s; logs: {}",
                tail.join("\n")
            ));
        }
    };

    let service_id = if keep_running {
        let service_id = format!(
            "local-service-{}",
            NEXT_SERVICE_ID.fetch_add(1, Ordering::Relaxed)
        );
        let service = ManagedService {
            child,
            pid,
            command: command.clone(),
            cwd: cwd.clone(),
            url: url.clone(),
            logs: logs.clone(),
            started_at: Instant::now(),
        };
        manager()
            .lock()
            .await
            .services
            .insert(service_id.clone(), service);
        Some(service_id)
    } else {
        child
            .kill()
            .await
            .map_err(|e| format!("local service became ready at {url}, but stop failed: {e}"))?;
        None
    };

    let tail = logs_tail(&logs).await;
    Ok(json!({
        "tool": "run_local_service",
        "ready": true,
        "url": url,
        "service_id": service_id,
        "pid": pid,
        "kept_running": keep_running,
        "stopped": !keep_running,
        "command": command,
        "cwd": display_path(&cwd, &ctx.workspace),
        "logs_tail": tail,
    })
    .to_string())
}

fn resolve_cwd(cwd: Option<&str>, workspace: &Path) -> Result<PathBuf, String> {
    let workspace = workspace
        .canonicalize()
        .map_err(|e| format!("workspace not accessible: {e}"))?;
    let candidate = match cwd {
        Some(value) if !value.trim().is_empty() => {
            let path = Path::new(value.trim());
            if path.is_absolute() {
                return Err("cwd must be relative to workspace".to_string());
            }
            workspace.join(path)
        }
        _ => workspace.clone(),
    };
    let canonical = candidate
        .canonicalize()
        .map_err(|e| format!("invalid cwd: {e}"))?;
    if !canonical.starts_with(&workspace) {
        return Err("cwd must be inside workspace".to_string());
    }
    Ok(canonical)
}

fn authorize_service_command(command: &str, cwd: &Path, workspace: &Path) -> Result<(), String> {
    let req = ShellCommandRequest {
        command: command.to_string(),
        args: command_args(command),
        cwd: cwd.to_path_buf(),
        origin: ShellOrigin::LocalCli,
        workspace_root: workspace.to_path_buf(),
    };
    let (decision, _risk) = authorize(&req, &ShellGateConfig::default());
    match decision {
        ShellGateDecision::Deny { reason } => Err(format!("denied by Shell Gate: {reason}")),
        ShellGateDecision::Allow | ShellGateDecision::ApprovalRequired { .. } => Ok(()),
    }
}

fn initial_readiness_url(args: &Value, readiness_path: &str) -> Result<Option<String>, String> {
    if let Some(url) = args
        .get("expected_url")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|url| !url.is_empty())
    {
        if !is_local_http_url(url) {
            return Err("expected_url must be a localhost or loopback HTTP(S) URL".to_string());
        }
        return Ok(Some(normalize_display_url(url)));
    }

    if let Some(port) = args.get("port").and_then(Value::as_u64) {
        if port == 0 || port > u16::MAX as u64 {
            return Err("port must be between 1 and 65535".to_string());
        }
        return Ok(Some(format!(
            "http://127.0.0.1:{}{}",
            port,
            normalize_path(readiness_path)
        )));
    }

    Ok(None)
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        "/".to_string()
    } else if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    }
}

async fn url_ready(client: &reqwest::Client, url: &str) -> bool {
    client.get(url).send().await.is_ok()
}

fn spawn_log_reader<R>(reader: R, logs: Arc<Mutex<VecDeque<String>>>, stream: &'static str)
where
    R: AsyncRead + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            push_log(&logs, format!("{stream}: {}", redact_secrets(&line))).await;
        }
    });
}

async fn push_log(logs: &Arc<Mutex<VecDeque<String>>>, line: String) {
    let mut logs = logs.lock().await;
    if logs.len() >= LOG_CAPACITY {
        logs.pop_front();
    }
    logs.push_back(line);
}

async fn logs_tail(logs: &Arc<Mutex<VecDeque<String>>>) -> Vec<String> {
    let logs = logs.lock().await;
    logs.iter()
        .skip(logs.len().saturating_sub(LOG_TAIL))
        .cloned()
        .collect()
}

fn extract_local_url(text: &str) -> Option<String> {
    for raw in text.split_whitespace() {
        let token = raw.trim_matches(|c: char| {
            matches!(
                c,
                '"' | '\'' | '(' | ')' | '[' | ']' | '<' | '>' | ',' | ';'
            )
        });
        if (token.starts_with("http://") || token.starts_with("https://"))
            && is_local_http_url(token)
        {
            return Some(normalize_display_url(token));
        }
    }
    None
}

fn is_local_http_url(url: &str) -> bool {
    let rest = if let Some(rest) = url.strip_prefix("http://") {
        rest
    } else if let Some(rest) = url.strip_prefix("https://") {
        rest
    } else {
        return false;
    };
    let host_port = rest.split(['/', '?', '#']).next().unwrap_or("");
    let host = if host_port.starts_with('[') {
        host_port
            .split(']')
            .next()
            .unwrap_or("")
            .trim_start_matches('[')
    } else {
        host_port.split(':').next().unwrap_or("")
    };
    matches!(host, "localhost" | "127.0.0.1" | "0.0.0.0" | "::1")
}

fn normalize_display_url(url: &str) -> String {
    url.trim()
        .trim_end_matches(['.', ','])
        .replacen("http://0.0.0.0", "http://127.0.0.1", 1)
        .replacen("https://0.0.0.0", "https://127.0.0.1", 1)
}

fn redact_secrets(input: &str) -> String {
    let mut output = input.to_string();
    for marker in [
        "api_key=",
        "api-key=",
        "apikey=",
        "token=",
        "access_token=",
        "authorization: bearer ",
        "Authorization: Bearer ",
    ] {
        output = redact_after_marker(&output, marker);
    }
    output
}

fn redact_after_marker(input: &str, marker: &str) -> String {
    let mut result = String::new();
    let mut rest = input;
    while let Some(index) = rest.find(marker) {
        let (before, after_before) = rest.split_at(index);
        result.push_str(before);
        result.push_str(marker);
        result.push_str("[redacted]");
        let secret_start = marker.len();
        let after_marker = &after_before[secret_start..];
        let secret_end = after_marker
            .find(|c: char| c.is_whitespace() || matches!(c, '&' | '"' | '\'' | ';'))
            .unwrap_or(after_marker.len());
        rest = &after_marker[secret_end..];
    }
    result.push_str(rest);
    result
}

async fn service_status(service_id: &str) -> Result<String, String> {
    let mut services = manager().lock().await;
    let Some(service) = services.services.get_mut(service_id) else {
        return Err(format!("unknown local service '{service_id}'"));
    };
    let exited = service
        .child
        .try_wait()
        .map_err(|e| format!("failed to inspect local service: {e}"))?;
    let running = exited.is_none();
    let output = json!({
        "tool": "local_service_status",
        "service_id": service_id,
        "running": running,
        "exit_status": exited.map(|status| status.to_string()),
        "url": service.url,
        "pid": service.pid,
        "command": service.command,
        "cwd": service.cwd.to_string_lossy(),
        "uptime_secs": service.started_at.elapsed().as_secs(),
        "logs_tail": logs_tail(&service.logs).await,
    })
    .to_string();
    if !running {
        services.services.remove(service_id);
    }
    Ok(output)
}

async fn stop_service(service_id: &str) -> Result<String, String> {
    let service = {
        let mut services = manager().lock().await;
        services
            .services
            .remove(service_id)
            .ok_or_else(|| format!("unknown local service '{service_id}'"))?
    };
    let ManagedService {
        mut child,
        pid,
        command,
        cwd,
        url,
        logs,
        ..
    } = service;
    let kill_result = child.kill().await;
    let logs_tail = logs_tail(&logs).await;
    match kill_result {
        Ok(()) => Ok(json!({
            "tool": "stop_local_service",
            "service_id": service_id,
            "stopped": true,
            "url": url,
            "pid": pid,
            "command": command,
            "cwd": cwd.to_string_lossy(),
            "logs_tail": logs_tail,
        })
        .to_string()),
        Err(e) => Err(format!("failed to stop local service '{service_id}': {e}")),
    }
}

fn manager() -> &'static Mutex<ServiceManager> {
    SERVICE_MANAGER.get_or_init(|| Mutex::new(ServiceManager::default()))
}

fn display_path(path: &Path, workspace: &Path) -> String {
    let workspace = workspace
        .canonicalize()
        .unwrap_or_else(|_| workspace.to_path_buf());
    path.strip_prefix(&workspace)
        .unwrap_or(path)
        .to_string_lossy()
        .trim_start_matches(std::path::MAIN_SEPARATOR)
        .to_string()
}

#[cfg(unix)]
fn build_command(command: &str) -> Command {
    let mut c = Command::new("sh");
    c.arg("-c").arg(command);
    c
}

#[cfg(windows)]
fn build_command(command: &str) -> Command {
    let mut c = Command::new("cmd");
    c.arg("/C").arg(command);
    c
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use tempfile::TempDir;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    fn ctx(workspace: &Path) -> ToolContext {
        ToolContext::new(
            workspace.to_path_buf(),
            "test-session".to_string(),
            protocol_interface::RuntimeMode::Chat,
        )
    }

    #[test]
    fn extracts_common_local_service_urls() {
        assert_eq!(
            extract_local_url("VITE ready at http://localhost:5173/").as_deref(),
            Some("http://localhost:5173/")
        );
        assert_eq!(
            extract_local_url("ready - started server on 0.0.0.0:3000, url: http://0.0.0.0:3000")
                .as_deref(),
            Some("http://127.0.0.1:3000")
        );
        assert_eq!(
            extract_local_url("Uvicorn running on http://127.0.0.1:8000").as_deref(),
            Some("http://127.0.0.1:8000")
        );
        assert_eq!(
            extract_local_url("Rocket has launched from http://localhost:8000").as_deref(),
            Some("http://localhost:8000")
        );
    }

    #[test]
    fn rejects_non_loopback_readiness_url() {
        assert!(!is_local_http_url("https://example.com"));
        assert!(is_local_http_url("http://127.0.0.1:3000"));
        assert!(is_local_http_url("http://[::1]:3000"));
    }

    #[test]
    fn redacts_basic_secrets_from_logs() {
        let line = "token=abc123 api_key=secret Authorization: Bearer real";
        let redacted = redact_secrets(line);
        assert!(!redacted.contains("abc123"));
        assert!(!redacted.contains("secret"));
        assert!(!redacted.contains("real"));
        assert!(redacted.contains("[redacted]"));
    }

    async fn start_test_http_server() -> Option<(tokio::task::JoinHandle<()>, String)> {
        let listener = match tokio::net::TcpListener::bind("127.0.0.1:0").await {
            Ok(listener) => listener,
            Err(_) => return None,
        };
        let url = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            while let Ok((mut socket, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let mut buf = [0u8; 1024];
                    let _ = socket.read(&mut buf).await;
                    let _ = socket
                        .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK")
                        .await;
                });
            }
        });
        Some((handle, url))
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_local_service_stops_by_default_after_readiness() {
        let tmp = TempDir::new().unwrap();
        let Some((_server, url)) = start_test_http_server().await else {
            return;
        };
        let result = RunLocalService
            .execute(
                json!({
                    "command": "sleep 30",
                    "expected_url": url,
                    "timeout_secs": 3
                }),
                ctx(tmp.path()),
            )
            .await
            .unwrap();

        assert!(result.success, "{}", result.output);
        let output: Value = serde_json::from_str(&result.output).unwrap();
        assert_eq!(output["ready"], true);
        assert_eq!(output["kept_running"], false);
        assert_eq!(output["stopped"], true);
        assert!(output["url"]
            .as_str()
            .unwrap()
            .starts_with("http://127.0.0.1:"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_local_service_keep_running_can_status_and_stop() {
        let tmp = TempDir::new().unwrap();
        let Some((_server, url)) = start_test_http_server().await else {
            return;
        };
        let result = RunLocalService
            .execute(
                json!({
                    "command": "sleep 30",
                    "expected_url": url,
                    "timeout_secs": 3,
                    "keep_running": true
                }),
                ctx(tmp.path()),
            )
            .await
            .unwrap();
        assert!(result.success, "{}", result.output);
        let output: Value = serde_json::from_str(&result.output).unwrap();
        let service_id = output["service_id"].as_str().unwrap();

        let status = LocalServiceStatus
            .execute(json!({ "service_id": service_id }), ctx(tmp.path()))
            .await
            .unwrap();
        assert!(status.success, "{}", status.output);
        assert_eq!(
            serde_json::from_str::<Value>(&status.output).unwrap()["running"],
            true
        );

        let stopped = StopLocalService
            .execute(json!({ "service_id": service_id }), ctx(tmp.path()))
            .await
            .unwrap();
        assert!(stopped.success, "{}", stopped.output);
        assert_eq!(
            serde_json::from_str::<Value>(&stopped.output).unwrap()["stopped"],
            true
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn run_local_service_timeout_kills_child() {
        let tmp = TempDir::new().unwrap();
        let result = RunLocalService
            .execute(
                json!({
                    "command": "sleep 30",
                    "expected_url": "http://127.0.0.1:1",
                    "timeout_secs": 1
                }),
                ctx(tmp.path()),
            )
            .await
            .unwrap();

        assert!(!result.success);
        assert!(result.output.contains("did not become ready"));
    }
}
