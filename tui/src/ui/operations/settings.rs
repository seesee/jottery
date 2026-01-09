//! Settings management operations

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;

use crate::{
    models::sync::SyncCredentials,
    repository::{EncryptionRepository, SettingsRepository, sync::SyncRepository},
};

use super::super::app::App;
use super::super::state::{AppState, InputMode};

/// Start editing a setting field (forward for cyclic fields)
pub fn start_editing_setting(app: &mut App) {
    // Populate input buffer with current value for string/number fields
    match app.selected_setting {
        0 => {
            // Language
            app.setting_input = app.settings.language.clone();
            app.input_mode = InputMode::SettingsEdit;
        }
        1 => {
            // Theme: cycle forward immediately, no input needed
            cycle_theme(app);
        }
        2 => {
            // Sort order: cycle forward immediately, no input needed
            cycle_sort_order(app);
        }
        3 => {
            // Auto-lock timeout
            app.setting_input = app.settings.auto_lock_timeout.to_string();
            app.input_mode = InputMode::SettingsEdit;
        }
        4 => {
            // Sync enabled: toggle immediately
            app.settings.sync_enabled = !app.settings.sync_enabled;
            if let Err(e) = save_settings(app) {
                app.error = Some(format!("Failed to save settings: {}", e));
            }
        }
        5 => {
            // Sync endpoint
            app.setting_input = app.settings.sync_endpoint.clone().unwrap_or_default();
            app.input_mode = InputMode::SettingsEdit;
        }
        6 => {
            // Auto-sync interval
            app.setting_input = app.settings.auto_sync_interval_minutes.to_string();
            app.input_mode = InputMode::SettingsEdit;
        }
        7 => {
            // Remember password: toggle with password verification
            if app.settings.remember_password {
                // Currently ON -> turn OFF and delete stored password
                if let Err(e) = super::auth::forget_stored_password(app) {
                    app.error = Some(format!("Failed to forget password: {}", e));
                } else {
                    app.sync_status = Some(t!("password.remember_disabled").to_string());
                    app.sync_status_set_at = Some(Instant::now());
                }
            } else {
                // Currently OFF -> prompt for password to enable
                app.setting_input.clear();
                app.input_mode = InputMode::PasswordVerify;
                app.sync_status = Some(t!("password.enter_to_enable").to_string());
                app.sync_status_set_at = Some(Instant::now());
            }
        }
        _ => {}
    }
}

/// Start editing a setting field (backward for cyclic fields)
pub fn start_editing_setting_backward(app: &mut App) {
    // For cyclic fields, cycle backward; for others, behave like forward
    match app.selected_setting {
        0 => {
            // Language
            app.setting_input = app.settings.language.clone();
            app.input_mode = InputMode::SettingsEdit;
        }
        1 => {
            // Theme: cycle backward immediately, no input needed
            cycle_theme_backward(app);
        }
        2 => {
            // Sort order: cycle backward immediately, no input needed
            cycle_sort_order_backward(app);
        }
        3 => {
            // Auto-lock timeout
            app.setting_input = app.settings.auto_lock_timeout.to_string();
            app.input_mode = InputMode::SettingsEdit;
        }
        4 => {
            // Sync enabled: toggle immediately
            app.settings.sync_enabled = !app.settings.sync_enabled;
            if let Err(e) = save_settings(app) {
                app.error = Some(format!("Failed to save settings: {}", e));
            }
        }
        5 => {
            // Sync endpoint
            app.setting_input = app.settings.sync_endpoint.clone().unwrap_or_default();
            app.input_mode = InputMode::SettingsEdit;
        }
        6 => {
            // Auto-sync interval
            app.setting_input = app.settings.auto_sync_interval_minutes.to_string();
            app.input_mode = InputMode::SettingsEdit;
        }
        7 => {
            // Remember password: toggle with password verification
            if app.settings.remember_password {
                // Currently ON -> turn OFF and delete stored password
                if let Err(e) = super::auth::forget_stored_password(app) {
                    app.error = Some(format!("Failed to forget password: {}", e));
                } else {
                    app.sync_status = Some(t!("password.remember_disabled").to_string());
                    app.sync_status_set_at = Some(Instant::now());
                }
            } else {
                // Currently OFF -> prompt for password to enable
                app.setting_input.clear();
                app.input_mode = InputMode::PasswordVerify;
                app.sync_status = Some(t!("password.enter_to_enable").to_string());
                app.sync_status_set_at = Some(Instant::now());
            }
        }
        _ => {}
    }
}

