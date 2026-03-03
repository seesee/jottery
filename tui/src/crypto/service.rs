//! Cryptographic service for encryption and key derivation
#![allow(dead_code)]

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
/// Default PBKDF2 iterations for new databases (OWASP 2023 recommendation: 600,000+)
pub const DEFAULT_ITERATIONS: u32 = 600_000;
/// Minimum iterations accepted (for backwards compatibility with existing databases)
const MIN_ITERATIONS: u32 = 100_000;
/// PBKDF2 iterations for envelope wrapping key derivation (onboarding/migration only)
pub const WRAPPING_ITERATIONS: u32 = 1_000_000;
/// KDF version for envelope encryption (1=PBKDF2, 2=Argon2id future)
pub const WRAPPING_KDF_VERSION: u32 = 1;
/// Maximum hash chain length (matching web client)
pub const MAX_HASH_CHAIN_LENGTH: usize = 50;

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
    /// * `iterations` - PBKDF2 iterations (minimum 100,000 for backwards compatibility)
    pub fn derive_key(
        &self,
        password: &str,
        salt: &[u8],
        iterations: u32,
    ) -> Result<[u8; KEY_LENGTH]> {
        if salt.len() < SALT_LENGTH {
            anyhow::bail!("Salt must be at least {} bytes", SALT_LENGTH);
        }

        // Enforce minimum iterations for security, but accept 100k for existing databases
        let iterations = if iterations < MIN_ITERATIONS {
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
            nonce: general_purpose::STANDARD.encode(nonce),
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
    #[allow(dead_code)]
    pub fn encrypt_binary(&self, data: &[u8], key: &[u8; KEY_LENGTH]) -> Result<EncryptedData> {
        let cipher = Aes256Gcm::new(key.into());
        let nonce = self.generate_nonce();
        let nonce_ref = Nonce::from_slice(&nonce);

        let ciphertext = cipher
            .encrypt(nonce_ref, data)
            .map_err(|_| anyhow::anyhow!("Encryption failed"))?;

        Ok(EncryptedData {
            ciphertext: general_purpose::STANDARD.encode(&ciphertext),
            nonce: general_purpose::STANDARD.encode(nonce),
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

    /// Compute content hash for a note (for git-like conflict detection)
    /// Matches web client implementation in src/lib/services/crypto.ts
    pub fn compute_content_hash(&self, note: &crate::models::Note) -> String {
        use serde_json::json;

        // Get sorted attachment IDs for consistent hashing
        let mut attachment_ids: Vec<&str> = note.attachments.iter()
            .map(|a| a.id.as_str())
            .collect();
        attachment_ids.sort();

        // Build hash data matching web client structure
        let hash_data = json!({
            "content": note.content,
            "tags": note.tags,
            "attachments": attachment_ids,
            "pinned": note.pinned,
            "syntaxLanguage": note.syntax_language.to_string(),
            "wordWrap": note.word_wrap,
        });

        self.hash(&hash_data.to_string())
    }

    // === Envelope Encryption Methods ===

    /// Generate a random 256-bit master key
    pub fn generate_master_key(&self) -> [u8; KEY_LENGTH] {
        let mut key = [0u8; KEY_LENGTH];
        OsRng.fill_bytes(&mut key);
        key
    }

    /// Derive wrapping key from password + userId (used during onboarding/migration only)
    ///
    /// Uses PBKDF2 with 1M iterations and the userId string as salt.
    /// This is intentionally slow — only runs during onboarding, not daily unlock.
    pub fn derive_wrapping_key(
        &self,
        password: &str,
        user_id: &str,
    ) -> Result<[u8; KEY_LENGTH]> {
        let salt = user_id.as_bytes();
        if salt.len() < KEY_LENGTH {
            anyhow::bail!("userId must be at least {} bytes when used as salt", KEY_LENGTH);
        }
        let mut key = [0u8; KEY_LENGTH];
        pbkdf2_hmac::<Sha256>(password.as_bytes(), salt, WRAPPING_ITERATIONS, &mut key);
        Ok(key)
    }

    /// Wrap (encrypt) master key bytes with a wrapping key using AES-256-GCM
    pub fn wrap_master_key(
        &self,
        wrapping_key: &[u8; KEY_LENGTH],
        master_key_bytes: &[u8],
    ) -> Result<EncryptedData> {
        self.encrypt_binary(master_key_bytes, wrapping_key)
    }

    /// Unwrap (decrypt) master key bytes from a wrapped blob
    pub fn unwrap_master_key(
        &self,
        wrapping_key: &[u8; KEY_LENGTH],
        wrapped: &EncryptedData,
    ) -> Result<[u8; KEY_LENGTH]> {
        let bytes = self.decrypt_binary(wrapped, wrapping_key)?;
        if bytes.len() != KEY_LENGTH {
            anyhow::bail!("Unwrapped master key has invalid length: {} (expected {})", bytes.len(), KEY_LENGTH);
        }
        let mut key = [0u8; KEY_LENGTH];
        key.copy_from_slice(&bytes);
        Ok(key)
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

/// Update hash chain by prepending new hash and trimming to MAX_HASH_CHAIN_LENGTH
/// Matches web client implementation
pub fn update_hash_chain(current_hash: &str, parent_chain: Option<&[String]>) -> Vec<String> {
    let mut new_chain = vec![current_hash.to_string()];
    if let Some(chain) = parent_chain {
        new_chain.extend(chain.iter().cloned());
    }
    new_chain.truncate(MAX_HASH_CHAIN_LENGTH);
    new_chain
}

/// Find common ancestor hash between two hash chains
/// Returns the first hash that appears in both chains, or None if no common ancestor
pub fn find_common_ancestor(chain_a: &[String], chain_b: &[String]) -> Option<String> {
    use std::collections::HashSet;
    let chain_b_set: HashSet<&String> = chain_b.iter().collect();
    for hash in chain_a {
        if chain_b_set.contains(hash) {
            return Some(hash.clone());
        }
    }
    None
}

impl Default for CryptoService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Get test password from environment variable or use default.
    /// Set JOTTERY_TEST_PASSWORD env var to override.
    fn test_password() -> String {
        std::env::var("JOTTERY_TEST_PASSWORD")
            .unwrap_or_else(|_| "test_password_placeholder".to_string())
    }

    #[test]
    fn test_key_derivation() {
        let service = CryptoService::new();
        let password = test_password();
        let salt = service.generate_salt();

        let key1 = service.derive_key(&password, &salt, 100_000).unwrap();
        let key2 = service.derive_key(&password, &salt, 100_000).unwrap();

        // Same password and salt should produce same key
        assert_eq!(key1, key2);

        // Different password should produce different key
        let key3 = service.derive_key(&format!("different_{}", password), &salt, 100_000).unwrap();
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_text_encryption_decryption() {
        let service = CryptoService::new();
        let salt = service.generate_salt();
        let key = service.derive_key(&test_password(), &salt, 100_000).unwrap();

        let plaintext = "Hello, World! This is a test message.";
        let encrypted = service.encrypt_text(plaintext, &key).unwrap();
        let decrypted = service.decrypt_text(&encrypted, &key).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    #[test]
    fn test_binary_encryption_decryption() {
        let service = CryptoService::new();
        let salt = service.generate_salt();
        let key = service.derive_key(&test_password(), &salt, 100_000).unwrap();

        let data = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let encrypted = service.encrypt_binary(&data, &key).unwrap();
        let decrypted = service.decrypt_binary(&encrypted, &key).unwrap();

        assert_eq!(data, decrypted);
    }

    #[test]
    fn test_wrong_key_decryption_fails() {
        let service = CryptoService::new();
        let salt = service.generate_salt();
        // Intentionally different passwords to verify key mismatch detection
        let password = test_password();
        let key1 = service.derive_key(&password, &salt, 100_000).unwrap();
        let key2 = service.derive_key(&format!("wrong_{}", password), &salt, 100_000).unwrap();

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
        let salt = service.generate_salt();
        let key = service.derive_key(&test_password(), &salt, 100_000).unwrap();

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

    // ===== Hash Chain Tests =====

    #[test]
    fn test_update_hash_chain_empty_parent() {
        let hash = "abc123";
        let chain = update_hash_chain(hash, None);

        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0], hash);
    }

    #[test]
    fn test_update_hash_chain_with_parent() {
        let current = "new_hash";
        let parent = vec!["parent1".to_string(), "parent2".to_string()];
        let chain = update_hash_chain(current, Some(&parent));

        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], current);
        assert_eq!(chain[1], "parent1");
        assert_eq!(chain[2], "parent2");
    }

    #[test]
    fn test_update_hash_chain_truncates_at_max_length() {
        // Create a parent chain at max length
        let parent: Vec<String> = (0..MAX_HASH_CHAIN_LENGTH)
            .map(|i| format!("hash_{}", i))
            .collect();

        let current = "new_hash";
        let chain = update_hash_chain(current, Some(&parent));

        // Should still be MAX_HASH_CHAIN_LENGTH (new hash replaces oldest)
        assert_eq!(chain.len(), MAX_HASH_CHAIN_LENGTH);
        assert_eq!(chain[0], current);
        // Last element should be hash_48 (not hash_49 which was truncated)
        assert_eq!(chain[MAX_HASH_CHAIN_LENGTH - 1], "hash_48");
    }

    #[test]
    fn test_update_hash_chain_empty_parent_slice() {
        let current = "new_hash";
        let empty: Vec<String> = vec![];
        let chain = update_hash_chain(current, Some(&empty));

        assert_eq!(chain.len(), 1);
        assert_eq!(chain[0], current);
    }

    #[test]
    fn test_find_common_ancestor_basic() {
        let chain_a = vec!["a1".to_string(), "common".to_string(), "old".to_string()];
        let chain_b = vec!["b1".to_string(), "b2".to_string(), "common".to_string()];

        let ancestor = find_common_ancestor(&chain_a, &chain_b);
        assert_eq!(ancestor, Some("common".to_string()));
    }

    #[test]
    fn test_find_common_ancestor_first_match() {
        // Should return the first match from chain_a's perspective
        let chain_a = vec!["a1".to_string(), "first".to_string(), "second".to_string()];
        let chain_b = vec!["b1".to_string(), "second".to_string(), "first".to_string()];

        let ancestor = find_common_ancestor(&chain_a, &chain_b);
        // "first" appears first in chain_a and is in chain_b
        assert_eq!(ancestor, Some("first".to_string()));
    }

    #[test]
    fn test_find_common_ancestor_no_common() {
        let chain_a = vec!["a1".to_string(), "a2".to_string()];
        let chain_b = vec!["b1".to_string(), "b2".to_string()];

        let ancestor = find_common_ancestor(&chain_a, &chain_b);
        assert_eq!(ancestor, None);
    }

    #[test]
    fn test_find_common_ancestor_identical_chains() {
        let chain = vec!["h1".to_string(), "h2".to_string(), "h3".to_string()];

        let ancestor = find_common_ancestor(&chain, &chain);
        // First element should be the common ancestor
        assert_eq!(ancestor, Some("h1".to_string()));
    }

    #[test]
    fn test_find_common_ancestor_empty_chains() {
        let empty: Vec<String> = vec![];
        let chain = vec!["h1".to_string()];

        assert_eq!(find_common_ancestor(&empty, &chain), None);
        assert_eq!(find_common_ancestor(&chain, &empty), None);
        assert_eq!(find_common_ancestor(&empty, &empty), None);
    }

    #[test]
    fn test_find_common_ancestor_single_element() {
        let chain_a = vec!["common".to_string()];
        let chain_b = vec!["common".to_string()];

        let ancestor = find_common_ancestor(&chain_a, &chain_b);
        assert_eq!(ancestor, Some("common".to_string()));
    }

    // ===== Envelope Encryption Tests =====

    #[test]
    fn test_generate_master_key() {
        let service = CryptoService::new();
        let key1 = service.generate_master_key();
        let key2 = service.generate_master_key();

        // Random keys should be different
        assert_ne!(key1, key2);
        assert_eq!(key1.len(), 32);
    }

    #[test]
    fn test_wrap_unwrap_master_key() {
        let service = CryptoService::new();
        let master_key = service.generate_master_key();
        let salt = service.generate_salt();
        let wrapping_key = service.derive_key(&test_password(), &salt, 100_000).unwrap();

        let wrapped = service.wrap_master_key(&wrapping_key, &master_key).unwrap();
        let unwrapped = service.unwrap_master_key(&wrapping_key, &wrapped).unwrap();

        assert_eq!(master_key, unwrapped);
    }

    #[test]
    fn test_wrap_unwrap_wrong_key_fails() {
        let service = CryptoService::new();
        let master_key = service.generate_master_key();
        let salt = service.generate_salt();
        let wrapping_key = service.derive_key(&test_password(), &salt, 100_000).unwrap();
        let wrong_key = service.derive_key(&format!("wrong_{}", test_password()), &salt, 100_000).unwrap();

        let wrapped = service.wrap_master_key(&wrapping_key, &master_key).unwrap();
        assert!(service.unwrap_master_key(&wrong_key, &wrapped).is_err());
    }

    #[test]
    fn test_derive_wrapping_key_deterministic() {
        let service = CryptoService::new();
        // UUID is 36 chars = 36 bytes >= 32
        let user_id = "550e8400-e29b-41d4-a716-446655440000";
        let password = test_password();

        let key1 = service.derive_wrapping_key(&password, user_id).unwrap();
        let key2 = service.derive_wrapping_key(&password, user_id).unwrap();

        assert_eq!(key1, key2);

        // Different userId should produce different key
        let key3 = service.derive_wrapping_key(&password, "660e8400-e29b-41d4-a716-446655440000").unwrap();
        assert_ne!(key1, key3);
    }

    #[test]
    fn test_compute_content_hash_deterministic() {
        use crate::models::Note;

        let service = CryptoService::new();
        let note = Note::new("Test content".to_string());

        let hash1 = service.compute_content_hash(&note);
        let hash2 = service.compute_content_hash(&note);

        // Same note should produce same hash
        assert_eq!(hash1, hash2);

        // Hash should be valid base64
        assert!(general_purpose::STANDARD.decode(&hash1).is_ok());
    }

    #[test]
    fn test_compute_content_hash_differs_by_content() {
        use crate::models::Note;

        let service = CryptoService::new();

        let note1 = Note::new("Content A".to_string());
        let mut note2 = Note::new("Content B".to_string());

        // Use same ID to isolate content difference
        note2.id = note1.id.clone();

        let hash1 = service.compute_content_hash(&note1);
        let hash2 = service.compute_content_hash(&note2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_content_hash_differs_by_tags() {
        use crate::models::Note;

        let service = CryptoService::new();

        let mut note1 = Note::new("Same content".to_string());
        note1.tags = vec!["tag1".to_string()];

        let mut note2 = Note::new("Same content".to_string());
        note2.id = note1.id.clone();
        note2.tags = vec!["tag2".to_string()];

        let hash1 = service.compute_content_hash(&note1);
        let hash2 = service.compute_content_hash(&note2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_content_hash_differs_by_pinned() {
        use crate::models::Note;

        let service = CryptoService::new();

        let mut note1 = Note::new("Same content".to_string());
        note1.pinned = false;

        let mut note2 = Note::new("Same content".to_string());
        note2.id = note1.id.clone();
        note2.pinned = true;

        let hash1 = service.compute_content_hash(&note1);
        let hash2 = service.compute_content_hash(&note2);

        assert_ne!(hash1, hash2);
    }

    #[test]
    fn test_compute_content_hash_ignores_timestamps() {
        use crate::models::Note;
        use chrono::Duration;

        let service = CryptoService::new();

        let note1 = Note::new("Same content".to_string());
        let mut note2 = Note::new("Same content".to_string());
        note2.id = note1.id.clone();
        note2.created_at = note1.created_at + Duration::hours(1);
        note2.modified_at = note1.modified_at + Duration::hours(1);

        let hash1 = service.compute_content_hash(&note1);
        let hash2 = service.compute_content_hash(&note2);

        // Timestamps should not affect hash
        assert_eq!(hash1, hash2);
    }
}
