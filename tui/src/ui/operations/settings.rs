//! Settings management operations

use anyhow::{Context, Result};
use rust_i18n::t;
use std::time::Instant;
use zeroize::Zeroize;

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
        app.debug_log(&format!("Paste credentials - Saving salt with {} iterations", crate::crypto::DEFAULT_ITERATIONS));
        encryption_repo.save(&salt, crate::crypto::DEFAULT_ITERATIONS)?;
        app.debug_log("Paste credentials - Salt saved successfully");
    }

    // Save sync metadata
    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

    // If salt was synced, we need to store plaintext and encrypt on next unlock
    // because the key will be re-derived with the new salt.
    // Otherwise, encrypt immediately with the current session key if available.
    let api_key_to_store = if creds.salt.is_some() {
        // Salt changed - must wait for next unlock to encrypt with new key
        app.debug_log("Paste credentials - Storing API key (will encrypt on next unlock with new salt)");
        format!("PLAINTEXT:{}", creds.api_key)
    } else if let Some(ref key) = app.key {
        // No salt change and we have a session key - encrypt immediately
        app.debug_log("Paste credentials - Encrypting API key immediately");
        let encrypted = app.crypto.encrypt_text(&creds.api_key, key)
            .context("Failed to encrypt API key")?;
        serde_json::to_string(&encrypted)
            .context("Failed to serialize encrypted API key")?
    } else {
        // No session key available (shouldn't happen in normal flow)
        app.debug_log("Paste credentials - No session key, storing plaintext temporarily");
        format!("PLAINTEXT:{}", creds.api_key)
    };

    metadata.api_key = Some(api_key_to_store);
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

        // Stop SSE connection before locking
        app.stop_sse();

        // Automatically lock the database
        app.key = None;
        app.notes.clear();
        app.selected_note = 0;
        app.password_input.zeroize();
        app.password_confirm.zeroize();
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

/// Generate sync credentials text
/// - use_legacy_format=false: encrypted format jottery:v1:<salt>.<encrypted_payload>
/// - use_legacy_format=true: legacy unencrypted base64 JSON (for older Jottery versions)
pub fn generate_sync_credentials_text(app: &App, use_legacy_format: bool) -> Result<String> {
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

        use base64::Engine;

        // Check if envelope encryption is active — use v2 format (just endpoint)
        if !use_legacy_format && encryption_meta.envelope_version.is_some() {
            return Ok(format!("jottery:v2:{}", metadata.sync_endpoint));
        }

        if use_legacy_format {
            // Legacy format: plain base64 JSON with salt inside (for older Jottery versions)
            let salt_b64 = base64::engine::general_purpose::STANDARD.encode(encryption_meta.salt.as_deref().unwrap_or(&[]));
            let mut creds = SyncCredentials::new(
                metadata.sync_endpoint,
                api_key,
                client_id,
            );
            creds.salt = Some(salt_b64);
            creds.to_base64()
        } else {
            // Encrypted format: jottery:v1:<salt_base64>.<encrypted_payload_base64>
            let salt_b64 = base64::engine::general_purpose::STANDARD.encode(encryption_meta.salt.as_deref().unwrap_or(&[]));

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
        }
    } else {
        anyhow::bail!("Database not available")
    }
}

