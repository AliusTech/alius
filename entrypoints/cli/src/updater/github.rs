use serde::Deserialize;

const REPO: &str = "AliusTech/alius";
const API_URL: &str = "https://api.github.com";

#[derive(Debug, Deserialize)]
pub struct ReleaseAsset {
    pub name: String,
    pub browser_download_url: String,
    #[allow(dead_code)]
    pub size: u64,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseInfo {
    pub tag_name: String,
    pub assets: Vec<ReleaseAsset>,
}

impl ReleaseInfo {
    /// The version string with leading 'v' stripped.
    pub fn version(&self) -> &str {
        self.tag_name.strip_prefix('v').unwrap_or(&self.tag_name)
    }

    /// Find the download URL for a specific asset name.
    pub fn asset_url(&self, asset_name: &str) -> Option<&str> {
        self.assets
            .iter()
            .find(|a| a.name == asset_name)
            .map(|a| a.browser_download_url.as_str())
    }
}

/// Fetch the latest release from GitHub.
pub async fn fetch_latest_release() -> anyhow::Result<ReleaseInfo> {
    let url = format!("{}/repos/{}/releases/latest", API_URL, REPO);
    let client = reqwest::Client::builder()
        .user_agent(super::user_agent())
        .build()?;
    let resp = client.get(&url).send().await?;

    if resp.status().is_client_error() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        anyhow::bail!("GitHub API error ({}): {}", status, body);
    }

    resp.json::<ReleaseInfo>().await.map_err(Into::into)
}

/// Compare two semver strings (major.minor.patch).
/// Returns true if `remote` is strictly greater than `local`.
pub fn is_newer_version(local: &str, remote: &str) -> bool {
    let parse = |v: &str| -> Vec<u32> { v.split('.').filter_map(|s| s.parse().ok()).collect() };
    let lv = parse(local);
    let rv = parse(remote);

    for i in 0..rv.len().max(lv.len()) {
        let l = lv.get(i).unwrap_or(&0);
        let r = rv.get(i).unwrap_or(&0);
        if r > l {
            return true;
        }
        if r < l {
            return false;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_normalization() {
        let info = ReleaseInfo {
            tag_name: "v0.1.0".to_string(),
            assets: vec![],
        };
        assert_eq!(info.version(), "0.1.0");
    }

    #[test]
    fn test_version_no_prefix() {
        let info = ReleaseInfo {
            tag_name: "0.2.0".to_string(),
            assets: vec![],
        };
        assert_eq!(info.version(), "0.2.0");
    }

    #[test]
    fn test_is_newer_version() {
        assert!(is_newer_version("0.0.2", "0.1.0"));
        assert!(is_newer_version("0.1.0", "0.1.1"));
        assert!(is_newer_version("0.1.9", "0.2.0"));
        assert!(!is_newer_version("0.2.0", "0.1.0"));
        assert!(!is_newer_version("0.1.0", "0.1.0"));
    }
}
