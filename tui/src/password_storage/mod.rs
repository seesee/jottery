//! Password storage abstraction for the "Remember Password" feature
//!
//! This module provides a trait-based abstraction for storing passwords,
//! with implementations for OS keychain (secure) and file-based storage (fallback).

mod file_backend;

#[cfg(feature = "keychain")]
mod keychain;

use anyhow::Result;
use std::path::Path;

pub use file_backend::FileStorage;
#[cfg(feature = "keychain")]
pub use keychain::KeychainStorage;

/// The type of storage backend being used
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)]
pub enum StorageBackendType {
    /// OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service)
    /// This is cryptographically secure.
    Keychain,
    /// File-based storage with obfuscation
    /// WARNING: This is NOT cryptographically secure - the key is hardcoded.
    /// Anyone with access to the source code can decrypt the stored password.
    File,
}

impl StorageBackendType {
    /// Returns true if this backend provides secure storage
    pub fn is_secure(&self) -> bool {
        matches!(self, StorageBackendType::Keychain)
    }
}

/// Result of attempting to retrieve a stored password
#[derive(Debug)]
pub enum RetrieveResult {
    /// Password was successfully retrieved
    Found(String),
    /// No password is stored
    NotFound,
    /// An error occurred during retrieval
    Error(anyhow::Error),
}

/// Trait for password storage backends
pub trait PasswordStorage: Send + Sync {
    /// Store a password
    fn store(&self, password: &str) -> Result<()>;

    /// Retrieve the stored password
    fn retrieve(&self) -> RetrieveResult;

    /// Delete the stored password
    fn delete(&self) -> Result<()>;

    /// Get the type of this storage backend
    fn backend_type(&self) -> StorageBackendType;
}

/// Create the best available password storage backend
///
/// This function tries to create a keychain-based storage first (if the feature is enabled),
/// and falls back to file-based storage if keychain is unavailable.
///
/// IMPORTANT: If a file-based password already exists (e.g., from store-password CLI),
/// we use file storage to honor that pre-stored password. This ensures demo/automation
/// scenarios work correctly even when keychain is unavailable in headless contexts.
///
/// If `migrate` is true and keychain is available AND no file password exists,
/// it will migrate any existing keychain password to file (not currently used).
#[allow(unused_variables)]
pub fn create_storage(config_dir: &Path, migrate: bool) -> Box<dyn PasswordStorage> {
    // First, check if a file-based password exists
    // If so, use file storage to honor pre-stored passwords from store-password CLI
    let file_storage = FileStorage::new(config_dir.to_path_buf());
    if let RetrieveResult::Found(_) = file_storage.retrieve() {
        tracing::info!("Found existing file-based password, using file storage backend");
        return Box::new(file_storage);
    }

    #[cfg(feature = "keychain")]
    {
        tracing::debug!("Attempting to create keychain storage backend");
        // Try to create keychain storage
        match KeychainStorage::new() {
            Ok(keychain) => {
                tracing::info!("Keychain storage backend available");
                return Box::new(keychain);
            }
            Err(e) => {
                tracing::warn!("Keychain unavailable, falling back to file storage: {}", e);
            }
        }
    }

    // Fall back to file-based storage
    tracing::info!("Using file-based password storage backend");
    Box::new(file_storage)
}

/// Create file-based storage only (for CLI usage where we might not have keychain access)
pub fn create_file_storage(config_dir: &Path) -> Box<dyn PasswordStorage> {
    Box::new(FileStorage::new(config_dir.to_path_buf()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    /// Get test password from environment variable or use default.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        std::env::var("JOTTERY_TEST_PASSWORD")
            .unwrap_or_else(|_| "test_password_placeholder".to_string())
    }

    #[test]
    fn test_file_storage_round_trip() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path().to_path_buf());
        let password = test_password();

        // Initially no password
        assert!(matches!(storage.retrieve(), RetrieveResult::NotFound));

        // Store password
        storage.store(&password).unwrap();

        // Retrieve password
        match storage.retrieve() {
            RetrieveResult::Found(retrieved) => assert_eq!(retrieved, password),
            other => panic!("Expected Found, got {:?}", other),
        }

        // Delete password
        storage.delete().unwrap();

        // Should be gone
        assert!(matches!(storage.retrieve(), RetrieveResult::NotFound));
    }

    #[test]
    fn test_backend_type() {
        let temp_dir = TempDir::new().unwrap();
        let storage = FileStorage::new(temp_dir.path().to_path_buf());
        assert_eq!(storage.backend_type(), StorageBackendType::File);
        assert!(!storage.backend_type().is_secure());
    }
}
