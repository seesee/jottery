//! Authentication and password management operations

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;

use crate::{
    crypto::CryptoService,
    db::Database,
    repository::{EncryptionRepository, SettingsRepository, sync::SyncRepository},
};

use super::super::app::App;
use super::super::state::AppState;

/// Unlock the database with the password stored in app.password_input
pub fn unlock(app: &mut App) -> Result<()> {
    // Open database
    let db = Database::open(&app.db_path, &app.password_input)
        .context("Failed to open database")?;

    let encryption_repo = EncryptionRepository::new(db.connection());

    // Get or create encryption metadata
    let (salt, iterations) = if let Some(metadata) = encryption_repo.get()? {
        // Load existing salt from database
        (metadata.salt, metadata.iterations)
    } else {
        // First-time setup: generate new salt and save it
        let new_salt = app.crypto.generate_salt();
        let iterations = 256_000;
        encryption_repo.save(&new_salt, iterations)?;
        (new_salt.to_vec(), iterations)
    };

    // Derive encryption key from password and salt
    app.debug_log(&format!("Unlock - Password length: {} chars", app.password_input.len()));
    app.debug_log(&format!("Unlock - Password is empty: {}", app.password_input.is_empty()));

    let key = app
        .crypto
        .derive_key(&app.password_input, &salt, iterations)?;

    // Debug logging for troubleshooting
    app.debug_log(&format!("Unlock - Salt (hex): {}", hex::encode(&salt)));
    app.debug_log(&format!("Unlock - Salt length: {} bytes", salt.len()));
    app.debug_log(&format!("Unlock - Iterations: {}", iterations));
    app.debug_log(&format!("Unlock - Key (first 8 bytes): {}", hex::encode(&key[0..8])));

    app.key_manager.set_master_key(key);
    app.key = Some(key);
    app.db = Some(db);

    // Check if API key needs encryption (from paste credentials flow)
    if let Some(db) = &app.db {
        let sync_repo = SyncRepository::new(db.connection());

        if let Ok(Some(mut metadata)) = sync_repo.get_metadata() {
            if let Some(api_key_str) = &metadata.api_key {
                // Check if API key is plaintext (prefixed with "PLAINTEXT:")
                if let Some(plaintext_key) = api_key_str.strip_prefix("PLAINTEXT:") {
                    app.debug_log("Unlock - Detected plaintext API key, encrypting with new key");

                    // Encrypt API key with the newly derived key
                    let encrypted = app.crypto.encrypt_text(plaintext_key, &key)?;
                    let encrypted_api_key = serde_json::to_string(&encrypted)?;

                    // Update metadata with encrypted API key
                    metadata.api_key = Some(encrypted_api_key);
                    sync_repo.update_metadata(&metadata)?;

                    app.debug_log("Unlock - API key encrypted and saved");
                }
            }
        }
    }

    // Load notes
    super::notes::load_notes(app)?;

    // Load settings
    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        app.settings = settings_repo.get()?;
        // Update color scheme from loaded settings
        app.color_scheme = crate::ui::ColorScheme::by_name(app.settings.theme.scheme_name());
    }

    // Store password if remember checkbox was enabled
    if app.remember_password_checkbox {
        // The password is still in app.password_input at this point
        let password_to_store = app.password_input.clone();
        if let Err(e) = store_password_for_autounlock(app, &password_to_store) {
            app.error = Some(format!("Failed to store password: {}", e));
        } else {
            app.sync_status = Some(t!("password.remember_enabled").to_string());
            app.sync_status_set_at = Some(Instant::now());
        }
    }

    // Clear password fields and reset flags
    app.password_input.clear();
    app.password_confirm.clear();
    app.is_new_database = false;  // Database now exists
    app.password_confirm_focused = false;  // Reset focus
    app.remember_password_checkbox = false;  // Reset checkbox
    app.state = AppState::NoteList;

    Ok(())
}

