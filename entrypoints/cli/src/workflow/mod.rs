//! Alius Workflow — Workflow parser and executor.
//!
//! Parses JSON workflow definitions and executes multi-step pipelines
//! with variable interpolation between steps.
//!
//! ## Runtime Integration
//!
//! Prompt steps call the LLM via [`LoopEngineHandle::run_prompt`].
//! Tool steps execute through [`LoopEngineHandle::run_tool`], which
//! routes through the `ToolRegistry` (including confirmation and audit).
//!
//! The production handle is [`RuntimeWorkflowHandle`], which delegates to
//! `CoreRuntimeManager` (LLM) and `ToolRegistry` (tools). A stub handle
//! exists for tests only.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use core_runtime::CoreRuntimeManager;
use protocol_interface::core::{CoreEventPayload, RuntimeMode};
use runtime_tools::{ToolContext, ToolRegistry};

/// Abstraction for runtime capabilities needed by workflow steps.
///
/// Implementors bridge workflow steps to the Core Runtime (LLM calls, tool execution).
/// This allows the workflow module to remain independent of `runtime/core` while
/// still using real runtime paths when available.
#[async_trait]
pub trait LoopEngineHandle: Send + Sync {
    /// Run a prompt through the LLM and return the text response.
    async fn run_prompt(&self, text: &str, mode: &str) -> Result<String>;

    /// Execute a tool by name with JSON arguments and execution mode.
    /// Returns the tool's JSON output.
    /// The `mode` parameter controls confirmation behavior ("chat" or "plan").
    async fn run_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
        mode: &str,
    ) -> Result<serde_json::Value>;
}

/// Production handle that delegates to `CoreRuntimeManager` and `ToolRegistry`.
///
/// - `run_prompt` enters the real LoopEngine via `CoreRuntimeManager::run_text()`,
///   which calls the LLM provider and runs the full event stream.
/// - `run_tool` looks up the tool in the shared `ToolRegistry` and executes it
///   through the real WASM/native/MCP execution path.
pub struct RuntimeWorkflowHandle {
    manager: CoreRuntimeManager,
    registry: Arc<ToolRegistry>,
}

impl RuntimeWorkflowHandle {
    pub fn new(manager: CoreRuntimeManager, registry: Arc<ToolRegistry>) -> Self {
        Self { manager, registry }
    }
}

#[async_trait]
impl LoopEngineHandle for RuntimeWorkflowHandle {
    async fn run_prompt(&self, text: &str, mode: &str) -> Result<String> {
        let rt_mode = match mode.to_lowercase().as_str() {
            "plan" => RuntimeMode::Plan,
            _ => RuntimeMode::Chat,
        };
        let envelopes = self
            .manager
            .run_text(text, rt_mode)
            .map_err(|e| anyhow::anyhow!("Runtime error: {}", e))?;

        // Extract the final result from the event stream.
        let mut final_content = String::new();
        let mut had_error = false;
        for envelope in &envelopes {
            match &envelope.payload.payload {
                CoreEventPayload::Final { content, success } => {
                    final_content = content.clone();
                    if !success {
                        had_error = true;
                    }
                }
                CoreEventPayload::Error { message, .. } if !message.is_empty() => {
                    final_content = message.clone();
                    had_error = true;
                }
                _ => {}
            }
        }

        if had_error && !final_content.is_empty() {
            Err(anyhow::anyhow!("{}", final_content))
        } else if final_content.is_empty() {
            Ok("(no response)".to_string())
        } else {
            Ok(final_content)
        }
    }

    async fn run_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
        mode: &str,
    ) -> Result<serde_json::Value> {
        let tool = self
            .registry
            .get(tool_name)
            .ok_or_else(|| anyhow::anyhow!("Tool not found: {}", tool_name))?;

        let workspace = self.manager.workspace_root();
        let rt_mode = match mode.to_lowercase().as_str() {
            "plan" => RuntimeMode::Plan,
            _ => RuntimeMode::Chat,
        };
        let ctx = ToolContext::new(workspace.to_path_buf(), "workflow".to_string(), rt_mode);

        let result = tool
            .execute(args.clone(), ctx)
            .await
            .map_err(|e| anyhow::anyhow!("Tool '{}' failed: {}", tool_name, e))?;

        Ok(serde_json::json!({
            "tool": tool_name,
            "args": args,
            "output": result.output,
            "success": result.success,
        }))
    }
}

/// A stub handle that returns formatted strings (for testing only).
#[allow(dead_code)]
pub struct StubLoopEngineHandle;

#[async_trait]
impl LoopEngineHandle for StubLoopEngineHandle {
    async fn run_prompt(&self, text: &str, _mode: &str) -> Result<String> {
        Ok(format!("[prompt] {}", text))
    }

    async fn run_tool(
        &self,
        tool_name: &str,
        args: serde_json::Value,
        _mode: &str,
    ) -> Result<serde_json::Value> {
        Ok(serde_json::json!({
            "tool": tool_name,
            "args": args,
            "output": format!("[tool:{}]", tool_name)
        }))
    }
}

fn user_agent() -> String {
    format!("alius-cli/{}", env!("ALIUS_VERSION"))
}

/// Failure handling policy for a workflow step.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OnFailurePolicy {
    /// Abort the workflow immediately (default).
    #[default]
    Abort,
    /// Skip this step and continue to the next.
    Skip,
    /// Retry the step up to `max_retries` times with exponential backoff.
    #[serde(rename = "retry")]
    Retry {
        max_retries: u32,
        #[serde(default = "default_backoff_ms")]
        backoff_ms: u64,
    },
}

fn default_backoff_ms() -> u64 {
    1000
}

/// A workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<Step>,
    /// Workflow execution mode: "chat" or "plan". Default: "chat".
    #[serde(default = "default_mode")]
    pub mode: String,
    /// Overall workflow timeout in milliseconds. None = no timeout.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

fn default_mode() -> String {
    "chat".to_string()
}

