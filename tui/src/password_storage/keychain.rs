//! OS keychain password storage backend
//!
//! This uses the system keychain for secure password storage:
//! - macOS: Keychain (requires codesigned app for reliable persistence)
//! - Windows: Credential Manager
//! - Linux: Secret Service (via D-Bus)
//!
//! NOTE: On macOS, unsigned apps have issues with keychain access controls.
//! The keychain restricts access to the binary that created the entry, so
//! rebuilding the app can cause access failures. For unsigned apps on macOS,
//! file-based storage is used instead.

use anyhow::{Context, Result};
use keyring::Entry;

use super::{PasswordStorage, RetrieveResult, StorageBackendType};

/// Service name for keychain entries
const SERVICE_NAME: &str = "jottery-tui";
/// Username/account for keychain entries
const ACCOUNT_NAME: &str = "database-password";

/// Keychain-based password storage
///
/// Uses the OS keychain for secure password storage.
pub struct KeychainStorage {
    entry: Entry,
}

impl KeychainStorage {
    /// Create a new keychain storage backend
    ///
    /// Returns an error if the keychain is not available or not functional on this system.
    pub fn new() -> Result<Self> {
        // On macOS, unsigned apps can't reliably use keychain across app rebuilds
        // due to access control list (ACL) restrictions. Fall back to file storage.
        #[cfg(target_os = "macos")]
        {
            // Check if the app appears to be codesigned
            if !Self::is_likely_codesigned() {
                return Err(anyhow::anyhow!(
                    "Keychain not available for unsigned macOS apps (ACL restrictions). \
                     Codesign the app for keychain support."
                ));
            }
        }

        let entry = Entry::new(SERVICE_NAME, ACCOUNT_NAME)
            .context("Failed to create keychain entry")?;

        // Test that we can actually use the keychain
        match entry.get_password() {
            Ok(_) => {} // Password exists, keychain works
            Err(keyring::Error::NoEntry) => {} // No password yet, but keychain works
            Err(e) => {
                return Err(anyhow::anyhow!(
                    "Keychain access test failed: {}",
                    e
                ));
            }
        }

        Ok(Self { entry })
    }

    /// Check if the app is properly codesigned (not ad-hoc) for macOS keychain access
    ///
    /// Ad-hoc signatures (created automatically by the linker) don't provide stable
    /// identity for keychain access control lists. When an app is rebuilt, the ad-hoc
    /// signature changes, causing keychain access failures.
    ///
    /// This function returns true only for:
    /// 1. Apps running from an .app bundle (assumed to be properly signed for distribution)
    /// 2. Apps with a proper Developer ID signature (has TeamIdentifier)
    #[cfg(target_os = "macos")]
    fn is_likely_codesigned() -> bool {
        if let Ok(exe_path) = std::env::current_exe() {
            // If running from an .app bundle, assume properly signed for distribution
            let path_str = exe_path.to_string_lossy();
            if path_str.contains(".app/Contents/MacOS/") {
                return true;
            }

            // Check if the binary has a proper signature (not ad-hoc)
            // A properly signed binary will have a TeamIdentifier
            if let Ok(output) = std::process::Command::new("codesign")
                .args(["-dvv"])
                .arg(&exe_path)
                .stderr(std::process::Stdio::piped())
                .output()
            {
                let stderr = String::from_utf8_lossy(&output.stderr);
                // Check for adhoc signature (linker-signed binaries)
                if stderr.contains("adhoc") || stderr.contains("Signature=adhoc") {
                    tracing::debug!("Detected ad-hoc signature, keychain not reliable");
                    return false;
                }
                // Check for TeamIdentifier - properly signed apps have this
                if stderr.contains("TeamIdentifier=") && !stderr.contains("TeamIdentifier=not set")
                {
                    return true;
                }
            }
        }
        false
    }
}

impl PasswordStorage for KeychainStorage {
    fn store(&self, password: &str) -> Result<()> {
        self.entry
            .set_password(password)
            .context("Failed to store password in keychain")?;
        Ok(())
    }

    fn retrieve(&self) -> RetrieveResult {
        match self.entry.get_password() {
            Ok(password) => RetrieveResult::Found(password),
            Err(keyring::Error::NoEntry) => RetrieveResult::NotFound,
            Err(e) => RetrieveResult::Error(anyhow::anyhow!("Keychain error: {}", e)),
        }
    }

    fn delete(&self) -> Result<()> {
        match self.entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()), // Already deleted, that's fine
            Err(e) => Err(anyhow::anyhow!("Failed to delete from keychain: {}", e)),
        }
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::Keychain
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: These tests require a working keychain on the system
    // They may fail in CI environments without keychain access

    #[test]
    #[ignore] // Run manually: cargo test --features keychain -- --ignored
    fn test_keychain_round_trip() {
        let storage = KeychainStorage::new().expect("Keychain should be available");

        // Clean up any previous test data
        let _ = storage.delete();

        // Initially no password
        assert!(matches!(storage.retrieve(), RetrieveResult::NotFound));

        // Store password
        storage.store("test_keychain_password").unwrap();

        // Retrieve password
        match storage.retrieve() {
            RetrieveResult::Found(password) => assert_eq!(password, "test_keychain_password"),
            other => panic!("Expected Found, got {:?}", other),
        }

        // Delete password
        storage.delete().unwrap();

        // Should be gone
        assert!(matches!(storage.retrieve(), RetrieveResult::NotFound));
    }

    #[test]
    #[ignore]
    fn test_keychain_backend_type() {
        let storage = KeychainStorage::new().expect("Keychain should be available");
        assert_eq!(storage.backend_type(), StorageBackendType::Keychain);
        assert!(storage.backend_type().is_secure());
    }
}
