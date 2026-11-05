//! Cryptography module for Jottery TUI
//! Implements AES-256-GCM encryption with PBKDF2 key derivation
//! Matches web app implementation (src/lib/services/crypto.ts)

mod service;
mod key_manager;

pub use service::{find_common_ancestor, update_hash_chain, CryptoService, DEFAULT_ITERATIONS, MAX_HASH_CHAIN_LENGTH, WRAPPING_ITERATIONS, WRAPPING_KDF_VERSION};
pub use key_manager::*;

// Re-export commonly used types
pub use crate::models::encryption::EncryptedData;