/// Process credentials input with a specified device name
/// This stores the device name for use during the clone-device call on unlock
pub fn process_credentials_input_with_device_name(app: &mut App, input: &str, device_name: &str) -> Result<()> {
    use base64::Engine;

    let input = input.trim();

    // Get database
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    // Check for v2 format: jottery:v2:<endpoint> (envelope encryption — just needs endpoint)
    if let Some(endpoint) = input.strip_prefix("jottery:v2:") {
        let endpoint = endpoint.trim().to_string();
        if endpoint.is_empty() {
            anyhow::bail!("Invalid v2 credentials: missing endpoint");
        }

        let endpoint = if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
            format!("https://{}", endpoint)
        } else {
            endpoint
        };

        app.debug_log(&format!("Process credentials - v2 format, endpoint: {}", endpoint));
        app.debug_log(&format!("Process credentials - Device name: {}", device_name));

        // Store endpoint and ONBOARD marker — the actual onboarding happens during
        // device registration (email + password needed to derive wrapping key)
        let sync_repo = SyncRepository::new(db.connection());
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
        metadata.api_key = Some("ONBOARD:".to_string());
        metadata.sync_endpoint = endpoint.clone();
        metadata.sync_enabled = false;
        metadata.pending_device_name = Some(device_name.to_string());
        sync_repo.update_metadata(&metadata)?;

        // Update app settings with the endpoint
        app.settings.sync_endpoint = Some(endpoint);
        app.settings.sync_enabled = false;
        save_settings(app)?;

        app.sync_status = Some(t!("sync.v2_import_success").to_string());
        app.sync_status_set_at = Some(std::time::Instant::now());

        return Ok(());
    }

    // Check for encrypted format: jottery:v1:<salt>.<encrypted_payload>
    if let Some(payload) = input.strip_prefix("jottery:v1:") {
        let dot_index = payload.find('.')
            .ok_or_else(|| anyhow::anyhow!("Invalid encrypted credentials format"))?;

        let salt_b64 = &payload[..dot_index];
        let encrypted_payload = &payload[dot_index + 1..];

        app.debug_log("Process credentials - Encrypted format detected");
        app.debug_log(&format!("Process credentials - Salt (base64): {}", salt_b64));
        app.debug_log(&format!("Process credentials - Device name: {}", device_name));

        // Decode and validate salt
        let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64)
            .context("Invalid base64 salt in encrypted credentials")?;

        if salt.len() < 32 {
            anyhow::bail!("Invalid salt length: {} bytes (expected at least 32 bytes)", salt.len());
        }

        // Update encryption metadata with the extracted salt
        let encryption_repo = EncryptionRepository::new(db.connection());
        encryption_repo.save(&salt, crate::crypto::DEFAULT_ITERATIONS)?;
        app.debug_log("Process credentials - Salt saved successfully");

        // Store encrypted payload with marker for deferred decryption AND device name
        let sync_repo = SyncRepository::new(db.connection());
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
        metadata.api_key = Some(format!("ENCRYPTED:{}", encrypted_payload));
        metadata.sync_enabled = false; // Will be enabled after successful clone-device
        metadata.pending_device_name = Some(device_name.to_string());
        sync_repo.update_metadata(&metadata)?;

        app.debug_log("Process credentials - Encrypted payload stored for deferred decryption");

        // Stop SSE connection before locking
        app.stop_sse();

        // Lock and force re-unlock to derive correct key from new salt
        app.key = None;
        app.notes.clear();
        app.selected_note = 0;
        app.password_input.zeroize();
        app.password_confirm.zeroize();
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
    app.debug_log(&format!("Process credentials - Device name: {}", device_name));

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
        app.debug_log(&format!("Process credentials - Saving salt with {} iterations", crate::crypto::DEFAULT_ITERATIONS));
        encryption_repo.save(&salt, crate::crypto::DEFAULT_ITERATIONS)?;
        app.debug_log("Process credentials - Salt saved successfully");
    }

    // Save sync metadata with PLAINTEXT marker for clone-device on unlock
    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    metadata.api_key = Some(format!("PLAINTEXT:{}", creds.api_key));
    metadata.sync_endpoint = creds.endpoint.clone();
    metadata.sync_enabled = false; // Will be enabled after successful clone-device
    metadata.pending_device_name = Some(device_name.to_string());
    sync_repo.update_metadata(&metadata)?;

    // Update settings
    app.settings.sync_endpoint = Some(creds.endpoint);
    app.settings.sync_enabled = false; // Will be enabled after clone-device
    save_settings(app)?;

    // Lock and force re-unlock with the new salt
    app.debug_log("Process credentials - Locking database to force re-unlock with new salt");

    // Stop SSE connection before locking
    app.stop_sse();

    app.key = None;
    app.notes.clear();
    app.selected_note = 0;
    app.password_input.zeroize();
    app.password_confirm.zeroize();
    app.password_confirm_focused = false;
    app.input_mode = InputMode::Normal;
    app.state = AppState::Locked;

    // Show message about what happened
    app.error = Some(t!("sync.salt_sync").to_string());

    Ok(())
}

/// Disconnect from sync server
/// Clears all sync credentials and metadata, but preserves local notes
pub fn disconnect_from_sync(app: &mut App) -> Result<()> {
    app.debug_log("Disconnect - Starting disconnect from sync server");

    // Stop SSE connection since we're disabling sync
    app.stop_sse();

    // Get database
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    // Clear sync metadata (credentials, client ID, etc.)
    let sync_repo = SyncRepository::new(db.connection());
    sync_repo.clear_all()?;

    app.debug_log("Disconnect - Sync metadata cleared");

    // Update settings to disable sync
    app.settings.sync_enabled = false;
    app.settings.sync_endpoint = None;
    save_settings(app)?;

    app.debug_log("Disconnect - Settings updated");

    // Show success message
    app.sync_status = Some(t!("sync.disconnected").to_string());
    app.sync_status_set_at = Some(Instant::now());

    Ok(())
}

/// Check registration status for an email
pub fn check_registration_status(app: &mut App, email: &str) -> Result<String> {
    // Need sync endpoint to check status
    let endpoint = app.settings.sync_endpoint.as_ref()
        .ok_or_else(|| anyhow::anyhow!(t!("sync.endpoint_not_configured").to_string()))?;

    app.debug_log(&format!("Checking registration status for: {}", email));

    let client = crate::api::AuthClient::new(endpoint.clone());
    let status = client.check_status(email)?;

    let message = if !status.exists {
        t!("sync.status.not_found").to_string()
    } else if status.is_approved && status.is_active {
        t!("sync.status.approved").to_string()
    } else if !status.is_approved {
        t!("sync.status.pending").to_string()
    } else {
        t!("sync.status.inactive").to_string()
    };

    Ok(message)
}

