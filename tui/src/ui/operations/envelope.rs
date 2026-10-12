//! Envelope encryption operations
//!
//! Handles migration from legacy (direct PBKDF2) to envelope encryption,
//! and onboarding new devices by downloading the wrapped master key from server.

use anyhow::{Context, Result};
use base64::Engine as _;

use crate::{
    api::{AuthClient, PutWrappedKeyRequest},
    crypto::{WRAPPING_ITERATIONS, WRAPPING_KDF_VERSION},
    repository::{EncryptionRepository, sync::SyncRepository},
};

use super::super::app::App;

/// Migrate from legacy (direct PBKDF2) to envelope encryption.
///
/// The master key bytes are the same — just stored differently:
/// 1. Upload wrapped key to server (password + userId → wrapping key)
/// 2. Convert local storage to envelope format (password + random device salt → device key)
///
/// This is non-fatal: if it fails, the legacy path still works.
pub fn try_migrate_to_envelope(app: &mut App, password: &str, master_key: &[u8; 32], local_password: Option<&str>) -> Result<()> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
    let sync_repo = SyncRepository::new(db.connection());

    let metadata = sync_repo.get_metadata()?;
    let metadata = match metadata {
        Some(m) => m,
        None => {
            app.debug_log("[Migration] Skipping: no sync metadata");
            return Ok(());
        }
    };

    // Only migrate if sync is configured and we have a userId
    if !metadata.sync_enabled || metadata.user_id.is_none() || metadata.api_key.is_none() || metadata.sync_endpoint.is_empty() {
        app.debug_log("[Migration] Skipping envelope migration: sync not configured or no userId");
        return Ok(());
    }

    let user_id = metadata.user_id.as_deref().unwrap();
    let endpoint = &metadata.sync_endpoint;

    app.debug_log("[Migration] Starting legacy → envelope migration...");

    // Derive wrapping key from the notes password + userId and wrap master key.
    // The server sees the account password in plaintext at login, so the blob must
    // not be wrapped with it. `password` is only the account password when no
    // separate notes (unlock) password is available.
    let notes_password = local_password.unwrap_or(password);
    let wrapping_key = app.crypto.derive_wrapping_key(notes_password, user_id)?;
    let server_wrapped = app.crypto.wrap_master_key(&wrapping_key, master_key)?;

    // Get plaintext API key for auth
    let encrypted_api_key_json = metadata.api_key.as_deref().unwrap();
    let encrypted_api_key: crate::models::encryption::EncryptedData =
        serde_json::from_str(encrypted_api_key_json)
            .context("Failed to parse encrypted API key JSON")?;
    let plaintext_api_key = app.crypto.decrypt_text(&encrypted_api_key, master_key)?;

    // Upload to server
    let client = AuthClient::new(endpoint.clone());
    client.put_wrapped_key(&plaintext_api_key, &PutWrappedKeyRequest {
        blob: serde_json::to_string(&server_wrapped)?,
        kdf_version: WRAPPING_KDF_VERSION,
        kdf_iterations: WRAPPING_ITERATIONS,
    })?;
    app.debug_log("[Migration] Wrapped key uploaded to server");

    // Convert local storage to envelope format
    let device_salt = app.crypto.generate_salt();
    let device_key = app.crypto.derive_key(notes_password, &device_salt, crate::crypto::DEFAULT_ITERATIONS)?;
    let local_wrapped = app.crypto.wrap_master_key(&device_key, master_key)?;

    let device_salt_b64 = base64::engine::general_purpose::STANDARD.encode(&device_salt);
    let local_wrapped_json = serde_json::to_string(&local_wrapped)?;

    let encryption_repo = EncryptionRepository::new(db.connection());
    encryption_repo.save_envelope(1, &device_salt_b64, &local_wrapped_json, WRAPPING_KDF_VERSION)?;

    app.debug_log("[Migration] Local metadata migrated to envelope format");
    app.debug_log("[Migration] Migration complete!");

    Ok(())
}

