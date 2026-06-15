use std::fs;
use std::io::{self, Read as _, Write as _};

use super::platform::{current_binary_path, is_windows};

/// Download an archive from `url` and install the binary, atomically replacing
/// the current executable.
pub async fn download_and_install(url: &str, asset_name: &str) -> anyhow::Result<()> {
    let current_exe = current_binary_path()?;

    // Download archive to a temp file in the same directory (same filesystem
    // guarantees atomic rename on Unix).
    let exe_dir = current_exe
        .parent()
        .ok_or_else(|| anyhow::anyhow!("Cannot determine binary directory"))?;

    let archive_data = download_bytes(url).await?;
    let extracted = extract_binary(&archive_data, asset_name)?;

    // Write extracted binary to a temp file in the same directory.
    let tmp_path = exe_dir.join(format!(".alius-update-{}", std::process::id()));
    {
        let mut f = fs::File::create(&tmp_path)?;
        f.write_all(&extracted)?;
    }

    // Set executable permission on Unix.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&tmp_path, fs::Permissions::from_mode(0o755))?;
    }

    // Compute .old path: append ".old" to the binary name.
    let old_path = if is_windows() {
        let mut p = current_exe.clone();
        p.set_extension("old");
        p
    } else {
        let exe_name = current_exe
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_else(|| "alius".to_string());
        current_exe.with_file_name(format!("{}.old", exe_name))
    };

    // Clean up any previous .old file.
    let _ = fs::remove_file(&old_path);

    // Atomic swap: rename old -> .old, rename new -> old.
    if let Err(e) = fs::rename(&current_exe, &old_path) {
        let _ = fs::remove_file(&tmp_path);
        if e.kind() == io::ErrorKind::PermissionDenied {
            anyhow::bail!(
                "Permission denied replacing {}. The binary may be in use. \
                 Stop all alius instances and try again, or update via your package manager.",
                current_exe.display()
            );
        }
        anyhow::bail!("Failed to replace binary: {}", e);
    }

    if let Err(e) = fs::rename(&tmp_path, &current_exe) {
        let _ = fs::rename(&old_path, &current_exe);
        anyhow::bail!("Failed to install new binary: {}", e);
    }

    // Best-effort cleanup of the .old file.
    let _ = fs::remove_file(&old_path);

    Ok(())
}

async fn download_bytes(url: &str) -> anyhow::Result<Vec<u8>> {
    let client = reqwest::Client::builder()
        .user_agent(super::user_agent())
        .build()?;
    let resp = client.get(url).send().await?;

    if !resp.status().is_success() {
        anyhow::bail!("Download failed (HTTP {})", resp.status());
    }

    let bytes = resp.bytes().await?;
    Ok(bytes.to_vec())
}

/// Extract the `alius` binary from a tar.gz or zip archive.
fn extract_binary(archive: &[u8], asset_name: &str) -> anyhow::Result<Vec<u8>> {
    if asset_name.ends_with(".tar.gz") || asset_name.ends_with(".tgz") {
        extract_tar_gz(archive)
    } else if asset_name.ends_with(".zip") {
        extract_zip(archive)
    } else {
        anyhow::bail!("Unsupported archive format: {}", asset_name);
    }
}

fn extract_tar_gz(archive: &[u8]) -> anyhow::Result<Vec<u8>> {
    let decoder = flate2::read::GzDecoder::new(archive);
    let mut archive = tar::Archive::new(decoder);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let name = path
            .file_name()
            .map(|s| s.to_string_lossy().into_owned())
            .unwrap_or_default();

        if name == "alius" || name == "alius.exe" {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }

    anyhow::bail!("Could not find 'alius' binary in tar.gz archive")
}

fn extract_zip(archive: &[u8]) -> anyhow::Result<Vec<u8>> {
    let reader = io::Cursor::new(archive);
    let mut archive = zip::ZipArchive::new(reader)?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().rsplit('/').next().unwrap_or("").to_string();

        if name == "alius" || name == "alius.exe" {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf)?;
            return Ok(buf);
        }
    }

    anyhow::bail!("Could not find 'alius' binary in zip archive")
}
