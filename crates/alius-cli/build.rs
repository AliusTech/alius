//! Build script to embed version from .version file

use std::env;
use std::fs;
use std::path::Path;

fn main() {
    // Read version from .version file
    let version_path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .join(".version");

    let version = if version_path.exists() {
        fs::read_to_string(&version_path)
            .expect("Failed to read .version file")
            .trim()
            .to_string()
    } else {
        // Fallback to CARGO_PKG_VERSION
        env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "0.0.0".to_string())
    };

    // Set the version as an environment variable
    println!("cargo:rustc-env=ALIUS_VERSION={}", version);

    // Rerun if .version changes
    println!("cargo:rerun-if-changed={}", version_path.display());
}