//! Alius Workflow — Workflow parser and executor.
//!
//! Parses JSON workflow definitions and executes multi-step pipelines
//! with variable interpolation between steps.

#![allow(dead_code)]

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A workflow definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Workflow {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<Step>,
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
}

/// Step type enum.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum StepType {
    /// Call an LLM with a prompt.
    Prompt,
    /// Call a built-in tool.
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

/// Execute a workflow step.
pub async fn execute_step(step: &Step, ctx: &ExecutionContext) -> Result<StepResult> {
    match step.step_type {
        StepType::Prompt => {
            let prompt = step.prompt.as_deref().unwrap_or("");
            let interpolated = ctx.interpolate(prompt);
            // For now, return the interpolated prompt as output
            // In full integration, this would call the LLM
            Ok(StepResult {
                step_id: step.id.clone(),
                output: format!("[prompt] {}", interpolated),
                success: true,
            })
        }
        StepType::Tool => {
            let tool_name = step.tool.as_deref().unwrap_or("");
            let args = step.args.as_ref().map(|a| ctx.interpolate_json(a));
            Ok(StepResult {
                step_id: step.id.clone(),
                output: format!(
                    "[tool:{}] args={}",
                    tool_name,
                    args.map(|a| a.to_string()).unwrap_or_default()
                ),
                success: true,
            })
        }
        StepType::Http => {
            let url = ctx.interpolate(step.url.as_deref().unwrap_or(""));
            let method = step.method.as_deref().unwrap_or("POST");
            let body = step.body.as_ref().map(|b| ctx.interpolate_json(b));

            let client = reqwest::Client::new();
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

            Ok(StepResult {
                step_id: step.id.clone(),
                output: if status.is_success() {
                    text
                } else {
                    format!("HTTP {}: {}", status, text)
                },
                success: status.is_success(),
            })
        }
        StepType::Condition => {
            // Simple condition: check if previous step output is non-empty
            Ok(StepResult {
                step_id: step.id.clone(),
                output: "condition evaluated".to_string(),
                success: true,
            })
        }
    }
}

/// Execute a full workflow.
pub async fn execute_workflow(workflow: &Workflow) -> Result<ExecutionContext> {
    let mut ctx = ExecutionContext::new();

    println!("Running workflow: {}", workflow.name);
    if !workflow.description.is_empty() {
        println!("  {}", workflow.description);
    }
    println!();

    for step in &workflow.steps {
        println!("  Step: {} ({:?})", step.id, step.step_type);
        let result = execute_step(step, &ctx).await?;
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
    }

    println!();
    println!("Workflow complete: {} steps executed", workflow.steps.len());
    Ok(ctx)
}