/// Unwrap the server-held master key, trying each candidate password in order.
///
/// Duplicate candidates are skipped. If none work, the error from the last
/// attempt is returned. Nothing is written anywhere, so a failure is harmless.
fn unwrap_with_passwords(
    crypto: &crate::crypto::CryptoService,
    candidates: &[&str],
    user_id: &str,
    wrapped: &crate::models::encryption::EncryptedData,
) -> Result<[u8; 32]> {
    let mut tried: Vec<&str> = Vec::new();
    let mut last_error = anyhow::anyhow!("No password supplied to unwrap the master key");

    for &candidate in candidates {
        if tried.contains(&candidate) {
            continue;
        }
        tried.push(candidate);

        let wrapping_key = crypto.derive_wrapping_key(candidate, user_id)?;
        match crypto.unwrap_master_key(&wrapping_key, wrapped) {
            Ok(key) => return Ok(key),
            Err(e) => last_error = e,
        }
    }

    Err(last_error)
}

/// Onboard a new device by downloading the wrapped master key from the server.
///
/// Called during sync setup when the user provides email + password and the server
/// already has a wrapped key (from another device that migrated to envelope encryption).
///
/// Returns the unwrapped master key bytes.
pub fn onboard_from_server(
    app: &mut App,
    password: &str,
    endpoint: &str,
    user_id: &str,
    api_key: &str,
    local_password: Option<&str>,
) -> Result<[u8; 32]> {
    app.debug_log("[Onboard] Downloading wrapped key from server...");

    let client = AuthClient::new(endpoint.to_string());
    let wrapped_response = client.get_wrapped_key(api_key)?
        .ok_or_else(|| anyhow::anyhow!(
            "No wrapped key found on server. The source device may not have migrated to envelope encryption yet."
        ))?;

    // Unwrap the master key. New blobs are wrapped with the notes password; blobs
    // from older clients were wrapped with the account password, so try that too.
    let server_wrapped: crate::models::encryption::EncryptedData =
        serde_json::from_str(&wrapped_response.blob)
            .context("Invalid wrapped key blob JSON")?;
    let notes_password = local_password.unwrap_or(password);
    let master_key = unwrap_with_passwords(&app.crypto, &[notes_password, password], user_id, &server_wrapped)?;
    app.debug_log("[Onboard] Master key unwrapped from server blob");

    // Create local envelope storage
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
    let device_salt = app.crypto.generate_salt();
    let device_key = app.crypto.derive_key(notes_password, &device_salt, crate::crypto::DEFAULT_ITERATIONS)?;
    let local_wrapped = app.crypto.wrap_master_key(&device_key, &master_key)?;

    let device_salt_b64 = base64::engine::general_purpose::STANDARD.encode(&device_salt);
    let local_wrapped_json = serde_json::to_string(&local_wrapped)?;

    let encryption_repo = EncryptionRepository::new(db.connection());
    encryption_repo.save_envelope(1, &device_salt_b64, &local_wrapped_json, WRAPPING_KDF_VERSION)?;

    app.debug_log("[Onboard] Device onboarded via envelope encryption");

    Ok(master_key)
}