/// Save edited setting value
pub fn save_setting_value(app: &mut App) -> Result<()> {
    match app.selected_setting {
        0 => {
            // Language
            app.settings.language = app.setting_input.clone();
        }
        3 => {
            // Auto-lock timeout
            if let Ok(timeout) = app.setting_input.parse::<i32>() {
                if (1..=1440).contains(&timeout) {
                    app.settings.auto_lock_timeout = timeout;
                } else {
                    anyhow::bail!("Auto-lock timeout must be between 1 and 1440 minutes");
                }
            } else {
                anyhow::bail!("Invalid number");
            }
        }
        5 => {
            // Sync endpoint
            if app.setting_input.is_empty() {
                app.settings.sync_endpoint = None;
            } else {
                if !app.setting_input.starts_with("http://") && !app.setting_input.starts_with("https://") {
                    anyhow::bail!("Sync endpoint must start with http:// or https://");
                }
                app.settings.sync_endpoint = Some(app.setting_input.clone());
            }
        }
        6 => {
            // Auto-sync interval
            if let Ok(interval) = app.setting_input.parse::<i32>() {
                if (0..=1440).contains(&interval) {
                    app.settings.auto_sync_interval_minutes = interval;
                } else {
                    anyhow::bail!("Auto-sync interval must be between 0 and 1440 minutes");
                }
            } else {
                anyhow::bail!("Invalid number");
            }
        }
        _ => {}
    }

    save_settings(app)
}

/// Cycle forward through color scheme options
pub fn cycle_theme(app: &mut App) {
    app.settings.theme.cycle_next();
    // Update cached color scheme
    app.color_scheme = crate::ui::ColorScheme::by_name(app.settings.theme.scheme_name());
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
}

/// Cycle backward through color scheme options
pub fn cycle_theme_backward(app: &mut App) {
    app.settings.theme.cycle_prev();
    // Update cached color scheme
    app.color_scheme = crate::ui::ColorScheme::by_name(app.settings.theme.scheme_name());
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
}

/// Cycle forward through sort order options
pub fn cycle_sort_order(app: &mut App) {
    app.settings.sort_order = app.settings.sort_order.next();
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
}

/// Cycle backward through sort order options
pub fn cycle_sort_order_backward(app: &mut App) {
    app.settings.sort_order = app.settings.sort_order.prev();
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
}

/// Save settings to database
pub fn save_settings(app: &mut App) -> Result<()> {
    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        settings_repo.update(&app.settings)?;
    }
    Ok(())
}

/// Paste sync credentials from clipboard
#[allow(dead_code)]
pub fn paste_sync_credentials(app: &mut App) -> Result<()> {
    // Get clipboard content
    let mut clipboard = arboard::Clipboard::new()
        .context("Failed to access clipboard")?;
    let clipboard_text = clipboard.get_text()
        .context("Failed to read from clipboard")?;

    // Decode credentials
    let creds = SyncCredentials::from_base64(clipboard_text.trim())
        .context("Invalid sync credentials format")?;

    app.debug_log(&format!("Paste credentials - endpoint: {}", creds.endpoint));
    app.debug_log(&format!("Paste credentials - client_id: {}", creds.client_id));
    app.debug_log(&format!("Paste credentials - has salt: {}", creds.salt.is_some()));

    // Get database
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    // If web app salt is provided, update it first
    // We'll encrypt the API key AFTER the user unlocks with the new salt
    if let Some(salt_b64) = &creds.salt {
        use base64::Engine;
        let encryption_repo = EncryptionRepository::new(db.connection());

        // Decode the base64 salt from web app
        let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64)
            .context("Invalid base64 salt from sync credentials")?;

        app.debug_log(&format!("Paste credentials - Salt (base64): {}", salt_b64));
        app.debug_log(&format!("Paste credentials - Salt (hex): {}", hex::encode(&salt)));
        app.debug_log(&format!("Paste credentials - Salt length: {} bytes", salt.len()));

        // Validate salt length - must be at least 32 bytes (256 bits) for PBKDF2
        if salt.len() < 32 {
            anyhow::bail!("Invalid salt length: {} bytes (expected at least 32 bytes). Web app salt may be incompatible with TUI.", salt.len());
        }

        // Update encryption metadata with web app's salt AND iteration count
        app.debug_log("Paste credentials - Saving salt with 100,000 iterations");
        encryption_repo.save(&salt, 100_000)?;
        app.debug_log("Paste credentials - Salt saved successfully");
    }

    // Save sync metadata with PLAINTEXT API key temporarily
    // It will be encrypted on next unlock with the new salt
    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

    // Store API key as plaintext temporarily (will be encrypted on next unlock)
    // We use a special marker to indicate it needs encryption
    app.debug_log("Paste credentials - Storing API key (will encrypt on next unlock)");
    metadata.api_key = Some(format!("PLAINTEXT:{}", creds.api_key));
    metadata.client_id = Some(creds.client_id);
    metadata.sync_endpoint = creds.endpoint.clone();
    metadata.sync_enabled = true;

    sync_repo.update_metadata(&metadata)?;

    // Update settings
    app.settings.sync_endpoint = Some(creds.endpoint);
    app.settings.sync_enabled = true;
    save_settings(app)?;

    // If web app salt was provided, we need to lock and force re-unlock with the new salt
    // This ensures the user knows the salt was changed and re-enters their password
    if creds.salt.is_some() {
        app.debug_log("Paste credentials - Locking database to force re-unlock with new salt");

        // Automatically lock the database
        app.key = None;
        app.notes.clear();
        app.selected_note = 0;
        app.password_input.clear();
        app.password_confirm.clear();
        app.input_mode = InputMode::Normal;
        app.state = AppState::Locked;

        // Show message about what happened
        app.error = Some(t!("sync.salt_sync").to_string());
    }

    Ok(())
}