/// A single step in a workflow.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Step {
    pub id: String,
    #[serde(rename = "type")]
    pub step_type: StepType,
    /// For "prompt" type: the prompt text (supports {{var}} interpolation).
    #[serde(default)]
    pub prompt: Option<String>,
    /// For "tool" type: tool name to call.
    #[serde(default)]
    pub tool: Option<String>,
    /// For "tool" type: tool arguments.
    #[serde(default)]
    pub args: Option<serde_json::Value>,
    /// For "http" type: URL to call.
    #[serde(default)]
    pub url: Option<String>,
    /// For "http" type: HTTP method (default POST).
    #[serde(default)]
    pub method: Option<String>,
    /// For "http" type: request body (supports {{var}} interpolation).
    #[serde(default)]
    pub body: Option<serde_json::Value>,
    /// What to do when this step fails. Default: Abort.
    #[serde(default)]
    pub on_failure: OnFailurePolicy,
    /// Per-step timeout in milliseconds. None = no timeout.
    #[serde(default)]
    pub timeout_ms: Option<u64>,
}

/// Step type enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    /// Call an LLM with a prompt.
    Prompt,
    /// Call a Rust WASM module tool.
    Tool,
    /// Send an HTTP request.
    Http,
    /// Conditional branch.
    Condition,
}

/// Result of executing a single step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StepResult {
    pub step_id: String,
    pub output: String,
    pub success: bool,
    /// When the step started executing.
    #[serde(default)]
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    /// When the step finished executing.
    #[serde(default)]
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Execution duration in milliseconds.
    #[serde(default)]
    pub duration_ms: Option<u64>,
    /// Trace ID from the runtime event stream (for prompt steps).
    #[serde(default)]
    pub trace_id: Option<String>,
    /// Run reference from the runtime event stream (for prompt steps).
    #[serde(default)]
    pub run_ref: Option<String>,
}

/// Workflow execution status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum WorkflowRunStatus {
    Completed,
    Failed,
    Cancelled,
}

/// Persistent record of a workflow execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunRecord {
    pub workflow_name: String,
    pub status: WorkflowRunStatus,
    pub started_at: chrono::DateTime<chrono::Utc>,
    pub finished_at: chrono::DateTime<chrono::Utc>,
    pub duration_ms: u64,
    pub steps: Vec<StepResult>,
}

/// Execution context holding results from previous steps.
#[derive(Default)]
pub struct ExecutionContext {
    pub results: HashMap<String, StepResult>,
}

impl ExecutionContext {
    pub fn new() -> Self {
        Self::default()
    }

    /// Interpolate {{step_id.output}} variables in a string.
    pub fn interpolate(&self, template: &str) -> String {
        let mut result = template.to_string();
        for (id, step_result) in &self.results {
            let pattern = format!("{{{{{}.output}}}}", id);
            result = result.replace(&pattern, &step_result.output);
        }
        result
    }

    /// Interpolate variables in a JSON value.
    pub fn interpolate_json(&self, value: &serde_json::Value) -> serde_json::Value {
        match value {
            serde_json::Value::String(s) => serde_json::Value::String(self.interpolate(s)),
            serde_json::Value::Object(map) => {
                let mut new_map = serde_json::Map::new();
                for (k, v) in map {
                    new_map.insert(k.clone(), self.interpolate_json(v));
                }
                serde_json::Value::Object(new_map)
            }
            serde_json::Value::Array(arr) => {
                serde_json::Value::Array(arr.iter().map(|v| self.interpolate_json(v)).collect())
            }
            other => other.clone(),
        }
    }
}

/// Load a workflow from a JSON file.
pub fn load_workflow(path: &Path) -> Result<Workflow> {
    let content = std::fs::read_to_string(path)?;
    let workflow: Workflow = serde_json::from_str(&content)?;
    Ok(workflow)
}

/// Load all workflows from a directory.
pub fn load_workflows(dir: &Path) -> Result<Vec<Workflow>> {
    if !dir.exists() {
        return Ok(vec![]);
    }
    let mut workflows = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            match load_workflow(&path) {
                Ok(wf) => workflows.push(wf),
                Err(e) => eprintln!("Warning: failed to load {}: {}", path.display(), e),
            }
        }
    }
    workflows.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(workflows)
}

/// Get the workflows directory (~/.alius/workflows/).
pub fn workflows_dir() -> std::path::PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".alius")
        .join("workflows")
}

/// Helper to create a StepResult with timing fields set to None.
/// Timing is filled in by `execute_step` after execution.
fn step_result(step_id: String, output: String, success: bool) -> StepResult {
    StepResult {
        step_id,
        output,
        success,
        started_at: None,
        finished_at: None,
        duration_ms: None,
        trace_id: None,
        run_ref: None,
    }
}