/// Change password with envelope encryption support.
///
/// With envelope encryption, changing the password only re-wraps the master key —
/// no note re-encryption needed.
pub fn change_password(
    app: &mut App,
    current_password: &str,
    new_password: &str,
    master_key: &[u8; 32],
) -> Result<()> {
    // Check envelope status before borrowing app mutably
    let is_legacy = {
        let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
        let encryption_repo = EncryptionRepository::new(db.connection());
        let enc_meta = encryption_repo.get()?
            .ok_or_else(|| anyhow::anyhow!("No encryption metadata found"))?;
        enc_meta.envelope_version.is_none()
    };

    // If still on legacy, migrate first
    if is_legacy {
        if let Err(e) = try_migrate_to_envelope(app, current_password, master_key, None) {
            app.debug_log(&format!("[ChangePassword] Migration failed (non-fatal): {}", e));
        }
    }

    // Re-wrap master key with new password for local storage
    let new_device_salt = app.crypto.generate_salt();
    let new_device_key = app.crypto.derive_key(new_password, &new_device_salt, crate::crypto::DEFAULT_ITERATIONS)?;
    let new_local_wrapped = app.crypto.wrap_master_key(&new_device_key, master_key)?;

    let device_salt_b64 = base64::engine::general_purpose::STANDARD.encode(&new_device_salt);
    let local_wrapped_json = serde_json::to_string(&new_local_wrapped)?;

    {
        let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
        let encryption_repo = EncryptionRepository::new(db.connection());
        encryption_repo.save_envelope(1, &device_salt_b64, &local_wrapped_json, WRAPPING_KDF_VERSION)?;
    }
    app.debug_log("[ChangePassword] Local envelope updated with new password");

    // Re-wrap master key for server (if sync configured)
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
    let sync_repo = SyncRepository::new(db.connection());
    if let Some(sync_meta) = sync_repo.get_metadata()? {
        if sync_meta.sync_enabled && sync_meta.user_id.is_some() && sync_meta.api_key.is_some() && !sync_meta.sync_endpoint.is_empty() {
            let user_id = sync_meta.user_id.as_deref().unwrap();

            match (|| -> Result<()> {
                let wrapping_key = app.crypto.derive_wrapping_key(new_password, user_id)?;
                let server_wrapped = app.crypto.wrap_master_key(&wrapping_key, master_key)?;

                // Get plaintext API key for auth
                let encrypted_api_key_json = sync_meta.api_key.as_deref().unwrap();
                let encrypted_api_key: crate::models::encryption::EncryptedData =
                    serde_json::from_str(encrypted_api_key_json)?;
                let plaintext_api_key = app.crypto.decrypt_text(&encrypted_api_key, master_key)?;

                let client = AuthClient::new(sync_meta.sync_endpoint.clone());
                client.put_wrapped_key(&plaintext_api_key, &PutWrappedKeyRequest {
                    blob: serde_json::to_string(&server_wrapped)?,
                    kdf_version: WRAPPING_KDF_VERSION,
                    kdf_iterations: WRAPPING_ITERATIONS,
                })?;
                app.debug_log("[ChangePassword] Server wrapped key updated");
                Ok(())
            })() {
                Ok(()) => {}
                Err(e) => {
                    app.debug_log(&format!("[ChangePassword] Failed to update server wrapped key (non-fatal): {}", e));
                }
            }
        }
    }

    app.debug_log("[ChangePassword] Password changed successfully (no re-encryption needed)");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::CryptoService;

    const USER_ID: &str = "b3c1f5a2-8f0e-4c55-9d52-2f6a1c0e7a11";

    /// A fresh random password, so no credential-like literals live in the tests
    fn random_password(crypto: &CryptoService) -> String {
        base64::engine::general_purpose::STANDARD.encode(crypto.generate_master_key())
    }

    fn wrap_with(crypto: &CryptoService, password: &str, key: &[u8; 32]) -> crate::models::encryption::EncryptedData {
        let wrapping_key = crypto.derive_wrapping_key(password, USER_ID).unwrap();
        crypto.wrap_master_key(&wrapping_key, key).unwrap()
    }

    #[test]
    fn unwraps_a_blob_wrapped_with_the_notes_password() {
        let crypto = CryptoService::new();
        let (notes, server) = (random_password(&crypto), random_password(&crypto));
        let key = crypto.generate_master_key();
        let blob = wrap_with(&crypto, &notes, &key);

        let unwrapped = unwrap_with_passwords(&crypto, &[&notes, &server], USER_ID, &blob).unwrap();

        assert_eq!(unwrapped, key);
    }

    #[test]
    fn falls_back_to_the_server_password_for_older_blobs() {
        let crypto = CryptoService::new();
        let (notes, server) = (random_password(&crypto), random_password(&crypto));
        let key = crypto.generate_master_key();
        let blob = wrap_with(&crypto, &server, &key);

        let unwrapped = unwrap_with_passwords(&crypto, &[&notes, &server], USER_ID, &blob).unwrap();

        assert_eq!(unwrapped, key);
    }

    #[test]
    fn fails_when_no_candidate_matches() {
        let crypto = CryptoService::new();
        let (notes, other_a, other_b) = (
            random_password(&crypto),
            random_password(&crypto),
            random_password(&crypto),
        );
        let key = crypto.generate_master_key();
        let blob = wrap_with(&crypto, &notes, &key);

        let result = unwrap_with_passwords(&crypto, &[&other_a, &other_b], USER_ID, &blob);

        assert!(result.is_err());
    }

    #[test]
    fn identical_candidates_are_only_tried_once() {
        let crypto = CryptoService::new();
        let notes = random_password(&crypto);
        let key = crypto.generate_master_key();
        let blob = wrap_with(&crypto, &notes, &key);

        // Same password for notes and account: must still work
        let unwrapped = unwrap_with_passwords(&crypto, &[&notes, &notes], USER_ID, &blob).unwrap();

        assert_eq!(unwrapped, key);
    }
}