/// Check registration status with a specific endpoint
pub fn check_registration_status_with_endpoint(_app: &mut App, endpoint: &str, email: &str) -> Result<bool> {
    let client = crate::api::AuthClient::new(endpoint.to_string());
    let status = client.check_status(email)?;
    Ok(status.exists && status.is_approved && status.is_active)
}

/// Register a new user account
/// Returns true if the user needs approval, false if already approved
pub fn register_user(_app: &mut App, endpoint: &str, email: &str, password: &str) -> Result<bool> {
    let client = crate::api::AuthClient::new(endpoint.to_string());

    // Try to register the user
    match client.register_user(email, password) {
        Ok(response) => {
            // Check if user needs approval
            Ok(response.status == "pending_approval")
        }
        Err(e) => {
            let error_msg = e.to_string();
            // If user already exists, check their status
            if error_msg.contains("already exists") || error_msg.contains("409") {
                let status = client.check_status(email)?;
                if status.is_approved && status.is_active {
                    // User exists and is approved, can proceed to device registration
                    Ok(false)
                } else if !status.is_approved {
                    // User exists but still pending
                    Ok(true)
                } else {
                    anyhow::bail!("Account is inactive. Please contact administrator.")
                }
            } else {
                Err(e)
            }
        }
    }
}

/// Register a device with the sync server
pub fn register_device(app: &mut App, endpoint: &str, email: &str, password: &str, device_name: &str) -> Result<()> {
    let client = crate::api::AuthClient::new(endpoint.to_string());

    // Register the device
    let response = client.register_device(email, password, device_name, "tui")?;

    // Get database
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("No encryption key"))?;

    // Encrypt and save the API key
    let encrypted = app.crypto.encrypt_text(&response.api_key, key)?;
    let encrypted_api_key = serde_json::to_string(&encrypted)?;

    // Save sync metadata - also clear pending registration since we're now fully registered
    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    metadata.api_key = Some(encrypted_api_key);
    metadata.client_id = Some(response.client_id);
    metadata.user_id = Some(response.user_id);
    metadata.sync_endpoint = endpoint.to_string();
    metadata.sync_enabled = true;
    metadata.pending_registration_email = None; // Clear pending registration
    sync_repo.update_metadata(&metadata)?;

    // Update app settings
    app.settings.sync_endpoint = Some(endpoint.to_string());
    app.settings.sync_enabled = true;
    save_settings(app)?;

    Ok(())
}

/// Save pending registration state to database
pub fn save_pending_registration(app: &mut App, endpoint: &str, email: &str) -> Result<()> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    metadata.sync_endpoint = endpoint.to_string();
    metadata.pending_registration_email = Some(email.to_string());
    sync_repo.update_metadata(&metadata)?;

    app.debug_log(&format!("Saved pending registration: endpoint={}, email={}", endpoint, email));

    Ok(())
}

/// Get pending registration if one exists
pub fn get_pending_registration(app: &mut App) -> Result<Option<(String, String)>> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    let sync_repo = SyncRepository::new(db.connection());
    if let Some(metadata) = sync_repo.get_metadata()? {
        // Check if there's a pending registration (endpoint set, pending email set, but no API key)
        if !metadata.sync_endpoint.is_empty() && metadata.pending_registration_email.is_some() && metadata.api_key.is_none() {
            let endpoint = metadata.sync_endpoint;
            let email = metadata.pending_registration_email.unwrap();
            app.debug_log(&format!("Found pending registration: endpoint={}, email={}", endpoint, email));
            return Ok(Some((endpoint, email)));
        }
    }

    Ok(None)
}

/// Clear pending registration state (and all sync metadata)
pub fn clear_pending_registration(app: &mut App) -> Result<()> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    metadata.pending_registration_email = None;
    metadata.sync_endpoint = String::new();
    metadata.api_key = None;
    metadata.client_id = None;
    metadata.user_email = None;
    metadata.user_id = None;
    metadata.sync_enabled = false;
    sync_repo.update_metadata(&metadata)?;

    // Also clear app settings
    app.settings.sync_endpoint = None;
    app.settings.sync_enabled = false;
    save_settings(app)?;

    app.debug_log("Cleared pending registration and sync settings");

    Ok(())
}

/// Check if there's a pending registration
pub fn has_pending_registration(app: &mut App) -> bool {
    get_pending_registration(app).ok().flatten().is_some()
}

/// Check if sync is fully configured (has API key)
pub fn is_sync_fully_configured(app: &mut App) -> bool {
    if let Some(db) = &app.db {
        let sync_repo = SyncRepository::new(db.connection());
        if let Ok(Some(metadata)) = sync_repo.get_metadata() {
            return metadata.api_key.is_some();
        }
    }
    false
}