/// Attempt to auto-unlock using stored password
/// Returns Ok(true) if successfully unlocked, Ok(false) if no stored password, Err on failure
pub fn try_auto_unlock(app: &mut App) -> Result<bool> {
    // First, we need to read settings with a dummy password just to check if remember_password is enabled
    // This is a chicken-and-egg problem: we need the password to read settings, but settings contain the password!
    // Solution: Use a constant "bootstrap" password to encrypt the stored_password field specifically

    // For now, try to open database with empty password to see if it exists
    if !app.db_path.exists() {
        return Ok(false); // New database, can't auto-unlock
    }

    // Try to open with a known constant to read settings (this will fail but we'll handle it)
    // Actually, this won't work with SQLCipher. We need a different approach.
    // The password must be stored in a separate unencrypted file.

    // Check for stored password in a separate config file
    let config_dir = app.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
    let remember_file = config_dir.join(".jottery_remember");

    if !remember_file.exists() {
        return Ok(false); // No stored password
    }

    // Read and decrypt stored password
    let encrypted_password = std::fs::read_to_string(&remember_file)
        .context("Failed to read stored password file")?;

    if encrypted_password.trim().is_empty() {
        return Ok(false);
    }

    // Decrypt using device-specific constant key
    let device_key = get_device_key(app);
    let encrypted_data: crate::crypto::EncryptedData = serde_json::from_str(&encrypted_password)
        .context("Failed to parse stored password")?;
    let password = app.crypto.decrypt_text(&encrypted_data, &device_key)
        .context("Failed to decrypt stored password")?;

    // Try to unlock with this password
    app.password_input = password;
    match unlock(app) {
        Ok(()) => {
            app.debug_log("Auto-unlock successful");
            Ok(true)
        }
        Err(e) => {
            app.password_input.clear();
            // Delete invalid stored password file
            let _ = std::fs::remove_file(&remember_file);
            Err(e).context("Auto-unlock failed")
        }
    }
}

/// Get device-specific encryption key for storing password
/// WARNING: This is not cryptographically secure, just obfuscation
pub fn get_device_key(_app: &App) -> [u8; 32] {
    // Use a constant key derived from app name and version
    // Anyone with access to the code can decrypt this
    // The security warning makes this clear to users
    let constant = b"jottery-tui-device-key-v1.0.0---";
    let mut key = [0u8; 32];
    key.copy_from_slice(&constant[..32]);
    key
}

/// Enable/disable remember password feature
/// When enabling, encrypts and stores the current password
#[allow(dead_code)]
pub fn toggle_remember_password(app: &mut App) -> Result<()> {
    if app.settings.remember_password {
        // Disable: clear stored password
        app.settings.remember_password = false;
        app.settings.stored_password = None;

        // Delete remember file
        let config_dir = app.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let remember_file = config_dir.join(".jottery_remember");
        let _ = std::fs::remove_file(&remember_file);

        // Save settings
        if let Some(db) = &app.db {
            let settings_repo = SettingsRepository::new(db.connection());
            settings_repo.update(&app.settings)?;
        }

        app.sync_status = Some(t!("password.remember_disabled").to_string());
        app.sync_status_set_at = Some(Instant::now());
    } else {
        // Enable: this should be done through a confirmation flow
        app.sync_status = Some("Feature not yet fully implemented - use settings".to_string());
        app.sync_status_set_at = Some(Instant::now());
    }
    Ok(())
}

/// Store password for auto-unlock (call after successful unlock when user confirms)
pub fn store_password_for_autounlock(app: &mut App, password: &str) -> Result<()> {
    // Encrypt password with device key
    let device_key = get_device_key(app);
    let encrypted = app.crypto.encrypt_text(password, &device_key)?;
    let encrypted_json = serde_json::to_string(&encrypted)?;

    // Save to remember file
    let config_dir = app.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
    let remember_file = config_dir.join(".jottery_remember");
    std::fs::write(&remember_file, &encrypted_json)
        .context("Failed to write password storage file")?;

    // Update settings
    app.settings.remember_password = true;
    app.settings.stored_password = Some(encrypted_json);

    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        settings_repo.update(&app.settings)?;
    }

    Ok(())
}

/// Forget stored password (disable auto-unlock)
pub fn forget_stored_password(app: &mut App) -> Result<()> {
    // Delete remember file
    let config_dir = app.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
    let remember_file = config_dir.join(".jottery_remember");
    let _ = std::fs::remove_file(&remember_file);

    // Update settings
    app.settings.remember_password = false;
    app.settings.stored_password = None;

    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        settings_repo.update(&app.settings)?;
    }

    Ok(())
}

/// Verify password is correct for enabling remember password
/// Returns Ok(true) if password is correct, Ok(false) if incorrect, Err on database error
pub fn verify_password_for_remember(app: &App, password: &str) -> Result<bool> {
    // Try to open the database with the provided password
    // This verifies the password without actually unlocking the app
    match Database::open(&app.db_path, password) {
        Ok(_) => Ok(true),  // Password is correct
        Err(e) => {
            // Check if it's a password error or other error
            let error_msg = format!("{:?}", e);
            if error_msg.contains("wrong password") || error_msg.contains("corrupted") {
                Ok(false)  // Password is incorrect
            } else {
                Err(e)  // Other database error
            }
        }
    }
}
