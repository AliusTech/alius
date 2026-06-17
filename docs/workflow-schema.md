# Workflow JSON Schema

## Workflow Object

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `name` | string | yes | — | Workflow name |
| `description` | string | no | `""` | Human-readable description |
| `mode` | string | no | `"chat"` | Execution mode: `"chat"` or `"plan"`. Controls tool confirmation behavior. |
| `timeout_ms` | integer | no | `null` | Overall workflow timeout in milliseconds |
| `steps` | Step[] | yes | — | Ordered list of steps to execute |

## Step Object

| Field | Type | Required | Default | Description |
|-------|------|----------|---------|-------------|
| `id` | string | yes | — | Unique step identifier |
| `type` | string | yes | — | Step type: `"prompt"`, `"tool"`, `"http"`, `"condition"` |
| `prompt` | string | no | `null` | For `prompt` type: LLM prompt text. For `condition`: expression. Supports `{{step_id.output}}` interpolation. |
| `tool` | string | no | `null` | For `tool` type: tool name to execute |
| `args` | object | no | `null` | For `tool` type: JSON arguments. Supports `{{var}}` interpolation. |
| `url` | string | no | `null` | For `http` type: request URL |
| `method` | string | no | `"POST"` | For `http` type: HTTP method |
| `body` | object | no | `null` | For `http` type: request body. Supports `{{var}}` interpolation. |
| `on_failure` | string/object | no | `"abort"` | Failure policy: `"abort"`, `"skip"`, or `{"retry": {"max_retries": N, "backoff_ms": M}}` |
| `timeout_ms` | integer | no | `null` | Per-step timeout in milliseconds |

## OnFailurePolicy

- `"abort"` — Stop workflow immediately (default)
- `"skip"` — Log warning, continue to next step
- `{"retry": {"max_retries": 3, "backoff_ms": 1000}}` — Retry up to N times with M ms delay between attempts. Default backoff: 1000ms.

## Execution Modes

- `"chat"` — Tool steps execute without confirmation prompts
- `"plan"` — Tool steps that require confirmation (e.g., high-risk shell commands) will fail closed with an error, since workflows have no interactive confirmation channel

## Variable Interpolation

Use `{{step_id.output}}` in strings to reference a previous step's output.

Example:
```json
{"prompt": "Analyze this: {{fetch_step.output}}"}
```

## Examples

See `examples/workflows/` for complete examples.
