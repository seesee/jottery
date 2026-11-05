//! Authentication and password management operations

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;
use zeroize::{Zeroize, Zeroizing};

use crate::{
    db::Database,
    repository::{NoteRepository, SettingsRepository, sync::SyncRepository},
    vault,
};

use super::super::app::App;
use super::super::state::AppState;

/// How long a note stays in the bin.
const BIN_RETENTION_DAYS: i64 = 30;

/// Unlock the database with the password stored in app.password_input
pub fn unlock(app: &mut App) -> Result<()> {
    // Open database
    let db = Database::open(&app.db_path, &app.password_input)
        .context("Failed to open database")?;

    // Checks the password, and creates the vault if there isn't one yet
    let key = vault::unlock(db.connection(), &app.crypto, &app.password_input)?;

    app.key_manager.set_master_key(key);
    app.key = Some(key);
    app.db = Some(db);

    // Check if API key needs encryption/clone-device (from paste credentials flow)
    if let Some(db) = &app.db {
        let sync_repo = SyncRepository::new(db.connection());

        if let Ok(Some(mut metadata)) = sync_repo.get_metadata() {
            if let Some(api_key_str) = &metadata.api_key {
                // Get device name if pending
                let device_name = metadata.pending_device_name.clone()
                    .unwrap_or_else(|| hostname::get()
                        .map(|h| h.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "Jottery TUI".to_string()));

                // Check if API key is plaintext (prefixed with "PLAINTEXT:")
                if let Some(plaintext_key) = api_key_str.strip_prefix("PLAINTEXT:") {
                    app.debug_log("Unlock - Detected plaintext API key from import");

                    // If there's a pending device name, call clone-device to register as new device
                    if metadata.pending_device_name.is_some() {
                        app.debug_log(&format!("Unlock - Calling clone-device with device name: {}", device_name));

                        let endpoint = &metadata.sync_endpoint;
                        if endpoint.is_empty() {
                            app.debug_log("Unlock - ERROR: No endpoint for clone-device");
                            return Err(anyhow::anyhow!("No sync endpoint configured for clone-device"));
                        }

                        // Call clone-device endpoint
                        let client = crate::api::AuthClient::new(endpoint.clone());
                        let clone_result = client.clone_device(plaintext_key, &device_name, "tui")
                            .map_err(|e| anyhow::anyhow!("Clone device failed: {}", e))?;

                        app.debug_log(&format!("Unlock - Clone-device successful, new client_id: {}", clone_result.client_id));

                        // Encrypt the NEW API key
                        let encrypted = app.crypto.encrypt_text(&clone_result.api_key, &key)?;
                        let encrypted_api_key = serde_json::to_string(&encrypted)?;

                        // Update metadata with NEW credentials
                        metadata.api_key = Some(encrypted_api_key);
                        metadata.client_id = Some(clone_result.client_id);
                        metadata.user_id = Some(clone_result.user_id.clone());
                        metadata.sync_enabled = true;
                        metadata.forget_server_state();
                        metadata.pending_device_name = None; // Clear pending name
                        sync_repo.update_metadata(&metadata)?;

                        // Update app settings
                        app.settings.sync_enabled = true;
                        let settings_repo = SettingsRepository::new(db.connection());
                        settings_repo.update(&app.settings)?;

                        app.debug_log("Unlock - Registered as new device, sync enabled");
                    } else {
                        // Legacy flow (no device name) - encrypt directly
                        app.debug_log("Unlock - Encrypting API key with new key (legacy import)");

                        let encrypted = app.crypto.encrypt_text(plaintext_key, &key)?;
                        let encrypted_api_key = serde_json::to_string(&encrypted)?;

                        metadata.api_key = Some(encrypted_api_key);
                        sync_repo.update_metadata(&metadata)?;

                        app.debug_log("Unlock - API key encrypted and saved");
                    }
                }
                // Check if credentials are encrypted (prefixed with "ENCRYPTED:")
                else if let Some(encrypted_payload) = api_key_str.strip_prefix("ENCRYPTED:") {
                    app.debug_log("Unlock - Detected encrypted credentials, decrypting");

                    use base64::Engine;

                    // Decode and parse the encrypted data
                    let encrypted_json = base64::engine::general_purpose::STANDARD
                        .decode(encrypted_payload)
                        .map_err(|e| anyhow::anyhow!("Invalid base64 in encrypted credentials: {}", e))?;
                    let encrypted_json_str = String::from_utf8(encrypted_json)
                        .map_err(|e| anyhow::anyhow!("Invalid UTF-8 in encrypted credentials: {}", e))?;
                    let encrypted_data: crate::crypto::EncryptedData = serde_json::from_str(&encrypted_json_str)
                        .map_err(|e| anyhow::anyhow!("Invalid JSON in encrypted credentials: {}", e))?;

                    // Decrypt the credentials JSON
                    let creds_json = app.crypto.decrypt_text(&encrypted_data, &key)
                        .map_err(|_| anyhow::anyhow!("Failed to decrypt credentials. Please ensure you are using the same password as the source device."))?;

                    // Parse the decrypted credentials
                    let creds: serde_json::Value = serde_json::from_str(&creds_json)
                        .map_err(|e| anyhow::anyhow!("Invalid decrypted credentials JSON: {}", e))?;

                    let endpoint = creds["endpoint"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing endpoint in decrypted credentials"))?;
                    let api_key = creds["apiKey"].as_str()
                        .ok_or_else(|| anyhow::anyhow!("Missing apiKey in decrypted credentials"))?;

                    app.debug_log("Unlock - Credentials decrypted successfully");

                    // If there's a pending device name, call clone-device to register as new device
                    if metadata.pending_device_name.is_some() {
                        app.debug_log(&format!("Unlock - Calling clone-device with device name: {}", device_name));

                        // Call clone-device endpoint
                        let client = crate::api::AuthClient::new(endpoint.to_string());
                        let clone_result = client.clone_device(api_key, &device_name, "tui")
                            .map_err(|e| anyhow::anyhow!("Clone device failed: {}", e))?;

                        app.debug_log(&format!("Unlock - Clone-device successful, new client_id: {}", clone_result.client_id));

                        // Encrypt the NEW API key
                        let encrypted = app.crypto.encrypt_text(&clone_result.api_key, &key)?;
                        let encrypted_api_key = serde_json::to_string(&encrypted)?;

                        // Update metadata with NEW credentials
                        metadata.client_id = Some(clone_result.client_id);
                        metadata.user_id = Some(clone_result.user_id.clone());
                        metadata.sync_endpoint = endpoint.to_string();
                        metadata.api_key = Some(encrypted_api_key);
                        metadata.sync_enabled = true;
                        metadata.forget_server_state();
                        metadata.pending_device_name = None; // Clear pending name
                        sync_repo.update_metadata(&metadata)?;

                        // Update app settings and save to database
                        app.settings.sync_endpoint = Some(endpoint.to_string());
                        app.settings.sync_enabled = true;
                        let settings_repo = SettingsRepository::new(db.connection());
                        settings_repo.update(&app.settings)?;

                        app.debug_log("Unlock - Registered as new device, sync enabled");
                    } else {
                        // Legacy flow - use credentials directly (old behaviour for backwards compat)
                        let client_id = creds["clientId"].as_str()
                            .ok_or_else(|| anyhow::anyhow!("Missing clientId in decrypted credentials"))?;

                        // Re-encrypt the API key for storage
                        let encrypted = app.crypto.encrypt_text(api_key, &key)?;
                        let encrypted_api_key = serde_json::to_string(&encrypted)?;

                        // Update metadata with decrypted values
                        metadata.client_id = Some(client_id.to_string());
                        metadata.sync_endpoint = endpoint.to_string();
                        metadata.api_key = Some(encrypted_api_key);
                        metadata.sync_enabled = true;
                        metadata.forget_server_state();
                        sync_repo.update_metadata(&metadata)?;

                        // Update app settings and save to database
                        app.settings.sync_endpoint = Some(endpoint.to_string());
                        app.settings.sync_enabled = true;
                        let settings_repo = SettingsRepository::new(db.connection());
                        settings_repo.update(&app.settings)?;

                        app.debug_log("Unlock - Encrypted credentials processed (legacy), sync enabled");
                    }
                }
            }
        }
    }

    // What's been in the bin long enough goes now, on every client. Not being able to
    // doesn't stop the unlock, and the next one tries again.
    if let Some(db) = &app.db {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(BIN_RETENTION_DAYS);
        if let Err(e) = NoteRepository::new(db.connection()).purge_expired_bin(cutoff) {
            app.debug_log(&format!("Unlock - Could not empty the old part of the bin: {e}"));
        }
    }

    // Load notes
    super::notes::load_notes(app)?;

    // Load settings
    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        app.settings = settings_repo.get()?;
        app.debug_log(&format!("Unlock - Settings loaded: theme={}", app.settings.theme));
        app.debug_log(&format!("Unlock - Theme scheme name: {}", app.settings.theme.scheme_name()));
        app.debug_log(&format!("Unlock - Color palette present: {}", app.settings.color_palette.is_some()));
        app.debug_log(&format!("Unlock - Tag colors present: {}", app.settings.tag_colors.is_some()));
        // Update color scheme from loaded settings
        app.color_scheme = crate::ui::ColorScheme::by_name(app.settings.theme.scheme_name());
        app.debug_log("Unlock - Color scheme updated");
    }
    app.apply_auto_lock_setting();
    super::settings::apply_language(app);

    // Store password if remember checkbox was enabled
    if app.remember_password_checkbox {
        // The password is still in app.password_input at this point
        let password_to_store = Zeroizing::new(app.password_input.clone());
        if let Err(e) = store_password_for_autounlock(app, &password_to_store) {
            app.error = Some(format!("Failed to store password: {}", e));
        } else {
            // Show different message based on storage backend type
            let msg = if app.password_storage.backend_type().is_secure() {
                t!("password.remember_keychain_enabled")
            } else {
                t!("password.remember_enabled")
            };
            app.sync_status = Some(msg.to_string());
            app.sync_status_set_at = Some(Instant::now());
        }
    }

    // Save unlock password for envelope encryption (server password may differ from notes password)
    app.unlock_password = Some(Zeroizing::new(app.password_input.clone()));

    // Clear password fields and reset flags
    app.password_input.zeroize();
    app.password_confirm.zeroize();
    app.is_new_database = false;  // Database now exists
    app.password_confirm_focused = false;  // Reset focus
    app.remember_password_checkbox = false;  // Reset checkbox
    app.state = AppState::NoteList;

    // Start SSE connection for real-time sync notifications
    app.start_sse_if_enabled();

    // Once the device has the password and a sync behind it, it can repair the server's
    // copy of the key if an older client left it unreadable
    app.server_key_check_owed = true;

    // Trigger an immediate sync to fetch inbox items and any pending changes
    app.trigger_sync();

    Ok(())
}

