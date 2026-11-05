//! Cryptographic utility functions

use sha2::{Digest, Sha256};

/// Compute SHA-256 hash of a string and return as lowercase hex
///
/// Used for hashing API keys, session tokens, and SSE tokens for storage.
/// The input is never stored; only the hash is persisted.
pub fn hash_sha256(data: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// A short stand-in for an email address in logs. It lets someone reading them see
/// that attempts share an address without the logs saying whose it is, or holding a
/// password that was typed into the wrong field.
pub fn log_id(email: &str) -> String {
    let mut id = hash_sha256(&email.trim().to_lowercase());
    id.truncate(12);
    id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_log_id_says_nothing_but_stays_the_same() {
        let id = log_id("Someone@Example.com ");

        assert_eq!(id, log_id("someone@example.com"));
        assert_ne!(id, log_id("someone.else@example.com"));
        assert_eq!(id.len(), 12);
        assert!(!id.contains("someone"));
    }

    #[test]
    fn test_hash_sha256() {
        // Known test vector
        let hash = hash_sha256("hello");
        assert_eq!(
            hash,
            "2cf24dba5fb0a30e26e83b2ac5b9e29e1b161e5c1fa7425e73043362938b9824"
        );
    }

    #[test]
    fn test_hash_sha256_empty() {
        let hash = hash_sha256("");
        assert_eq!(
            hash,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
    }

    #[test]
    fn test_hash_sha256_deterministic() {
        let hash1 = hash_sha256("test");
        let hash2 = hash_sha256("test");
        assert_eq!(hash1, hash2);
    }
}
