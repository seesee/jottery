/// Cryptography module for Jottery TUI
/// Implements AES-256-GCM encryption with PBKDF2 key derivation
/// Matches web app implementation (src/lib/services/crypto.ts)

mod service;
mod key_manager;

pub use service::{
    CryptoService, DEFAULT_ITERATIONS, MAX_HASH_CHAIN_LENGTH,
    update_hash_chain, find_common_ancestor,
};
pub use key_manager::*;

// Re-export commonly used types
pub use crate::models::encryption::EncryptedData;
