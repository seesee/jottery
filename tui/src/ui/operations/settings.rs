//! Settings management operations

use anyhow::Result;
use rust_i18n::t;
use std::time::Instant;

use crate::{
    repository::{SettingsRepository, sync::SyncRepository},
};
#[cfg(test)]
use crate::db::Database;

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

/// Whether an address can be used as it stands. Plain HTTP to a public host would put the
/// account password and API key where anyone on the way can read them, so the first
/// attempt only warns. Entering the same address again means the user accepts that.
pub fn endpoint_confirmed(app: &mut App, endpoint: &str) -> bool {
    if crate::api::is_safe_endpoint(endpoint).unwrap_or(true) {
        return true;
    }

    if app.insecure_endpoint_warned.as_deref() == Some(endpoint) {
        return true;
    }

    app.insecure_endpoint_warned = Some(endpoint.to_string());
    app.error = Some(t!("register.insecure_endpoint").to_string());
    false
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
                    app.apply_auto_lock_setting();
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
                crate::api::is_safe_endpoint(&app.setting_input)?;
                app.settings.sync_endpoint = Some(app.setting_input.clone());
                point_sync_at(app, &app.setting_input.clone())?;
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

/// Sync reads its address from the stored metadata, so a new one has to go there too. The
/// pull position belongs to the old address.
fn point_sync_at(app: &mut App, endpoint: &str) -> Result<()> {
    let Some(db) = &app.db else {
        return Ok(());
    };

    let sync_repo = SyncRepository::new(db.connection());
    if let Some(mut metadata) = sync_repo.get_metadata()? {
        if metadata.sync_endpoint != endpoint {
            metadata.sync_endpoint = endpoint.to_string();
            metadata.pull_cursor = None;
            metadata.last_pull_at = None;
            sync_repo.update_metadata(&metadata)?;
        }
    }
    Ok(())
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

/// Whether the server already has an account for this email.
///
/// This is the check that doesn't take a password, on purpose. It's asked when the
/// password just typed is the notes password, to decide whether to refuse it, and the
/// checks that take one would hand it to the server before that's decided.
pub fn account_exists(endpoint: &str, email: &str) -> Result<bool> {
    Ok(crate::api::AuthClient::new(endpoint.to_string()).check_status(email)?.exists)
}

/// Register a new user account
/// Returns true if the user needs approval, false if already approved
pub fn register_user(app: &mut App, endpoint: &str, email: &str, password: &str) -> Result<bool> {
    // This is the first time the password would leave this device
    if !endpoint_confirmed(app, endpoint) {
        anyhow::bail!("{}", t!("register.insecure_endpoint"));
    }

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
                let status = client.check_own_status(email, password)?;
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
///
/// Nothing is saved locally until this device knows which key it will use, so a
/// refusal leaves it as it was, not half connected and pushing under the wrong key.
pub fn register_device(app: &mut App, endpoint: &str, email: &str, password: &str, device_name: &str) -> Result<()> {
    let notes_password = app.unlock_password.clone()
        .ok_or_else(|| anyhow::anyhow!("Vault is locked"))?;

    // Neither password is being chosen here, so a match can only be pointed out.
    // Refusing would stop this person adding a device at all.
    let passwords_match = crate::vault::check_passwords_differ(&notes_password, password).is_err();

    let local_key = app.key.ok_or_else(|| anyhow::anyhow!("No encryption key"))?;

    let client = crate::api::AuthClient::new(endpoint.to_string());

    app.debug_log(&format!("[Register] Starting device registration: endpoint={}, email={}", endpoint, email));

    // Register the device
    let response = client.register_device(email, password, device_name, "tui")?;
    app.debug_log(&format!("[Register] Server responded: user_id={}, client_id={}", response.user_id, response.client_id));

    let key = {
        let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
        let account = super::envelope::Account {
            client: &client,
            api_key: &response.api_key,
            user_id: &response.user_id,
        };
        super::envelope::adopt_account_key(db, &app.crypto, &account, &notes_password, &local_key)?
    };

    if key != local_key {
        app.debug_log("[Register] Now using the account's master key");
        app.key = Some(key);
        app.key_manager.set_master_key(key);
    }

    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

    // Encrypt and save the API key
    let encrypted = app.crypto.encrypt_text(&response.api_key, &key)?;
    let encrypted_api_key = serde_json::to_string(&encrypted)?;

    // Save sync metadata - also clear pending registration since we're now fully
    // registered. The first sync with an account pushes and pulls everything.
    let sync_repo = SyncRepository::new(db.connection());
    let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();
    metadata.api_key = Some(encrypted_api_key);
    metadata.client_id = Some(response.client_id);
    metadata.user_id = Some(response.user_id);
    metadata.sync_endpoint = endpoint.to_string();
    metadata.sync_enabled = true;
    metadata.pending_registration_email = None;
    metadata.last_sync_at = None;
    metadata.last_pull_at = None;
    metadata.pull_cursor = None;
    sync_repo.update_metadata(&metadata)?;
    app.debug_log("[Register] Sync metadata saved to database");

    // Update app.settings and persist to database
    app.settings.sync_endpoint = Some(endpoint.to_string());
    app.settings.sync_enabled = true;
    save_settings(app)?;
    app.debug_log("[Register] Settings persisted to database");

    super::notes::load_notes(app)?;

    if passwords_match {
        app.error = Some(t!("password.same_as_account_warning").to_string());
    }

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
    metadata.forget_server_state();
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app() -> App {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        std::mem::forget(dir);
        App::new(path, None).unwrap()
    }

    #[test]
    fn plain_http_to_a_public_host_has_to_be_entered_twice() {
        let mut app = app();

        assert!(!endpoint_confirmed(&mut app, "http://notes.example.com"));
        assert_eq!(app.error, Some(t!("register.insecure_endpoint").to_string()));

        assert!(endpoint_confirmed(&mut app, "http://notes.example.com"));
    }

    #[test]
    fn another_address_needs_its_own_confirmation() {
        let mut app = app();

        assert!(!endpoint_confirmed(&mut app, "http://first.example.com"));
        assert!(!endpoint_confirmed(&mut app, "http://second.example.com"));
    }

    #[test]
    fn a_safe_address_is_never_questioned() {
        let mut app = app();

        assert!(endpoint_confirmed(&mut app, "https://notes.example.com"));
        assert!(endpoint_confirmed(&mut app, "http://192.168.1.20:3030"));
        assert!(app.error.is_none());
    }

    /// The first attempt has to stop before the password goes anywhere. Nothing listens
    /// on this address, so a request would fail with a different message.
    #[test]
    fn registering_over_plain_http_warns_before_sending_the_password() {
        let mut app = app();
        let password = uuid::Uuid::new_v4().to_string();

        let error = register_user(&mut app, "http://notes.invalid", "someone@example.com", &password).unwrap_err();

        assert_eq!(error.to_string(), t!("register.insecure_endpoint"));
    }

    #[test]
    fn a_new_address_in_settings_reaches_sync_and_drops_the_pull_position() {
        let mut app = app();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        SyncRepository::new(db.connection()).update_metadata(&crate::models::sync::SyncMetadata {
            sync_endpoint: "https://old.example.com".to_string(),
            pull_cursor: Some("c9".to_string()),
            last_pull_at: Some(chrono::Utc::now()),
            ..Default::default()
        }).unwrap();
        app.db = Some(db);
        app.selected_setting = 5;
        app.setting_input = "https://new.example.com".to_string();

        save_setting_value(&mut app).unwrap();

        let metadata = app.sync_metadata().unwrap();
        assert_eq!(metadata.sync_endpoint, "https://new.example.com");
        assert_eq!(metadata.pull_cursor, None);
        assert_eq!(metadata.last_pull_at, None);
    }
}
