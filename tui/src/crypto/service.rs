use aes_gcm::{
    aead::{Aead, AeadCore, KeyInit, OsRng, rand_core::RngCore},
    Aes256Gcm, Nonce,
};
use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256};

use crate::models::encryption::EncryptedData;

// Constants matching web app
const KEY_LENGTH: usize = 32; // 256 bits
const NONCE_LENGTH: usize = 12; // 96 bits for GCM
const SALT_LENGTH: usize = 32; // 256 bits
const DEFAULT_ITERATIONS: u32 = 100_000; // Match web app

type NonceType = [u8; NONCE_LENGTH];

/// Cryptography service for encryption/decryption operations
pub struct CryptoService;

impl CryptoService {
    /// Create a new crypto service instance
    pub fn new() -> Self {
        Self
    }

    /// Derive a 256-bit AES key from password using PBKDF2
    ///
    /// # Arguments
    /// * `password` - User password
    /// * `salt` - Random salt (32 bytes)
    /// * `iterations` - PBKDF2 iterations (default: 100,000)
    pub fn derive_key(
        &self,
        password: &str,
        salt: &[u8],
        iterations: u32,
    ) -> Result<[u8; KEY_LENGTH]> {
        if salt.len() < SALT_LENGTH {
            anyhow::bail!("Salt must be at least {} bytes", SALT_LENGTH);
        }

        let iterations = if iterations < 100_000 {
            DEFAULT_ITERATIONS
        } else {
            iterations
        };

        let mut key = [0u8; KEY_LENGTH];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, iterations, &mut key);

        Ok(key)
    }

    /// Encrypt text data
    ///
    /// # Arguments
    /// * `plaintext` - Text to encrypt
    /// * `key` - 256-bit AES key
    pub fn encrypt_text(&self, plaintext: &str, key: &[u8; KEY_LENGTH]) -> Result<EncryptedData> {
        let cipher = Aes256Gcm::new(key.into());
        let nonce = self.generate_nonce();
        let nonce_ref = Nonce::from_slice(&nonce);

        let ciphertext = cipher
            .encrypt(nonce_ref, plaintext.as_bytes())
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        Ok(EncryptedData {
            ciphertext: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(&nonce),
            tag: String::new(), // GCM includes tag in ciphertext
        })
    }

    /// Decrypt text data
    ///
    /// # Arguments
    /// * `encrypted` - Encrypted data
    /// * `key` - 256-bit AES key
    pub fn decrypt_text(&self, encrypted: &EncryptedData, key: &[u8; KEY_LENGTH]) -> Result<String> {
        let cipher = Aes256Gcm::new(key.into());

        let ciphertext = general_purpose::STANDARD
            .decode(&encrypted.ciphertext)
            .context("Invalid base64 ciphertext")?;

        let nonce_bytes = general_purpose::STANDARD
            .decode(&encrypted.nonce)
            .context("Invalid base64 nonce")?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("Decryption failed. Invalid key or corrupted data."))?;

        String::from_utf8(plaintext).context("Decrypted data is not valid UTF-8")
    }

    /// Encrypt binary data (for attachments)
    ///
    /// # Arguments
    /// * `data` - Binary data to encrypt
    /// * `key` - 256-bit AES key
    pub fn encrypt_binary(&self, data: &[u8], key: &[u8; KEY_LENGTH]) -> Result<EncryptedData> {
        let cipher = Aes256Gcm::new(key.into());
        let nonce = self.generate_nonce();
        let nonce_ref = Nonce::from_slice(&nonce);

        let ciphertext = cipher
            .encrypt(nonce_ref, data)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        Ok(EncryptedData {
            ciphertext: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(&nonce),
            tag: String::new(),
        })
    }

    /// Decrypt binary data (for attachments)
    ///
    /// # Arguments
    /// * `encrypted` - Encrypted data
    /// * `key` - 256-bit AES key
    pub fn decrypt_binary(&self, encrypted: &EncryptedData, key: &[u8; KEY_LENGTH]) -> Result<Vec<u8>> {
        let cipher = Aes256Gcm::new(key.into());

        let ciphertext = general_purpose::STANDARD
            .decode(&encrypted.ciphertext)
            .context("Invalid base64 ciphertext")?;

        let nonce_bytes = general_purpose::STANDARD
            .decode(&encrypted.nonce)
            .context("Invalid base64 nonce")?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|_| anyhow::anyhow!("Decryption failed. Invalid key or corrupted data."))?;

        Ok(plaintext)
    }

    /// Generate random salt for key derivation (32 bytes)
    pub fn generate_salt(&self) -> Vec<u8> {
        let mut salt = vec![0u8; SALT_LENGTH];
        OsRng.fill_bytes(&mut salt);
        salt
    }

    /// Generate initialization vector/nonce for encryption (12 bytes)
    fn generate_nonce(&self) -> NonceType {
        Aes256Gcm::generate_nonce(&mut OsRng).into()
    }

    /// Hash data using SHA-256 (for sync conflict detection)
    pub fn hash(&self, data: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(data.as_bytes());
        let result = hasher.finalize();
        general_purpose::STANDARD.encode(result)
    }

    /// Encrypt JSON data (helper)
    pub fn encrypt_json<T: serde::Serialize>(
        &self,
        data: &T,
        key: &[u8; KEY_LENGTH],
    ) -> Result<EncryptedData> {
        let json = serde_json::to_string(data).context("JSON serialization failed")?;
        self.encrypt_text(&json, key)
    }

    /// Decrypt JSON data (helper)
    pub fn decrypt_json<T: serde::de::DeserializeOwned>(
        &self,
        encrypted: &EncryptedData,
        key: &[u8; KEY_LENGTH],
    ) -> Result<T> {
        let json = self.decrypt_text(encrypted, key)?;
        serde_json::from_str(&json).context("JSON deserialization failed")
    }
}