/// Copy sync credentials to clipboard
#[allow(dead_code)]
pub fn copy_sync_credentials(app: &mut App) -> Result<()> {
    // Get sync metadata
    if let Some(db) = &app.db {
        let sync_repo = SyncRepository::new(db.connection());
        let metadata = sync_repo.get_metadata()?
            .ok_or_else(|| anyhow::anyhow!("No sync configuration found"))?;

        // Check if credentials exist
        let encrypted_api_key = metadata.api_key
            .ok_or_else(|| anyhow::anyhow!("No API key configured. Enable sync first."))?;
        let client_id = metadata.client_id
            .ok_or_else(|| anyhow::anyhow!("No client ID found. Enable sync first."))?;

        // Decrypt API key
        let api_key = if let Some(key) = &app.key {
            let encrypted: crate::crypto::EncryptedData = serde_json::from_str(&encrypted_api_key)?;
            app.crypto.decrypt_text(&encrypted, key)?
        } else {
            anyhow::bail!("Database not unlocked");
        };

        // Create credentials payload
        let creds = SyncCredentials::new(
            metadata.sync_endpoint,
            api_key,
            client_id,
        );

        // Encode to base64
        let encoded = creds.to_base64()?;

        // Copy to clipboard
        let mut clipboard = arboard::Clipboard::new()
            .context("Failed to access clipboard")?;
        clipboard.set_text(&encoded)
            .context("Failed to write to clipboard")?;
    } else {
        anyhow::bail!("Database not available");
    }

    Ok(())
}

/// Generate sync credentials text (encrypted format: jottery:v1:<salt>.<encrypted_payload>)
pub fn generate_sync_credentials_text(app: &App) -> Result<String> {
    // Get sync metadata
    if let Some(db) = &app.db {
        let sync_repo = SyncRepository::new(db.connection());
        let metadata = sync_repo.get_metadata()?
            .ok_or_else(|| anyhow::anyhow!("No sync configuration found"))?;

        // Check if credentials exist
        let encrypted_api_key = metadata.api_key
            .ok_or_else(|| anyhow::anyhow!("No API key configured. Enable sync first."))?;
        let client_id = metadata.client_id
            .ok_or_else(|| anyhow::anyhow!("No client ID found. Enable sync first."))?;

        // Need encryption key to decrypt API key and encrypt credentials
        let key = app.key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

        // Decrypt API key
        let encrypted: crate::crypto::EncryptedData = serde_json::from_str(&encrypted_api_key)?;
        let api_key = app.crypto.decrypt_text(&encrypted, key)?;

        // Get encryption metadata for salt
        let encryption_repo = EncryptionRepository::new(db.connection());
        let encryption_meta = encryption_repo.get()?
            .ok_or_else(|| anyhow::anyhow!("Encryption metadata not found"))?;

        // Convert salt to base64 string
        use base64::Engine;
        let salt_b64 = base64::engine::general_purpose::STANDARD.encode(&encryption_meta.salt);

        // Create credentials payload WITHOUT salt (salt goes in prefix)
        let creds = SyncCredentials::new(
            metadata.sync_endpoint,
            api_key,
            client_id,
        );

        // Encrypt the credentials JSON
        let creds_json = serde_json::to_string(&creds)?;
        let encrypted_creds = app.crypto.encrypt_text(&creds_json, key)?;
        let encrypted_json = serde_json::to_string(&encrypted_creds)?;
        let encrypted_b64 = base64::engine::general_purpose::STANDARD.encode(encrypted_json.as_bytes());

        // Format: jottery:v1:<salt_base64>.<encrypted_payload_base64>
        Ok(format!("jottery:v1:{}.{}", salt_b64, encrypted_b64))
    } else {
        anyhow::bail!("Database not available")
    }
}