/// Execute the core logic of a single step (without timeout/retry wrapping).
async fn execute_step_inner(
    step: &Step,
    ctx: &ExecutionContext,
    handle: &dyn LoopEngineHandle,
    mode: &str,
) -> Result<StepResult> {
    match step.step_type {
        StepType::Prompt => {
            let prompt = step.prompt.as_deref().unwrap_or("");
            let interpolated = ctx.interpolate(prompt);
            match handle.run_prompt(&interpolated, "Plan").await {
                Ok(output) => Ok(step_result(step.id.clone(), output, true)),
                Err(e) => Ok(step_result(step.id.clone(), format!("Error: {}", e), false)),
            }
        }
        StepType::Tool => {
            let tool_name = step.tool.as_deref().unwrap_or("");
            let args = step
                .args
                .as_ref()
                .map(|a| ctx.interpolate_json(a))
                .unwrap_or(serde_json::json!({}));
            match handle.run_tool(tool_name, args, mode).await {
                Ok(output) => Ok(step_result(step.id.clone(), output.to_string(), true)),
                Err(e) => Ok(step_result(step.id.clone(), format!("Error: {}", e), false)),
            }
        }
        StepType::Http => {
            let url = ctx.interpolate(step.url.as_deref().unwrap_or(""));
            let method = step.method.as_deref().unwrap_or("POST");
            let body = step.body.as_ref().map(|b| ctx.interpolate_json(b));

            let client = reqwest::Client::builder()
                .user_agent(user_agent())
                .build()?;
            let mut req = match method.to_uppercase().as_str() {
                "GET" => client.get(&url),
                "POST" => client.post(&url),
                "PUT" => client.put(&url),
                "DELETE" => client.delete(&url),
                _ => client.post(&url),
            };

            if let Some(body) = &body {
                req = req.json(body);
            }

            let resp = req.send().await?;
            let status = resp.status();
            let text = resp.text().await?;

            Ok(step_result(
                step.id.clone(),
                if status.is_success() {
                    text
                } else {
                    format!("HTTP {}: {}", status, text)
                },
                status.is_success(),
            ))
        }
        StepType::Condition => {
            // Condition format: step_id.field operator "value"
            // Supported: check if a step's output contains a substring
            let condition = step.prompt.as_deref().unwrap_or("");
            let interpolated = ctx.interpolate(condition);
            // Simple evaluation: "step_id contains text" or "step_id success"
            let parts: Vec<&str> = interpolated.split_whitespace().collect();
            let (result, success) = if parts.len() >= 3 && parts[1] == "contains" {
                let target_step = parts[0];
                let needle = parts[2..].join(" ");
                match ctx.results.get(target_step) {
                    Some(r) => (r.output.contains(&needle), true),
                    None => (false, false),
                }
            } else if parts.len() >= 2 && parts[1] == "success" {
                let target_step = parts[0];
                match ctx.results.get(target_step) {
                    Some(r) => (r.success, true),
                    None => (false, false),
                }
            } else if parts.len() >= 2 && parts[1] == "failed" {
                let target_step = parts[0];
                match ctx.results.get(target_step) {
                    Some(r) => (!r.success, true),
                    None => (false, false),
                }
            } else {
                // Default: evaluate as truthy if non-empty
                (!interpolated.is_empty(), true)
            };
            Ok(step_result(step.id.clone(), result.to_string(), success))
        }
    }
}

/// Execute a workflow step with timeout and retry support.
///
/// - If `step.timeout_ms` is set, the execution is wrapped in `tokio::time::timeout`.
/// - If `step.on_failure` is `Retry`, failed steps are retried up to `max_retries` times
///   with `backoff_ms` delay between attempts.
pub async fn execute_step(
    step: &Step,
    ctx: &ExecutionContext,
    handle: &dyn LoopEngineHandle,
    mode: &str,
) -> Result<StepResult> {
    let (max_retries, backoff_ms) = match &step.on_failure {
        OnFailurePolicy::Retry {
            max_retries,
            backoff_ms,
        } => (*max_retries, *backoff_ms),
        _ => (0, 0),
    };

    let mut last_result = None;
    for attempt in 0..=max_retries {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(backoff_ms)).await;
        }

        let started_at = chrono::Utc::now();
        let mut result = if let Some(timeout_ms) = step.timeout_ms {
            match tokio::time::timeout(
                std::time::Duration::from_millis(timeout_ms),
                execute_step_inner(step, ctx, handle, mode),
            )
            .await
            {
                Ok(inner) => inner?,
                Err(_) => StepResult {
                    step_id: step.id.clone(),
                    output: format!("Timeout: step exceeded {}ms", timeout_ms),
                    success: false,
                    started_at: None,
                    finished_at: None,
                    duration_ms: None,
                    trace_id: None,
                    run_ref: None,
                },
            }
        } else {
            execute_step_inner(step, ctx, handle, mode).await?
        };

        // Record timing metadata.
        let finished_at = chrono::Utc::now();
        result.started_at = Some(started_at);
        result.finished_at = Some(finished_at);
        result.duration_ms =
            Some((finished_at.timestamp_millis() - started_at.timestamp_millis()).max(0) as u64);

        if result.success || attempt >= max_retries {
            return Ok(result);
        }
        last_result = Some(result);
    }

    // Unreachable in practice, but satisfy the compiler.
    Ok(last_result.unwrap_or(StepResult {
        step_id: step.id.clone(),
        output: "Retry exhausted".to_string(),
        success: false,
        started_at: None,
        finished_at: None,
        duration_ms: None,
        trace_id: None,
        run_ref: None,
    }))
}

/// Save a workflow run record to disk at `~/.alius/workflows/runs/`.
fn save_run_record(record: &WorkflowRunRecord) -> Result<()> {
    let runs_dir = workflows_dir().join("runs");
    std::fs::create_dir_all(&runs_dir)?;

    let ts = record.started_at.format("%Y%m%d-%H%M%S");
    let filename = format!("{}-{}.json", ts, record.workflow_name);
    let path = runs_dir.join(filename);

    let json = serde_json::to_string_pretty(record)?;
    std::fs::write(&path, json)?;
    Ok(())
}