/// Enable/disable remember password feature
/// When enabling, encrypts and stores the current password
#[allow(dead_code)]
pub fn toggle_remember_password(app: &mut App) -> Result<()> {
    if app.settings.remember_password {
        // Disable: clear stored password using the storage backend
        app.settings.remember_password = false;

        // Delete stored password
        let _ = app.password_storage.delete();

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
    // Store password using the storage backend (keychain or file). A keychain that
    // doesn't keep it isn't worth keeping either: the file works, and says what it is.
    if let Err(e) = app.password_storage.store(password) {
        if !app.password_storage.backend_type().is_secure() {
            return Err(e);
        }

        app.debug_log(&format!("Keychain refused the password, falling back to file: {}", e));
        let config_dir = app.db_path.parent().context("Invalid db path")?;
        let file_storage = crate::password_storage::create_file_storage(config_dir);
        file_storage.store(password)?;
        app.password_storage = file_storage;
    }

    app.debug_log(&format!(
        "Password stored using {} backend",
        if app.password_storage.backend_type().is_secure() { "keychain" } else { "file" }
    ));

    // Update settings (note: we no longer store encrypted password in settings)
    app.settings.remember_password = true;

    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        settings_repo.update(&app.settings)?;
    }

    Ok(())
}

/// Forget stored password (disable auto-unlock)
pub fn forget_stored_password(app: &mut App) -> Result<()> {
    // Delete stored password using the storage backend
    let _ = app.password_storage.delete();

    // Update settings
    app.settings.remember_password = false;

    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        settings_repo.update(&app.settings)?;
    }

    Ok(())
}

/// Check a password against the open vault, for actions that ask for it again.
pub fn verify_password(app: &App, password: &str) -> Result<bool> {
    let db = app.db.as_ref().context("Database not unlocked")?;
    let key = app.key.as_ref().context("Vault is locked")?;
    vault::password_matches(db.connection(), &app.crypto, password, key)
}