/// Process credentials input from text
/// Supports both encrypted format (jottery:v1:...) and legacy unencrypted format
pub fn process_credentials_input(app: &mut App, input: &str) -> Result<()> {
    use base64::Engine;

    let input = input.trim();

    // Get database
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    // Check for encrypted format: jottery:v1:<salt>.<encrypted_payload>
    if input.starts_with("jottery:v1:") {
        let payload = &input["jottery:v1:".len()..];
        let dot_index = payload.find('.')
            .ok_or_else(|| anyhow::anyhow!("Invalid encrypted credentials format"))?;

        let salt_b64 = &payload[..dot_index];
        let encrypted_payload = &payload[dot_index + 1..];

        app.debug_log("Process credentials - Encrypted format detected");
        app.debug_log(&format!("Process credentials - Salt (base64): {}", salt_b64));

        // Decode and validate salt
        let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64)
            .context("Invalid base64 salt in encrypted credentials")?;

        if salt.len() < 32 {
            anyhow::bail!("Invalid salt length: {} bytes (expected at least 32 bytes)", salt.len());
        }

        // Update encryption metadata with the extracted salt
        let encryption_repo = EncryptionRepository::new(db.connection());
        encryption_repo.save(&salt, 100_000)?;
        app.debug_log("Process credentials - Salt saved successfully");

        // Store encrypted payload with marker for deferred decryption
        let sync_repo = SyncRepository::new(db.connection());
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
        metadata.api_key = Some(format!("ENCRYPTED:{}", encrypted_payload));
        metadata.sync_enabled = false; // Will be enabled after successful decryption
        sync_repo.update_metadata(&metadata)?;

        app.debug_log("Process credentials - Encrypted payload stored for deferred decryption");

        // Lock and force re-unlock to derive correct key from new salt
        app.key = None;
        app.notes.clear();
        app.selected_note = 0;
        app.password_input.clear();
        app.password_confirm.clear();
        app.password_confirm_focused = false;
        app.input_mode = InputMode::Normal;
        app.state = AppState::Locked;

        app.error = Some(t!("sync.encrypted_import").to_string());
        return Ok(());
    }

    // Legacy unencrypted format: base64(JSON)
    let creds = SyncCredentials::from_base64(input)
        .context("Invalid sync credentials format")?;

    app.debug_log(&format!("Process credentials - Legacy format - endpoint: {}", creds.endpoint));
    app.debug_log(&format!("Process credentials - client_id: {}", creds.client_id));
    app.debug_log(&format!("Process credentials - has salt: {}", creds.salt.is_some()));

    // If web app salt is provided, update it first
    if let Some(salt_b64) = &creds.salt {
        let encryption_repo = EncryptionRepository::new(db.connection());

        // Decode the base64 salt from web app
        let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64)
            .context("Invalid base64 salt from sync credentials")?;

        app.debug_log(&format!("Process credentials - Salt (base64): {}", salt_b64));
        app.debug_log(&format!("Process credentials - Salt (hex): {}", hex::encode(&salt)));
        app.debug_log(&format!("Process credentials - Salt length: {} bytes", salt.len()));

        // Validate salt length
        if salt.len() < 32 {
            anyhow::bail!("Invalid salt length: {} bytes (expected at least 32 bytes)", salt.len());
        }

        // Update encryption metadata with web app's salt AND iteration count
        app.debug_log("Process credentials - Saving salt with 100,000 iterations");
        encryption_repo.save(&salt, 100_000)?;
        app.debug_log("Process credentials - Salt saved successfully");
    }

    // Save sync metadata with PLAINTEXT API key temporarily
    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

    // Store API key as plaintext temporarily (will be encrypted on next unlock)
    app.debug_log("Process credentials - Storing API key (will encrypt on next unlock)");
    metadata.api_key = Some(format!("PLAINTEXT:{}", creds.api_key));
    metadata.client_id = Some(creds.client_id);
    metadata.sync_endpoint = creds.endpoint.clone();
    metadata.sync_enabled = true;

    sync_repo.update_metadata(&metadata)?;

    // Update settings
    app.settings.sync_endpoint = Some(creds.endpoint);
    app.settings.sync_enabled = true;
    save_settings(app)?;

    // If web app salt was provided, lock and force re-unlock
    if creds.salt.is_some() {
        app.debug_log("Process credentials - Locking database to force re-unlock with new salt");

        // Automatically lock the database
        app.key = None;
        app.notes.clear();
        app.selected_note = 0;
        app.password_input.clear();
        app.password_confirm.clear();
        app.password_confirm_focused = false;  // Ensure single password field on unlock
        app.input_mode = InputMode::Normal;
        app.state = AppState::Locked;

        // Show message about what happened
        app.error = Some(t!("sync.salt_sync").to_string());
    }

    Ok(())
}
