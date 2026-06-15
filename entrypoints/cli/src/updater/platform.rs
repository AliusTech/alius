use std::path::PathBuf;

/// How the CLI was installed.
#[derive(Debug, Clone, PartialEq)]
pub enum InstallMethod {
    /// Installed via npm (`@alius-tech/alius`).
    Npm,
    /// Installed via Homebrew (`brew install alius`).
    Homebrew,
    /// Standalone binary (direct download or cargo install).
    Standalone,
    /// Development build (cargo run / target/ directory).
    Development,
}

/// Return the release asset name matching the current platform.
pub fn target_asset_name() -> &'static str {
    if cfg!(target_os = "macos") && cfg!(target_arch = "aarch64") {
        "alius-macos-arm64.tar.gz"
    } else if cfg!(target_os = "macos") && cfg!(target_arch = "x86_64") {
        "alius-macos-x64.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "x86_64") {
        "alius-linux-x64.tar.gz"
    } else if cfg!(target_os = "linux") && cfg!(target_arch = "aarch64") {
        "alius-linux-arm64.tar.gz"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "x86_64") {
        "alius-windows-x64.zip"
    } else if cfg!(target_os = "windows") && cfg!(target_arch = "aarch64") {
        "alius-windows-arm64.zip"
    } else {
        "alius-linux-x64.tar.gz"
    }
}

/// Whether the current platform is Windows.
pub fn is_windows() -> bool {
    cfg!(target_os = "windows")
}

/// Path to the currently running binary.
pub fn current_binary_path() -> anyhow::Result<PathBuf> {
    std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("Cannot determine current binary path: {}", e))
}

/// Detect how the CLI was installed based on binary path.
pub fn detect_install_method() -> InstallMethod {
    let Ok(exe) = current_binary_path() else {
        return InstallMethod::Standalone;
    };

    let path_str = exe.to_string_lossy();

    if path_str.contains("/target/") || path_str.contains("\\target\\") {
        return InstallMethod::Development;
    }

    if path_str.contains("node_modules") {
        return InstallMethod::Npm;
    }

    if path_str.contains("/Cellar/") || path_str.contains("\\Cellar\\") {
        return InstallMethod::Homebrew;
    }

    InstallMethod::Standalone
}
