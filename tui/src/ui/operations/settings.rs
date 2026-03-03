//! Settings management operations

use anyhow::Result;
use rust_i18n::t;
use std::time::Instant;

use crate::{
    repository::{SettingsRepository, sync::SyncRepository},
};

use super::super::app::App;
use super::super::state::InputMode;

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
    // Copy key before borrowing app mutably
    let key = *app.key.as_ref().ok_or_else(|| anyhow::anyhow!("No encryption key"))?;

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    // Encrypt and save the API key
    let encrypted = app.crypto.encrypt_text(&response.api_key, &key)?;
    let encrypted_api_key = serde_json::to_string(&encrypted)?;

    // Save values before moving response fields
    let api_key_for_check = response.api_key.clone();
    let user_id_for_check = response.user_id.clone();

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

    // Update app.settings so the UI reflects the new state immediately
    app.settings.sync_endpoint = Some(endpoint.to_string());
    app.settings.sync_enabled = true;

    let local_password = app.unlock_password.clone();
    match try_envelope_setup(app, password, &key, endpoint, &api_key_for_check, &user_id_for_check, local_password.as_deref()) {
        Ok(()) => app.debug_log("Envelope setup succeeded after device registration"),
        Err(e) => app.debug_log(&format!("Envelope setup after registration failed (non-fatal): {}", e)),
    }

    Ok(())
}

/// Try to set up envelope encryption after device registration.
/// First attempts onboarding (downloading wrapped key from server),
/// then falls back to migrating local legacy encryption.
fn try_envelope_setup(
    app: &mut App,
    password: &str,
    master_key: &[u8; 32],
    endpoint: &str,
    plaintext_api_key: &str,
    user_id: &str,
    local_password: Option<&str>,
) -> Result<()> {
    // Try onboarding first (server may already have a wrapped key from another device)
    let client = crate::api::AuthClient::new(endpoint.to_string());
    match client.get_wrapped_key(plaintext_api_key)? {
        Some(_) => {
            // Server has a wrapped key — onboard from it
            app.debug_log("Server has wrapped key, onboarding...");
            let onboarded_key = super::envelope::onboard_from_server(app, password, endpoint, user_id, plaintext_api_key, local_password)?;
            // Verify onboarded key matches our current key
            if onboarded_key != *master_key {
                app.debug_log("Warning: onboarded master key differs from local key (expected for new device joining existing account)");
            }
            Ok(())
        }
        None => {
            // No wrapped key on server — migrate our local key
            app.debug_log("No wrapped key on server, migrating local...");
            super::envelope::try_migrate_to_envelope(app, password, master_key, local_password)
        }
    }
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
            if let Some(ref key) = metadata.api_key {
                return !key.starts_with("PLAINTEXT:") && !key.starts_with("ENCRYPTED:");
            }
        }
    }
    false
}