/// Execute a full workflow using the provided runtime handle.
///
/// Respects each step's `on_failure` policy:
/// - `Abort`: stop workflow on failure (default).
/// - `Skip`: log warning, continue to next step.
/// - `Retry`: retry the step (handled inside `execute_step`), abort if exhausted.
///
/// If a `CancellationToken` is provided, it is checked before each step.
/// Cancelling the token will abort the workflow gracefully.
pub async fn execute_workflow(
    workflow: &Workflow,
    handle: &dyn LoopEngineHandle,
    cancel_token: Option<tokio_util::sync::CancellationToken>,
) -> Result<(ExecutionContext, WorkflowRunRecord)> {
    let workflow_started_at = chrono::Utc::now();
    let mut ctx = ExecutionContext::new();
    let mut aborted = false;
    let mut cancelled = false;

    println!("Running workflow: {}", workflow.name);
    if !workflow.description.is_empty() {
        println!("  {}", workflow.description);
    }
    println!();

    for step in &workflow.steps {
        // Check cancellation before each step.
        if let Some(ref token) = cancel_token {
            if token.is_cancelled() {
                println!("  Workflow cancelled before step '{}'", step.id);
                cancelled = true;
                break;
            }
        }

        if aborted {
            break;
        }

        println!("  Step: {} ({:?})", step.id, step.step_type);
        let result = execute_step(step, &ctx, handle, &workflow.mode).await?;
        let is_success = result.success;

        println!(
            "    {} {}",
            if result.success { "OK" } else { "ERR" },
            if result.output.len() > 100 {
                &result.output[..100]
            } else {
                &result.output
            }
        );
        ctx.results.insert(step.id.clone(), result);

        if !is_success {
            match &step.on_failure {
                OnFailurePolicy::Abort => {
                    println!("  Workflow aborted at step '{}'", step.id);
                    aborted = true;
                }
                OnFailurePolicy::Skip => {
                    println!("  Step '{}' failed — skipping", step.id);
                }
                OnFailurePolicy::Retry { .. } => {
                    // Retry was already attempted inside execute_step.
                    // If we reach here with success=false, retries are exhausted.
                    println!(
                        "  Step '{}' failed after retries — aborting workflow",
                        step.id
                    );
                    aborted = true;
                }
            }
        }
    }

    println!();
    println!(
        "Workflow complete: {} steps executed{}",
        ctx.results.len(),
        if aborted {
            " (aborted)"
        } else if cancelled {
            " (cancelled)"
        } else {
            ""
        }
    );

    let workflow_finished_at = chrono::Utc::now();
    let status = if cancelled {
        WorkflowRunStatus::Cancelled
    } else if aborted || ctx.results.values().any(|r| !r.success) {
        WorkflowRunStatus::Failed
    } else {
        WorkflowRunStatus::Completed
    };

    let record = WorkflowRunRecord {
        workflow_name: workflow.name.clone(),
        status,
        started_at: workflow_started_at,
        finished_at: workflow_finished_at,
        duration_ms: (workflow_finished_at.timestamp_millis()
            - workflow_started_at.timestamp_millis())
        .max(0) as u64,
        steps: ctx.results.values().cloned().collect(),
    };

    // Persist the run record (best-effort, don't fail the workflow).
    if let Err(e) = save_run_record(&record) {
        eprintln!("Warning: failed to save workflow run record: {}", e);
    }

    Ok((ctx, record))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};

    /// A mock handle that records calls and returns configurable results.
    struct MockHandle {
        prompt_results: Arc<Mutex<Vec<String>>>,
        tool_results: Arc<Mutex<Vec<serde_json::Value>>>,
    }

    impl MockHandle {
        #[allow(clippy::type_complexity)]
        fn new() -> (
            Self,
            Arc<Mutex<Vec<String>>>,
            Arc<Mutex<Vec<serde_json::Value>>>,
        ) {
            let prompt_results = Arc::new(Mutex::new(Vec::new()));
            let tool_results = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    prompt_results: Arc::clone(&prompt_results),
                    tool_results: Arc::clone(&tool_results),
                },
                prompt_results,
                tool_results,
            )
        }
    }

    #[async_trait]
    impl LoopEngineHandle for MockHandle {
        async fn run_prompt(&self, text: &str, _mode: &str) -> Result<String> {
            let response = format!("LLM response to: {}", text);
            self.prompt_results.lock().unwrap().push(text.to_string());
            Ok(response)
        }

        async fn run_tool(
            &self,
            tool_name: &str,
            args: serde_json::Value,
            _mode: &str,
        ) -> Result<serde_json::Value> {
            let result = serde_json::json!({"tool": tool_name, "args": args, "result": "ok"});
            self.tool_results.lock().unwrap().push(result.clone());
            Ok(result)
        }
    }

    #[tokio::test]
    async fn test_prompt_step_calls_runtime() {
        let (handle, prompts, _) = MockHandle::new();
        let step = Step {
            id: "s1".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("Hello {{name}}".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::default(),
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("LLM response to"));
        assert_eq!(prompts.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_tool_step_calls_runtime() {
        let (handle, _, tools) = MockHandle::new();
        let step = Step {
            id: "s1".to_string(),
            step_type: StepType::Tool,
            prompt: None,
            tool: Some("read_file".to_string()),
            args: Some(serde_json::json!({"path": "src/main.rs"})),
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::default(),
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("read_file"));
        assert_eq!(tools.lock().unwrap().len(), 1);
    }

    #[tokio::test]
    async fn test_condition_step_contains() {
        let (handle, _, _) = MockHandle::new();
        let mut ctx = ExecutionContext::new();
        ctx.results.insert(
            "prev".to_string(),
            step_result("prev".to_string(), "hello world".to_string(), true),
        );

        let step = Step {
            id: "cond".to_string(),
            step_type: StepType::Condition,
            prompt: Some("prev contains hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::default(),
            timeout_ms: None,
        };
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "true");
    }

    #[tokio::test]
    async fn test_condition_step_success() {
        let (handle, _, _) = MockHandle::new();
        let mut ctx = ExecutionContext::new();
        ctx.results.insert(
            "prev".to_string(),
            step_result("prev".to_string(), "ok".to_string(), true),
        );

        let step = Step {
            id: "cond".to_string(),
            step_type: StepType::Condition,
            prompt: Some("prev success".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::default(),
            timeout_ms: None,
        };
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.success);
        assert_eq!(result.output, "true");
    }

    #[tokio::test]
    async fn test_execute_workflow_with_handle() {
        let (handle, prompts, tools) = MockHandle::new();
        let workflow = Workflow {
            name: "test".to_string(),
            description: "test workflow".to_string(),
            steps: vec![
                Step {
                    id: "ask".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("What is Rust?".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::default(),
                    timeout_ms: None,
                },
                Step {
                    id: "read".to_string(),
                    step_type: StepType::Tool,
                    prompt: None,
                    tool: Some("read_file".to_string()),
                    args: Some(serde_json::json!({"path": "Cargo.toml"})),
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::default(),
                    timeout_ms: None,
                },
            ],
            mode: "chat".to_string(),
            timeout_ms: None,
        };

        let (ctx, _record) = execute_workflow(&workflow, &handle, None).await.unwrap();
        assert_eq!(ctx.results.len(), 2);
        assert!(ctx.results["ask"].success);
        assert!(ctx.results["read"].success);
        assert_eq!(prompts.lock().unwrap().len(), 1);
        assert_eq!(tools.lock().unwrap().len(), 1);
    }

    #[test]
    fn test_interpolation() {
        let mut ctx = ExecutionContext::new();
        ctx.results.insert(
            "step1".to_string(),
            step_result("step1".to_string(), "world".to_string(), true),
        );
        assert_eq!(ctx.interpolate("Hello {{step1.output}}!"), "Hello world!");
    }

    #[tokio::test]
    async fn test_stub_handle() {
        let handle = StubLoopEngineHandle;
        let result = handle.run_prompt("test", "Plan").await.unwrap();
        assert!(result.contains("[prompt]"));
        let result = handle
            .run_tool("shell", serde_json::json!({"cmd": "ls"}), "chat")
            .await
            .unwrap();
        assert!(result["tool"].as_str().unwrap() == "shell");
    }

    /// Integration test: RuntimeWorkflowHandle exercises the real CoreRuntime
    /// and ToolRegistry paths — output must NOT contain stub markers.
    #[tokio::test(flavor = "multi_thread")]
    async fn test_runtime_handle_uses_real_paths() {
        use protocol_interface::core::WorkspaceRef;
        use runtime_model::{ChatEvent, ChatStream, LlmClient, LlmProvider, ToolCall};
        use runtime_tools::{AliusTool, ToolResult as RealToolResult};
        use std::future::Future;
        use std::pin::Pin;

        // Fake LLM provider: returns known text, no tool calls.
        struct FakeProvider;
        impl LlmProvider for FakeProvider {
            fn chat_stream<'a>(
                &'a self,
                _conv: &'a runtime_model::Conversation,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<ChatStream>> + Send + 'a>> {
                Box::pin(async {
                    let stream: ChatStream = Box::pin(futures::stream::iter(vec![
                        Ok(ChatEvent::Delta {
                            text: "real-llm-response".to_string(),
                        }),
                        Ok(ChatEvent::Done {
                            full_response: String::new(),
                        }),
                    ]));
                    Ok(stream)
                })
            }
            fn chat_once<'a>(
                &'a self,
                _prompt: &'a str,
                _system: Option<&'a str>,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<String>> + Send + 'a>> {
                Box::pin(async { Ok("real-llm-response".to_string()) })
            }
            fn list_models<'a>(
                &'a self,
            ) -> Pin<Box<dyn Future<Output = anyhow::Result<Vec<String>>> + Send + 'a>>
            {
                Box::pin(async { Ok(vec!["fake-model".to_string()]) })
            }
            fn chat_stream_with_tools<'a>(
                &'a self,
                _conv: &'a runtime_model::Conversation,
                _tools: &'a [protocol_interface::ToolDef],
            ) -> Pin<Box<dyn Future<Output = runtime_model::ToolResponse> + Send + 'a>>
            {
                Box::pin(async {
                    let stream: ChatStream = Box::pin(futures::stream::iter(vec![
                        Ok(ChatEvent::Delta {
                            text: "real-llm-response".to_string(),
                        }),
                        Ok(ChatEvent::Done {
                            full_response: String::new(),
                        }),
                    ]));
                    Ok((stream, None))
                })
            }
            fn continue_with_tool_results<'a>(
                &'a self,
                _conv: &'a runtime_model::Conversation,
                _results: &'a [(String, String, String)],
                _calls: &'a [ToolCall],
                _tools: &'a [protocol_interface::ToolDef],
            ) -> Pin<Box<dyn Future<Output = runtime_model::ToolResponse> + Send + 'a>>
            {
                Box::pin(async {
                    let stream: ChatStream =
                        Box::pin(futures::stream::iter(vec![Ok(ChatEvent::Done {
                            full_response: String::new(),
                        })]));
                    Ok((stream, None))
                })
            }
        }

        // Fake tool: returns known output through real ToolRegistry path.
        struct FakeTool;
        #[async_trait::async_trait]
        impl AliusTool for FakeTool {
            fn name(&self) -> &'static str {
                "fake_greet"
            }
            fn description(&self) -> &'static str {
                "fake tool for testing"
            }
            fn input_schema(&self) -> serde_json::Value {
                serde_json::json!({"type": "object", "properties": {}})
            }
            async fn execute(
                &self,
                _args: serde_json::Value,
                _ctx: runtime_tools::ToolContext,
            ) -> Result<RealToolResult, protocol_interface::AliusError> {
                Ok(RealToolResult::success("real-tool-executed".to_string()))
            }
        }

        // Build CoreRuntime with fake provider and tool registry.
        let registry = Arc::new({
            let reg = runtime_tools::ToolRegistry::new();
            reg.register(FakeTool).unwrap();
            reg
        });
        let client = LlmClient::new_with_provider_for_test(
            Box::new(FakeProvider),
            "fake-model",
            protocol_interface::ProviderType::Openai,
        );
        let tmp = tempfile::TempDir::new().unwrap();
        let runtime = core_runtime::CoreRuntimeBuilder::new()
            .workspace_ref(WorkspaceRef::new(tmp.path()))
            .settings(runtime_config::Settings::default())
            .client(client)
            .tool_registry_arc(registry.clone())
            .build()
            .unwrap();
        let manager = core_runtime::CoreRuntimeManager::from_runtime(tmp.path(), runtime);

        let handle = RuntimeWorkflowHandle::new(manager, registry);

        // Test run_prompt: must go through real LLM, not return "[prompt] ...".
        let prompt_result = handle.run_prompt("Say hello", "Chat").await.unwrap();
        assert!(
            !prompt_result.contains("[prompt]"),
            "RuntimeWorkflowHandle must not use stub — got: {}",
            prompt_result
        );
        assert!(
            prompt_result.contains("real-llm-response"),
            "Expected real LLM response — got: {}",
            prompt_result
        );

        // Test run_tool: must go through real ToolRegistry, not return "[tool:...]".
        let tool_result = handle
            .run_tool("fake_greet", serde_json::json!({}), "chat")
            .await
            .unwrap();
        let output = tool_result["output"].as_str().unwrap();
        assert!(
            !output.contains("[tool:"),
            "RuntimeWorkflowHandle must not use stub — got: {}",
            output
        );
        assert!(
            output.contains("real-tool-executed"),
            "Expected real tool output — got: {}",
            output
        );
    }

    // ---- Step 1.1: Schema extension tests ----

    #[test]
    fn test_backward_compat_deserialize_without_new_fields() {
        let json = r#"{
            "name": "legacy",
            "steps": [{"id": "s1", "type": "prompt", "prompt": "hello"}]
        }"#;
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert_eq!(wf.name, "legacy");
        assert_eq!(wf.mode, "chat"); // default
        assert_eq!(wf.timeout_ms, None);
        assert_eq!(wf.steps[0].on_failure, OnFailurePolicy::Abort); // default
        assert_eq!(wf.steps[0].timeout_ms, None);
    }

    #[test]
    fn test_deserialize_on_failure_abort() {
        let json = r#"{"id": "s1", "type": "prompt", "prompt": "hi", "on_failure": "abort"}"#;
        let step: Step = serde_json::from_str(json).unwrap();
        assert_eq!(step.on_failure, OnFailurePolicy::Abort);
    }

    #[test]
    fn test_deserialize_on_failure_skip() {
        let json = r#"{"id": "s1", "type": "prompt", "prompt": "hi", "on_failure": "skip"}"#;
        let step: Step = serde_json::from_str(json).unwrap();
        assert_eq!(step.on_failure, OnFailurePolicy::Skip);
    }

    #[test]
    fn test_deserialize_on_failure_retry() {
        let json = r#"{"id": "s1", "type": "prompt", "prompt": "hi", "on_failure": {"retry": {"max_retries": 3, "backoff_ms": 500}}}"#;
        let step: Step = serde_json::from_str(json).unwrap();
        assert_eq!(
            step.on_failure,
            OnFailurePolicy::Retry {
                max_retries: 3,
                backoff_ms: 500,
            }
        );
    }

    #[test]
    fn test_deserialize_on_failure_retry_default_backoff() {
        let json = r#"{"id": "s1", "type": "prompt", "prompt": "hi", "on_failure": {"retry": {"max_retries": 2}}}"#;
        let step: Step = serde_json::from_str(json).unwrap();
        assert_eq!(
            step.on_failure,
            OnFailurePolicy::Retry {
                max_retries: 2,
                backoff_ms: 1000, // default
            }
        );
    }

    #[test]
    fn test_deserialize_step_timeout_ms() {
        let json = r#"{"id": "s1", "type": "prompt", "prompt": "hi", "timeout_ms": 5000}"#;
        let step: Step = serde_json::from_str(json).unwrap();
        assert_eq!(step.timeout_ms, Some(5000));
    }

    #[test]
    fn test_deserialize_workflow_mode_and_timeout() {
        let json = r#"{
            "name": "test",
            "mode": "plan",
            "timeout_ms": 60000,
            "steps": [{"id": "s1", "type": "prompt", "prompt": "hi"}]
        }"#;
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert_eq!(wf.mode, "plan");
        assert_eq!(wf.timeout_ms, Some(60000));
    }

    #[test]
    fn test_serialize_roundtrip_with_new_fields() {
        let wf = Workflow {
            name: "roundtrip".to_string(),
            description: String::new(),
            steps: vec![Step {
                id: "s1".to_string(),
                step_type: StepType::Prompt,
                prompt: Some("hi".to_string()),
                tool: None,
                args: None,
                url: None,
                method: None,
                body: None,
                on_failure: OnFailurePolicy::Retry {
                    max_retries: 3,
                    backoff_ms: 500,
                },
                timeout_ms: Some(5000),
            }],
            mode: "plan".to_string(),
            timeout_ms: Some(60000),
        };
        let json = serde_json::to_string(&wf).unwrap();
        let wf2: Workflow = serde_json::from_str(&json).unwrap();
        assert_eq!(wf2.mode, "plan");
        assert_eq!(wf2.timeout_ms, Some(60000));
        assert_eq!(wf2.steps[0].timeout_ms, Some(5000));
        assert_eq!(
            wf2.steps[0].on_failure,
            OnFailurePolicy::Retry {
                max_retries: 3,
                backoff_ms: 500,
            }
        );
    }

    // ---- Step 1.8: Example file tests ----

    #[test]
    fn test_example_simple_prompt() {
        let json = include_str!("../../../../examples/workflows/simple-prompt.json");
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert_eq!(wf.name, "simple-prompt");
        assert_eq!(wf.steps.len(), 1);
        assert_eq!(wf.steps[0].step_type, StepType::Prompt);
    }

    #[test]
    fn test_example_tool_with_retry() {
        let json = include_str!("../../../../examples/workflows/tool-with-retry.json");
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert_eq!(wf.name, "tool-with-retry");
        assert_eq!(wf.steps.len(), 2);
        assert_eq!(
            wf.steps[0].on_failure,
            OnFailurePolicy::Retry {
                max_retries: 3,
                backoff_ms: 500,
            }
        );
        assert_eq!(wf.steps[0].timeout_ms, Some(5000));
    }

    #[test]
    fn test_example_multi_step_failure_policy() {
        let json =
            include_str!("../../../../examples/workflows/multi-step-with-failure-policy.json");
        let wf: Workflow = serde_json::from_str(json).unwrap();
        assert_eq!(wf.name, "multi-step-failure-policy");
        assert_eq!(wf.mode, "plan");
        assert_eq!(wf.timeout_ms, Some(120000));
        assert_eq!(wf.steps.len(), 4);
        assert_eq!(wf.steps[0].on_failure, OnFailurePolicy::Skip);
        assert_eq!(wf.steps[3].on_failure, OnFailurePolicy::Abort);
    }

    // ---- Step 1.5: Timing metadata tests ----

    #[tokio::test]
    async fn test_step_result_has_timestamps() {
        let (handle, _, _) = MockHandle::new();
        let step = Step {
            id: "timed".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::Abort,
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.success);
        assert!(result.started_at.is_some());
        assert!(result.finished_at.is_some());
        assert!(result.started_at.unwrap() <= result.finished_at.unwrap());
    }

    #[tokio::test]
    async fn test_step_result_has_duration() {
        let (handle, _, _) = MockHandle::new();
        let step = Step {
            id: "dur".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::Abort,
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.duration_ms.is_some());
        // Duration should be >= 0 (it's always 0+ for a fast mock)
        assert!(result.duration_ms.unwrap() < 10000); // sanity check: less than 10s
    }

    // ---- Step 1.2: Timeout and retry tests ----

    /// A mock handle that fails N times then succeeds.
    struct FailThenSucceedHandle {
        remaining_fails: Arc<Mutex<u32>>,
    }

    impl FailThenSucceedHandle {
        fn new(fail_count: u32) -> (Self, Arc<Mutex<u32>>) {
            let remaining = Arc::new(Mutex::new(fail_count));
            (
                Self {
                    remaining_fails: Arc::clone(&remaining),
                },
                remaining,
            )
        }
    }

    #[async_trait]
    impl LoopEngineHandle for FailThenSucceedHandle {
        async fn run_prompt(&self, text: &str, _mode: &str) -> Result<String> {
            let mut remaining = self.remaining_fails.lock().unwrap();
            if *remaining > 0 {
                *remaining -= 1;
                Err(anyhow::anyhow!("simulated failure"))
            } else {
                Ok(format!("success: {}", text))
            }
        }

        async fn run_tool(
            &self,
            tool_name: &str,
            _args: serde_json::Value,
            _mode: &str,
        ) -> Result<serde_json::Value> {
            let mut remaining = self.remaining_fails.lock().unwrap();
            if *remaining > 0 {
                *remaining -= 1;
                Err(anyhow::anyhow!("tool failure"))
            } else {
                Ok(serde_json::json!({"tool": tool_name, "result": "ok"}))
            }
        }
    }

    /// A mock handle that always sleeps longer than the timeout.
    struct SlowHandle;

    #[async_trait]
    impl LoopEngineHandle for SlowHandle {
        async fn run_prompt(&self, _text: &str, _mode: &str) -> Result<String> {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            Ok("should not reach here".to_string())
        }

        async fn run_tool(
            &self,
            _tool_name: &str,
            _args: serde_json::Value,
            _mode: &str,
        ) -> Result<serde_json::Value> {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            Ok(serde_json::json!({}))
        }
    }

    #[tokio::test]
    async fn test_step_timeout_returns_failure() {
        let step = Step {
            id: "slow".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::Abort,
            timeout_ms: Some(100), // 100ms timeout
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &SlowHandle, "chat")
            .await
            .unwrap();
        assert!(!result.success);
        assert!(result.output.contains("Timeout"));
        assert!(result.output.contains("100ms"));
    }

    #[tokio::test]
    async fn test_step_retry_succeeds_on_second_attempt() {
        let (handle, remaining) = FailThenSucceedHandle::new(1); // fail once, then succeed
        let step = Step {
            id: "retry".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::Retry {
                max_retries: 3,
                backoff_ms: 10, // short backoff for test
            },
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(result.success);
        assert!(result.output.contains("success"));
        assert_eq!(*remaining.lock().unwrap(), 0); // all retries consumed
    }

    #[tokio::test]
    async fn test_step_retry_exhausted_returns_failure() {
        let (handle, remaining) = FailThenSucceedHandle::new(5); // always fail
        let step = Step {
            id: "retry".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::Retry {
                max_retries: 2,
                backoff_ms: 10,
            },
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(!result.success);
        assert!(result.output.contains("simulated failure"));
        // Should have tried 3 times total (1 initial + 2 retries)
        assert_eq!(*remaining.lock().unwrap(), 2); // 5 - 3 = 2 remaining
    }

    #[tokio::test]
    async fn test_step_abort_on_failure_no_retry() {
        let (handle, remaining) = FailThenSucceedHandle::new(1);
        let step = Step {
            id: "no-retry".to_string(),
            step_type: StepType::Prompt,
            prompt: Some("hello".to_string()),
            tool: None,
            args: None,
            url: None,
            method: None,
            body: None,
            on_failure: OnFailurePolicy::Abort, // default, no retry
            timeout_ms: None,
        };
        let ctx = ExecutionContext::new();
        let result = execute_step(&step, &ctx, &handle, "chat").await.unwrap();
        assert!(!result.success);
        assert_eq!(*remaining.lock().unwrap(), 0); // only 1 attempt
    }

    // ---- Step 1.3: on_failure policy in execute_workflow tests ----

    #[tokio::test]
    async fn test_workflow_abort_on_failure() {
        // Use a custom handle that fails on "fail-me" prompt.
        struct FailOnSpecific {
            fail_on: String,
        }
        #[async_trait]
        impl LoopEngineHandle for FailOnSpecific {
            async fn run_prompt(&self, text: &str, _mode: &str) -> Result<String> {
                if text.contains(&self.fail_on) {
                    Err(anyhow::anyhow!("deliberate failure"))
                } else {
                    Ok(format!("ok: {}", text))
                }
            }
            async fn run_tool(
                &self,
                tool_name: &str,
                _args: serde_json::Value,
                _mode: &str,
            ) -> Result<serde_json::Value> {
                Ok(serde_json::json!({"tool": tool_name, "result": "ok"}))
            }
        }

        let handle = FailOnSpecific {
            fail_on: "fail-me".to_string(),
        };
        let workflow = Workflow {
            name: "abort-test".to_string(),
            description: String::new(),
            steps: vec![
                Step {
                    id: "s1".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step one".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
                Step {
                    id: "s2".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("fail-me now".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
                Step {
                    id: "s3".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step three".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
            ],
            mode: "chat".to_string(),
            timeout_ms: None,
        };

        let (ctx, _record) = execute_workflow(&workflow, &handle, None).await.unwrap();
        assert_eq!(ctx.results.len(), 2); // s3 should NOT have executed
        assert!(ctx.results["s1"].success);
        assert!(!ctx.results["s2"].success);
        assert!(!ctx.results.contains_key("s3"));
    }

    #[tokio::test]
    async fn test_workflow_skip_on_failure() {
        struct FailOnSpecific {
            fail_on: String,
        }
        #[async_trait]
        impl LoopEngineHandle for FailOnSpecific {
            async fn run_prompt(&self, text: &str, _mode: &str) -> Result<String> {
                if text.contains(&self.fail_on) {
                    Err(anyhow::anyhow!("deliberate failure"))
                } else {
                    Ok(format!("ok: {}", text))
                }
            }
            async fn run_tool(
                &self,
                tool_name: &str,
                _args: serde_json::Value,
                _mode: &str,
            ) -> Result<serde_json::Value> {
                Ok(serde_json::json!({"tool": tool_name, "result": "ok"}))
            }
        }

        let handle = FailOnSpecific {
            fail_on: "fail-me".to_string(),
        };
        let workflow = Workflow {
            name: "skip-test".to_string(),
            description: String::new(),
            steps: vec![
                Step {
                    id: "s1".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step one".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Skip,
                    timeout_ms: None,
                },
                Step {
                    id: "s2".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("fail-me now".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Skip,
                    timeout_ms: None,
                },
                Step {
                    id: "s3".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step three".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Skip,
                    timeout_ms: None,
                },
            ],
            mode: "chat".to_string(),
            timeout_ms: None,
        };

        let (ctx, _record) = execute_workflow(&workflow, &handle, None).await.unwrap();
        assert_eq!(ctx.results.len(), 3); // all 3 steps executed
        assert!(ctx.results["s1"].success);
        assert!(!ctx.results["s2"].success);
        assert!(ctx.results["s3"].success); // s3 still runs because s2 was skipped
    }

    #[tokio::test]
    async fn test_workflow_retry_then_abort() {
        let (handle, _remaining) = FailThenSucceedHandle::new(10); // always fail
        let workflow = Workflow {
            name: "retry-abort-test".to_string(),
            description: String::new(),
            steps: vec![
                Step {
                    id: "s1".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step one".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Retry {
                        max_retries: 2,
                        backoff_ms: 10,
                    },
                    timeout_ms: None,
                },
                Step {
                    id: "s2".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step two".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
            ],
            mode: "chat".to_string(),
            timeout_ms: None,
        };

        let (ctx, _record) = execute_workflow(&workflow, &handle, None).await.unwrap();
        // s1 retries 3 times (1 initial + 2 retries), all fail, then workflow aborts
        // s2 should NOT execute
        assert_eq!(ctx.results.len(), 1);
        assert!(!ctx.results["s1"].success);
        assert!(!ctx.results.contains_key("s2"));
    }

    // ---- Step 1.4: CancellationToken tests ----

    #[tokio::test]
    async fn test_workflow_cancel_before_start() {
        let (handle, _, _) = MockHandle::new();
        let token = tokio_util::sync::CancellationToken::new();
        token.cancel(); // cancel immediately

        let workflow = Workflow {
            name: "cancel-test".to_string(),
            description: String::new(),
            steps: vec![Step {
                id: "s1".to_string(),
                step_type: StepType::Prompt,
                prompt: Some("hello".to_string()),
                tool: None,
                args: None,
                url: None,
                method: None,
                body: None,
                on_failure: OnFailurePolicy::Abort,
                timeout_ms: None,
            }],
            mode: "chat".to_string(),
            timeout_ms: None,
        };

        let (ctx, record) = execute_workflow(&workflow, &handle, Some(token))
            .await
            .unwrap();
        assert_eq!(ctx.results.len(), 0); // no steps executed
        assert_eq!(record.status, WorkflowRunStatus::Cancelled);
    }

    #[tokio::test]
    async fn test_workflow_cancel_mid_execution() {
        // Use a slow handle that gives us time to cancel between steps.
        struct SlowThenCheckHandle {
            call_count: Arc<Mutex<u32>>,
            cancel_token: tokio_util::sync::CancellationToken,
        }
        #[async_trait]
        impl LoopEngineHandle for SlowThenCheckHandle {
            async fn run_prompt(&self, text: &str, _mode: &str) -> Result<String> {
                let mut count = self.call_count.lock().unwrap();
                *count += 1;
                if *count >= 2 {
                    // Cancel after first step completes
                    self.cancel_token.cancel();
                }
                Ok(format!("ok: {}", text))
            }
            async fn run_tool(
                &self,
                tool_name: &str,
                _args: serde_json::Value,
                _mode: &str,
            ) -> Result<serde_json::Value> {
                Ok(serde_json::json!({"tool": tool_name}))
            }
        }

        let token = tokio_util::sync::CancellationToken::new();
        let handle = SlowThenCheckHandle {
            call_count: Arc::new(Mutex::new(0)),
            cancel_token: token.clone(),
        };

        let workflow = Workflow {
            name: "cancel-mid".to_string(),
            description: String::new(),
            steps: vec![
                Step {
                    id: "s1".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step one".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
                Step {
                    id: "s2".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step two".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
                Step {
                    id: "s3".to_string(),
                    step_type: StepType::Prompt,
                    prompt: Some("step three".to_string()),
                    tool: None,
                    args: None,
                    url: None,
                    method: None,
                    body: None,
                    on_failure: OnFailurePolicy::Abort,
                    timeout_ms: None,
                },
            ],
            mode: "chat".to_string(),
            timeout_ms: None,
        };

        let (ctx, _record) = execute_workflow(&workflow, &handle, Some(token))
            .await
            .unwrap();
        // s1 executes, s2 executes (cancel happens during s2's run_prompt),
        // s3 is skipped because cancel is checked before it.
        assert!(ctx.results.contains_key("s1"));
        assert!(ctx.results.contains_key("s2"));
        assert!(!ctx.results.contains_key("s3"));
        assert_eq!(ctx.results.len(), 2);
    }
}