impl Default for CryptoService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_derivation() {
        let service = CryptoService::new();
        let password = "test_password";
        let salt = service.generate_salt();

        let key1 = service.derive_key(password, &salt, 100_000).unwrap();
        let key2 = service.derive_key(password, &salt, 100_000).unwrap();

        // Same password and salt should produce same key
        assert_eq!(key1, key2);

        // Different password should produce different key
        let key3 = service.derive_key("different_password", &salt, 100_000).unwrap();
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_text_encryption_decryption() {
        let service = CryptoService::new();
        let password = "test_password";
        let salt = service.generate_salt();
        let key = service.derive_key(password, &salt, 100_000).unwrap();

        let plaintext = "Hello, World! This is a test message.";
        let encrypted = service.encrypt_text(plaintext, &key).unwrap();
        let decrypted = service.decrypt_text(&encrypted, &key).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_binary_encryption_decryption() {
        let service = CryptoService::new();
        let password = "test_password";
        let salt = service.generate_salt();
        let key = service.derive_key(password, &salt, 100_000).unwrap();

        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let encrypted = service.encrypt_binary(&data, &key).unwrap();
        let decrypted = service.decrypt_binary(&encrypted, &key).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_wrong_key_decryption_fails() {
        let service = CryptoService::new();
        let salt = service.generate_salt();
        let key1 = service.derive_key("password1", &salt, 100_000).unwrap();
        let key2 = service.derive_key("password2", &salt, 100_000).unwrap();

        let plaintext = "Secret message";
        let encrypted = service.encrypt_text(plaintext, &key1).unwrap();

        // Should fail with wrong key
        assert!(service.decrypt_text(&encrypted, &key2).is_err());
    }

    #[test]
    fn test_json_encryption_decryption() {
        use serde::{Deserialize, Serialize};

        #[derive(Serialize, Deserialize, PartialEq, Debug)]
        struct TestData {
            name: String,
            value: i32,
            tags: Vec<String>,
        }

        let service = CryptoService::new();
        let password = "test_password";
        let salt = service.generate_salt();
        let key = service.derive_key(password, &salt, 100_000).unwrap();

        let data = TestData {
            name: "Test".to_string(),
            value: 42,
            tags: vec!["tag1".to_string(), "tag2".to_string()],
        };

        let encrypted = service.encrypt_json(&data, &key).unwrap();
        let decrypted: TestData = service.decrypt_json(&encrypted, &key).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_hash() {
        let service = CryptoService::new();
        let data = "test data";

        let hash1 = service.hash(data);
        let hash2 = service.hash(data);

        // Same data should produce same hash
        assert_eq!(hash1, hash2);

        // Different data should produce different hash
        let hash3 = service.hash("different data");
        assert_ne!(hash1, hash3);

        // Hash should be base64
        assert!(general_purpose::STANDARD.decode(&hash1).is_ok());
    }
}
