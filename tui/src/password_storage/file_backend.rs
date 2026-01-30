//! File-based password storage backend
//!
//! This stores the password in a file encrypted with a hardcoded key.
//! WARNING: This is NOT cryptographically secure - it's obfuscation only.
//! Anyone with access to the source code can decrypt the stored password.
//! This backend is used as a fallback when OS keychain is unavailable.

use anyhow::{Context, Result};
use std::path::PathBuf;

use super::{PasswordStorage, RetrieveResult, StorageBackendType};

/// The filename for the stored password
const REMEMBER_FILE: &str = ".jottery_remember";

/// Hardcoded key for obfuscation (NOT secure!)
/// This key is intentionally visible - it provides no real security,
/// just prevents casual access to the password.
const OBFUSCATION_KEY: &[u8; 32] = b"jottery-tui-device-key-v1.0.0---";

/// File-based password storage
///
/// Stores passwords in a file using AES-256-GCM encryption with a hardcoded key.
/// This is NOT secure - it's only obfuscation to prevent casual snooping.
pub struct FileStorage {
    config_dir: PathBuf,
}

impl FileStorage {
    /// Create a new file storage backend
    pub fn new(config_dir: PathBuf) -> Self {
        Self { config_dir }
    }

    /// Get the path to the remember file
    fn remember_file_path(&self) -> PathBuf {
        self.config_dir.join(REMEMBER_FILE)
    }

    /// Get the obfuscation key
    fn get_key(&self) -> [u8; 32] {
        *OBFUSCATION_KEY
    }
}

impl PasswordStorage for FileStorage {
    fn store(&self, password: &str) -> Result<()> {
        use crate::crypto::CryptoService;

        let crypto = CryptoService::new();
        let key = self.get_key();

        // Encrypt the password
        let encrypted = crypto.encrypt_text(password, &key)?;
        let encrypted_json = serde_json::to_string(&encrypted)?;

        // Write to file
        let remember_file = self.remember_file_path();
        std::fs::write(&remember_file, &encrypted_json)
            .context("Failed to write password storage file")?;

        Ok(())
    }

    fn retrieve(&self) -> RetrieveResult {
        use crate::crypto::{CryptoService, EncryptedData};

        let remember_file = self.remember_file_path();

        if !remember_file.exists() {
            return RetrieveResult::NotFound;
        }

        // Read encrypted password
        let encrypted_password = match std::fs::read_to_string(&remember_file) {
            Ok(content) => content,
            Err(e) => return RetrieveResult::Error(e.into()),
        };

        if encrypted_password.trim().is_empty() {
            return RetrieveResult::NotFound;
        }

        // Parse encrypted data
        let encrypted_data: EncryptedData = match serde_json::from_str(&encrypted_password) {
            Ok(data) => data,
            Err(e) => return RetrieveResult::Error(e.into()),
        };

        // Decrypt
        let crypto = CryptoService::new();
        let key = self.get_key();

        match crypto.decrypt_text(&encrypted_data, &key) {
            Ok(password) => RetrieveResult::Found(password),
            Err(e) => RetrieveResult::Error(e),
        }
    }

    fn delete(&self) -> Result<()> {
        let remember_file = self.remember_file_path();

        if remember_file.exists() {
            std::fs::remove_file(&remember_file)
                .context("Failed to delete password storage file")?;
        }

        Ok(())
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::File
    }
}
