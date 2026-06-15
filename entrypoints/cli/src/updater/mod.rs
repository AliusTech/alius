mod github;
mod installer;
pub mod platform;

use anyhow::Result;
use runtime_config::Settings;

pub(crate) fn user_agent() -> String {
    format!("alius-cli/{}", env!("ALIUS_VERSION"))
}

/// Information about an available update.
pub struct UpdateInfo {
    pub current: &'static str,
    pub latest: String,
    pub download_url: String,
    pub asset_name: String,
}

/// Check if a newer version is available on GitHub.
pub async fn check_for_update() -> Result<Option<UpdateInfo>> {
    let current = env!("ALIUS_VERSION");
    let release = github::fetch_latest_release().await?;
    let latest = release.version().to_string();

    if !github::is_newer_version(current, &latest) {
        return Ok(None);
    }

    let asset_name = platform::target_asset_name().to_string();
    let download_url = release
        .asset_url(&asset_name)
        .ok_or_else(|| anyhow::anyhow!("No matching asset '{}' in release {}", asset_name, latest))?
        .to_string();

    Ok(Some(UpdateInfo {
        current,
        latest,
        download_url,
        asset_name,
    }))
}

/// Download and install the latest version.
pub async fn perform_update(info: &UpdateInfo) -> Result<()> {
    installer::download_and_install(&info.download_url, &info.asset_name).await
}

/// Whether we should auto-check for updates based on config and timestamp.
pub fn should_auto_check(settings: &Settings) -> bool {
    if !settings.update.auto_check {
        return false;
    }

    // Skip for development builds.
    if platform::detect_install_method() != platform::InstallMethod::Standalone {
        return false;
    }

    // Skip for version 0.0.0 (dev builds).
    if env!("ALIUS_VERSION") == "0.0.0" {
        return false;
    }

    let ts_path = timestamp_file_path();
    if !ts_path.exists() {
        return true;
    }

    let Ok(content) = std::fs::read_to_string(&ts_path) else {
        return true;
    };

    let Ok(last) = chrono::DateTime::parse_from_rfc3339(content.trim()) else {
        return true;
    };

    let elapsed = chrono::Utc::now().signed_duration_since(last.with_timezone(&chrono::Utc));
    let interval = chrono::Duration::hours(settings.update.check_interval_hours as i64);

    elapsed >= interval
}

/// Record that an update check was performed now.
pub fn record_check_time() -> Result<()> {
    let ts_path = timestamp_file_path();
    if let Some(parent) = ts_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&ts_path, chrono::Utc::now().to_rfc3339())?;
    Ok(())
}

/// Silent check suitable for startup: only prints a notice when an update is available.
pub async fn check_and_notify_silent() -> Result<()> {
    match check_for_update().await {
        Ok(Some(info)) => {
            println!(
                "A new version of alius is available: {} -> {}. Run `alius update install` to update.",
                info.current, info.latest
            );
        }
        Ok(None) => {}
        Err(_) => {} // Silently ignore network errors during auto-check.
    }
    let _ = record_check_time();
    Ok(())
}

fn timestamp_file_path() -> std::path::PathBuf {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    std::path::PathBuf::from(home)
        .join(".alius")
        .join("update_check_timestamp")
}
