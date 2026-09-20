//! File-based password storage backend
//!
//! This stores the password in a file encrypted with a per-installation key
//! that lives in a sibling file. Both files are created with owner-only
//! permissions on Unix.
//!
//! WARNING: This is NOT cryptographically secure - it's obfuscation only.
//! Anyone who can read both files can decrypt the stored password. It merely
//! prevents casual snooping and means a copied remember file is useless on
//! its own. This backend is used as a fallback when the OS keychain is
//! unavailable.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use super::{PasswordStorage, RetrieveResult, StorageBackendType};

/// The filename for the stored password
const REMEMBER_FILE: &str = ".jottery_remember";

/// The filename for the per-installation obfuscation key
const KEY_FILE: &str = ".jottery_remember.key";

/// Length of the obfuscation key in bytes (AES-256)
const KEY_LENGTH: usize = 32;

/// File-based password storage
///
/// Stores passwords in a file using AES-256-GCM with a random key generated
/// on first use and kept alongside the password file.
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

    /// Get the path to the obfuscation key file
    fn key_file_path(&self) -> PathBuf {
        self.config_dir.join(KEY_FILE)
    }

    /// Write a file readable and writable only by the owner (on Unix)
    fn write_private(path: &Path, contents: &[u8]) -> Result<()> {
        #[cfg(unix)]
        {
            use std::io::Write;
            use std::os::unix::fs::OpenOptionsExt;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600) // Owner read/write only
                .open(path)
                .with_context(|| format!("Failed to create {}", path.display()))?;
            file.write_all(contents)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }

        #[cfg(not(unix))]
        {
            std::fs::write(path, contents)
                .with_context(|| format!("Failed to write {}", path.display()))?;
        }

        Ok(())
    }

    /// Load the obfuscation key if a key file exists
    fn load_key(&self) -> Result<Option<[u8; KEY_LENGTH]>> {
        let key_file = self.key_file_path();
        if !key_file.exists() {
            return Ok(None);
        }

        let bytes = std::fs::read(&key_file).context("Failed to read password storage key file")?;
        let key = <[u8; KEY_LENGTH]>::try_from(bytes.as_slice()).map_err(|_| {
            anyhow::anyhow!(
                "Password storage key file has invalid length: {} (expected {})",
                bytes.len(),
                KEY_LENGTH
            )
        })?;
        Ok(Some(key))
    }

    /// Load the obfuscation key, generating and persisting a new one if absent
    fn load_or_create_key(&self) -> Result<[u8; KEY_LENGTH]> {
        if let Some(key) = self.load_key()? {
            return Ok(key);
        }

        let key = crate::crypto::CryptoService::new().generate_master_key();
        Self::write_private(&self.key_file_path(), &key)?;
        Ok(key)
    }
}

impl PasswordStorage for FileStorage {
    fn store(&self, password: &str) -> Result<()> {
        use crate::crypto::CryptoService;

        let crypto = CryptoService::new();
        let key = self.load_or_create_key()?;

        // Encrypt the password
        let encrypted = crypto.encrypt_text(password, &key)?;
        let encrypted_json = serde_json::to_string(&encrypted)?;

        Self::write_private(&self.remember_file_path(), encrypted_json.as_bytes())
            .context("Failed to write password storage file")
    }

    fn retrieve(&self) -> RetrieveResult {
        use crate::crypto::{CryptoService, EncryptedData};

        let remember_file = self.remember_file_path();

        if !remember_file.exists() {
            return RetrieveResult::NotFound;
        }

        // Load the key. A remember file without a key file was written by an
        // older version using a fixed key; it cannot be read any more, so
        // discard it and let the user be prompted again.
        let key = match self.load_key() {
            Ok(Some(key)) => key,
            Ok(None) => {
                tracing::warn!(
                    "Removing remembered password written by an older version; you will be asked for your password again"
                );
                return match std::fs::remove_file(&remember_file) {
                    Ok(()) => RetrieveResult::NotFound,
                    Err(e) => RetrieveResult::Error(e.into()),
                };
            }
            Err(e) => return RetrieveResult::Error(e),
        };

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
        match crypto.decrypt_text(&encrypted_data, &key) {
            Ok(password) => RetrieveResult::Found(password),
            Err(e) => RetrieveResult::Error(e),
        }
    }

    fn delete(&self) -> Result<()> {
        for path in [self.remember_file_path(), self.key_file_path()] {
            if path.exists() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("Failed to delete {}", path.display()))?;
            }
        }

        Ok(())
    }

    fn backend_type(&self) -> StorageBackendType {
        StorageBackendType::File
    }
}
