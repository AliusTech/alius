//! Retry and timeout utilities

use anyhow::Result;
use std::time::Duration;
use tokio::time::timeout;

use alius_config::AgentSettings;

/// Execute with timeout
pub async fn with_timeout<F, T>(
    timeout_secs: u64,
    fut: F,
) -> Result<T>
where
    F: std::future::Future<Output = Result<T>>,
{
    timeout(Duration::from_secs(timeout_secs), fut)
        .await
        .map_err(|_| anyhow::anyhow!("Request timed out after {} seconds", timeout_secs))?
}

/// Execute with retry
pub async fn with_retry<F, Fut, T>(
    config: &AgentSettings,
    f: F,
) -> Result<T>
where
    F: Fn() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let max_retries = config.max_retries;
    let timeout_secs = config.timeout_seconds;

    for attempt in 0..max_retries {
        let result = with_timeout(timeout_secs, f()).await;

        match result {
            Ok(r) => return Ok(r),
            Err(e) if attempt < max_retries - 1 => {
                let delay = Duration::from_secs(2u64.pow(attempt));
                tokio::time::sleep(delay).await;
                tracing::warn!("Retry attempt {} after error: {}", attempt + 1, e);
            }
            Err(e) => return Err(e),
        }
    }

    Err(anyhow::anyhow!("Max retries exceeded"))
}