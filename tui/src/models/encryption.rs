use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use base64::{Engine as _, engine::general_purpose};

/// Encryption metadata stored per user
/// Contains information needed for key derivation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptionMetadata {
    pub salt: String,        // Base64-encoded salt for PBKDF2
    pub iterations: i32,     // PBKDF2 iterations (minimum 100,000, default 256,000)
    pub created_at: DateTime<Utc>,
    pub algorithm: EncryptionAlgorithm,
}

/// Encryption algorithm identifier
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum EncryptionAlgorithm {
    #[serde(rename = "AES-256-GCM")]
    Aes256Gcm,
}

impl Default for EncryptionAlgorithm {
    fn default() -> Self {
        Self::Aes256Gcm
    }
}

impl std::fmt::Display for EncryptionAlgorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Aes256Gcm => write!(f, "AES-256-GCM"),
        }
    }
}

impl EncryptionMetadata {
    /// Create new encryption metadata with default iterations
    pub fn new(salt: String) -> Self {
        Self {
            salt,
            iterations: 256_000, // Match web app and SQLCipher
            created_at: Utc::now(),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
        }
    }

    /// Create encryption metadata with custom iterations
    pub fn with_iterations(salt: String, iterations: i32) -> Result<Self, String> {
        if iterations < 100_000 {
            return Err("Iterations must be at least 100,000".to_string());
        }

        Ok(Self {
            salt,
            iterations,
            created_at: Utc::now(),
            algorithm: EncryptionAlgorithm::Aes256Gcm,
        })
    }

    /// Validate encryption metadata
    pub fn validate(&self) -> Result<(), String> {
        if self.iterations < 100_000 {
            return Err("Iterations must be at least 100,000".to_string());
        }

        if self.salt.is_empty() {
            return Err("Salt cannot be empty".to_string());
        }

        // Validate salt is valid base64
        if general_purpose::STANDARD.decode(&self.salt).is_err() {
            return Err("Salt must be valid base64".to_string());
        }

        Ok(())
    }
}

/// Encrypted data structure
/// Used for storing encrypted content
/// Compatible with both TUI format (nonce+tag) and web app format (iv only)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EncryptedData {
    pub ciphertext: String,       // Base64-encoded encrypted data
    #[serde(alias = "iv")]
    pub nonce: String,            // Base64-encoded nonce/IV
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub tag: String,              // Base64-encoded authentication tag (for GCM, optional for web compat)
}

impl EncryptedData {
    /// Create new encrypted data
    pub fn new(ciphertext: String, nonce: String, tag: String) -> Self {
        Self {
            ciphertext,
            nonce,
            tag,
        }
    }

    /// Validate encrypted data
    pub fn validate(&self) -> Result<(), String> {
        if self.ciphertext.is_empty() {
            return Err("Ciphertext cannot be empty".to_string());
        }

        if self.nonce.is_empty() {
            return Err("Nonce cannot be empty".to_string());
        }

        // Tag is optional for web app compatibility (Web Crypto API includes tag in ciphertext)

        // Validate all fields are valid base64
        if general_purpose::STANDARD.decode(&self.ciphertext).is_err() {
            return Err("Ciphertext must be valid base64".to_string());
        }

        if general_purpose::STANDARD.decode(&self.nonce).is_err() {
            return Err("Nonce must be valid base64".to_string());
        }

        if !self.tag.is_empty() && general_purpose::STANDARD.decode(&self.tag).is_err() {
            return Err("Tag must be valid base64".to_string());
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_metadata_creation() {
        let salt = general_purpose::STANDARD.encode(b"test_salt_16byte");
        let metadata = EncryptionMetadata::new(salt.clone());
        assert_eq!(metadata.salt, salt);
        assert_eq!(metadata.iterations, 256_000);
        assert_eq!(metadata.algorithm, EncryptionAlgorithm::Aes256Gcm);
    }

    #[test]
    fn test_encryption_metadata_validation() {
        let salt = general_purpose::STANDARD.encode(b"test_salt");
        let metadata = EncryptionMetadata::new(salt);
        assert!(metadata.validate().is_ok());

        // Invalid iterations
        let mut bad_metadata = metadata.clone();
        bad_metadata.iterations = 50_000;
        assert!(bad_metadata.validate().is_err());

        // Invalid salt (not base64)
        let mut bad_metadata = metadata.clone();
        bad_metadata.salt = "not valid base64!!!".to_string();
        assert!(bad_metadata.validate().is_err());
    }

    #[test]
    fn test_encrypted_data() {
        let data = EncryptedData::new(
            general_purpose::STANDARD.encode(b"ciphertext"),
            general_purpose::STANDARD.encode(b"nonce_12byte"),
            general_purpose::STANDARD.encode(b"tag_16_bytes!!!"),
        );
        assert!(data.validate().is_ok());

        // Invalid base64
        let bad_data = EncryptedData::new(
            "not base64!!!".to_string(),
            general_purpose::STANDARD.encode(b"nonce"),
            general_purpose::STANDARD.encode(b"tag"),
        );
        assert!(bad_data.validate().is_err());
    }
}
