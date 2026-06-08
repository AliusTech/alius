//! Credential resolution with keyring and environment variable support.
//!
//! Supports two reference formats:
//! - `env:VAR_NAME` — read from environment variable
//! - `secure:KEY_NAME` — read from OS keyring via the `keyring` crate
//!
//! When the keyring is unavailable (no GUI session, headless CI), the provider
//! gracefully degrades to environment variable lookup with a warning.

use anyhow::{bail, Result};

/// Service name used for keyring entries.
const SERVICE_NAME: &str = "ai.alius.cli";

/// Resolve a credential reference to its actual value.
///
/// # Arguments
/// * `key_ref` — A credential reference in one of:
///   - `env:VAR_NAME` — read from environment variable `VAR_NAME`
///   - `secure:KEY_NAME` — read from OS keyring entry `KEY_NAME`
///   - Bare string — treated as the literal value (for backward compat)
///
/// # Errors
/// Returns an error if the referenced credential cannot be found.
pub fn resolve_secret(key_ref: &str) -> Result<String> {
    if let Some(var_name) = key_ref.strip_prefix("env:") {
        return std::env::var(var_name)
            .map_err(|_| anyhow::anyhow!("environment variable '{}' not set", var_name));
    }

    if let Some(key_name) = key_ref.strip_prefix("secure:") {
        return resolve_from_keyring(key_name);
    }

    // Bare string — return as-is for backward compatibility.
    Ok(key_ref.to_string())
}

/// Store a secret in the OS keyring.
pub fn store_secret(key_name: &str, value: &str) -> Result<()> {
    if !check_keyring_available() {
        bail!("keyring is not available in this environment");
    }
    let entry = keyring::Entry::new(SERVICE_NAME, key_name)?;
    entry.set_password(value)?;
    Ok(())
}

/// Delete a secret from the OS keyring.
pub fn delete_secret(key_name: &str) -> Result<()> {
    let entry = keyring::Entry::new(SERVICE_NAME, key_name)?;
    entry.delete_credential()?;
    Ok(())
}

/// Check whether the OS keyring is accessible.
///
/// Returns `false` in headless/CI environments where no keyring daemon is
/// running, or when the `keyring` crate fails to initialize.
pub fn check_keyring_available() -> bool {
    std::panic::catch_unwind(|| {
        keyring::Entry::new(SERVICE_NAME, "__alius_availability_check__").is_ok()
    })
    .unwrap_or(false)
}

/// Try to resolve from keyring, falling back to env on failure.
fn resolve_from_keyring(key_name: &str) -> Result<String> {
    match try_keyring_get(key_name) {
        Ok(Some(val)) => return Ok(val),
        Ok(None) => {
            eprintln!(
                "[alius] warning: keyring entry '{}' not found, falling back to env",
                key_name
            );
        }
        Err(e) => {
            eprintln!(
                "[alius] warning: keyring error for '{}': {}, falling back to env",
                key_name, e
            );
        }
    }

    // Fallback: try env variable with the same name.
    std::env::var(key_name)
        .map_err(|_| anyhow::anyhow!("credential '{}' not found in keyring or env", key_name))
}

/// Attempt to read from keyring, returning None if the entry does not exist.
fn try_keyring_get(key_name: &str) -> std::result::Result<Option<String>, String> {
    let entry = keyring::Entry::new(SERVICE_NAME, key_name).map_err(|e| e.to_string())?;
    match entry.get_password() {
        Ok(val) => Ok(Some(val)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_env_variable() {
        std::env::set_var("ALIUS_TEST_KEY_123", "test_value_abc");
        let val = resolve_secret("env:ALIUS_TEST_KEY_123").unwrap();
        assert_eq!(val, "test_value_abc");
        std::env::remove_var("ALIUS_TEST_KEY_123");
    }

    #[test]
    fn test_resolve_env_missing_returns_error() {
        let result = resolve_secret("env:ALIUS_NONEXISTENT_KEY_XYZ_999");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not set"));
    }

    #[test]
    fn test_resolve_bare_string_returns_as_is() {
        let val = resolve_secret("literal_value").unwrap();
        assert_eq!(val, "literal_value");
    }

    #[test]
    fn test_check_keyring_available_returns_bool() {
        // Should not panic — just returns true or false depending on environment.
        let _ = check_keyring_available();
    }

    #[test]
    fn test_invalid_secure_graceful_fallback() {
        // A secure: reference that doesn't exist should still produce a
        // meaningful error (either from keyring or env fallback).
        let result = resolve_secret("secure:ALIUS_DEFINITELY_NONEXISTENT_999");
        assert!(result.is_err());
    }
}
