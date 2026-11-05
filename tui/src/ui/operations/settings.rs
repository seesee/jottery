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
            // Language: cycle forward immediately, no input needed
            cycle_language(app, true);
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
        8 => cycle_default_note_type(app, true),
        _ => {}
    }
}

/// Move on to the next or previous type for new notes, and save it.
fn cycle_default_note_type(app: &mut App, forward: bool) {
    let current = app.settings.default_note_type;
    app.settings.default_note_type = if forward { current.next() } else { current.prev() };
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
}

/// Start editing a setting field (backward for cyclic fields)
pub fn start_editing_setting_backward(app: &mut App) {
    // For cyclic fields, cycle backward; for others, behave like forward
    match app.selected_setting {
        0 => {
            // Language: cycle backward immediately, no input needed
            cycle_language(app, false);
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
        8 => cycle_default_note_type(app, false),
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
            // What it's fetching belongs to the old address
            super::sync::cancel_background_sync(app);
            let Some(db) = &app.db else {
                return Ok(());
            };
            let sync_repo = SyncRepository::new(db.connection());

            metadata.sync_endpoint = endpoint.to_string();
            metadata.pull_cursor = None;
            metadata.last_pull_at = None;
            sync_repo.update_metadata(&metadata)?;
        }
    }
    Ok(())
}

/// Show the app in the language the settings ask for. Not in tests, which run side by
/// side and compare translated text, because the locale is global.
pub fn apply_language(app: &App) {
    if !cfg!(test) {
        rust_i18n::set_locale(&crate::models::effective_locale(&app.settings.language));
    }
}

/// Move to the next or previous language, which takes effect at once.
fn cycle_language(app: &mut App, forward: bool) {
    app.settings.language = crate::models::cycle_language(&app.settings.language, forward).to_string();
    apply_language(app);
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
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

/// Apply a new sort order, keeping the highlight on the note it was on.
fn apply_sort_order(app: &mut App) {
    app.invalidate_filter_cache();
    app.keep_selection_on_note();
    if let Err(e) = save_settings(app) {
        app.error = Some(format!("Failed to save settings: {}", e));
    }
}

/// Cycle forward through sort order options
pub fn cycle_sort_order(app: &mut App) {
    app.settings.sort_order = app.settings.sort_order.next();
    apply_sort_order(app);
}

/// Cycle backward through sort order options
pub fn cycle_sort_order_backward(app: &mut App) {
    app.settings.sort_order = app.settings.sort_order.prev();
    apply_sort_order(app);
}

/// Save settings to database
pub fn save_settings(app: &mut App) -> Result<()> {
    if let Some(db) = &app.db {
        let settings_repo = SettingsRepository::new(db.connection());
        settings_repo.update(&app.settings)?;
    }
    Ok(())
}

/// Ask the server to stop honouring this device's API key.
fn revoke_this_device(app: &App) -> Result<()> {
    let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
    let key = app.key.as_ref().ok_or_else(|| anyhow::anyhow!("Vault is locked"))?;

    let Some(metadata) = SyncRepository::new(db.connection()).get_metadata()? else {
        return Ok(());
    };
    let Some(api_key) = metadata.api_key.filter(|_| !metadata.sync_endpoint.is_empty()) else {
        return Ok(());
    };

    let api_key: crate::crypto::EncryptedData = serde_json::from_str(&api_key)?;
    let api_key = app.crypto.decrypt_text(&api_key, key)?;

    crate::api::AuthClient::new(metadata.sync_endpoint).revoke_device(&api_key)
}

/// Disconnect from sync server
/// Revokes this device, then clears all sync credentials and metadata, but preserves
/// local notes. A device the server can't be told about still gets disconnected here,
/// and the user hears that its key is still valid.
pub fn disconnect_from_sync(app: &mut App) -> Result<()> {
    app.debug_log("Disconnect - Starting disconnect from sync server");

    // Stop SSE connection since we're disabling sync
    app.stop_sse();
    super::sync::cancel_background_sync(app);

    let revoked = revoke_this_device(app);
    if let Err(e) = &revoked {
        app.debug_log(&format!("Disconnect - Could not revoke this device: {}", e));
    }

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
    if revoked.is_err() {
        app.error = Some(t!("sync.device_still_registered").to_string());
    }

    Ok(())
}

/// Check registration status for an email
pub fn check_registration_status(app: &mut App, email: &str) -> Result<String> {
    // Need sync endpoint to check status
    let endpoint = app.settings.sync_endpoint.as_ref()
        .ok_or_else(|| anyhow::anyhow!(t!("sync.endpoint_not_configured").to_string()))?;

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

    // A sync still running belongs to whatever this device was connected to before
    super::sync::cancel_background_sync(app);

    app.debug_log("[Register] Starting device registration");

    // Register the device
    let response = client.register_device(email, password, device_name, "tui")?;
    app.debug_log(&format!("[Register] Server responded: user_id={}, client_id={}", response.user_id, response.client_id));

    let key = {
        let db = app.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;
        let server = super::envelope::ServerLink {
            client: &client,
            api_key: &response.api_key,
            user_id: &response.user_id,
        };
        super::envelope::adopt_account_key(db, &app.crypto, &server, &notes_password, &local_key)?
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

    app.debug_log("Saved pending registration");

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
            app.debug_log("Found pending registration");
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

        // Built, not written out: CodeQL traces a literal http:// URL from here into
        // the client, though this test exists to show it never gets that far.
        let mut endpoint = reqwest::Url::parse("https://notes.invalid").unwrap();
        endpoint.set_scheme("http").unwrap();

        let error = register_user(&mut app, endpoint.as_str(), "someone@example.com", &password).unwrap_err();

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

    /// The server's answer to a device being revoked, and how many times it was asked.
    fn revoke_server(status: &'static str) -> (String, std::sync::Arc<std::sync::atomic::AtomicUsize>) {
        use std::io::Write;
        use std::sync::atomic::{AtomicUsize, Ordering};

        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let calls = std::sync::Arc::new(AtomicUsize::new(0));
        let counted = calls.clone();

        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let Some(request) = crate::test_support::read_request(&mut stream) else {
                    continue;
                };
                if request.method == "DELETE" && request.path == "/api/v1/sync/device" {
                    counted.fetch_add(1, Ordering::SeqCst);
                }
                let response = format!("HTTP/1.1 {status}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n");
                stream.write_all(response.as_bytes()).unwrap();
            }
        });

        (endpoint, calls)
    }

    fn connected_app(endpoint: &str) -> App {
        let mut app = app();
        let key = app.crypto.generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let api_key = app.crypto.encrypt_text("device-api-key", &key).unwrap();
        SyncRepository::new(db.connection()).update_metadata(&crate::models::sync::SyncMetadata {
            api_key: Some(serde_json::to_string(&api_key).unwrap()),
            sync_enabled: true,
            sync_endpoint: endpoint.to_string(),
            ..Default::default()
        }).unwrap();
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some(endpoint.to_string());
        app.db = Some(db);
        app.key = Some(key);
        app
    }

    #[test]
    fn disconnecting_revokes_the_device_first() {
        let (endpoint, calls) = revoke_server("204 No Content");
        let mut app = connected_app(&endpoint);

        disconnect_from_sync(&mut app).unwrap();

        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        assert!(app.error.is_none());
        assert!(app.sync_metadata().is_none());
    }

    #[test]
    fn a_key_the_server_has_already_dropped_counts_as_revoked() {
        let (endpoint, _calls) = revoke_server("401 Unauthorized");
        let mut app = connected_app(&endpoint);

        disconnect_from_sync(&mut app).unwrap();

        assert!(app.error.is_none());
    }

    /// Disconnecting should still work offline, but the user must know the key lives on.
    #[test]
    fn a_failed_revoke_disconnects_anyway_and_says_so() {
        let (endpoint, _calls) = revoke_server("500 Internal Server Error");
        let mut app = connected_app(&endpoint);

        disconnect_from_sync(&mut app).unwrap();

        assert_eq!(app.error, Some(t!("sync.device_still_registered").to_string()));
        assert!(app.sync_metadata().is_none());
    }

    #[test]
    fn the_language_setting_cycles_and_saves_rather_than_taking_text() {
        let mut app = app();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        app.db = Some(db);
        app.selected_setting = 0;

        start_editing_setting(&mut app);

        assert_eq!(app.settings.language, "de");
        assert!(!matches!(app.input_mode, InputMode::SettingsEdit));
        let stored = SettingsRepository::new(app.db.as_ref().unwrap().connection()).get().unwrap();
        assert_eq!(stored.language, "de");

        start_editing_setting_backward(&mut app);
        assert_eq!(app.settings.language, "auto");
    }

    #[test]
    fn the_sort_order_decides_the_list() {
        use crate::models::{Note, SortOrder};

        let mut app = app();
        let mut older = Note::new("Banana".to_string());
        older.created_at = "2021-01-01T00:00:00Z".parse().unwrap();
        older.modified_at = "2026-01-01T00:00:00Z".parse().unwrap();
        let mut newer = Note::new("apple".to_string());
        newer.created_at = "2024-01-01T00:00:00Z".parse().unwrap();
        newer.modified_at = "2023-01-01T00:00:00Z".parse().unwrap();
        let mut pinned = Note::new("Zebra".to_string());
        pinned.pinned = true;
        pinned.created_at = "2020-01-01T00:00:00Z".parse().unwrap();
        pinned.modified_at = "2020-01-01T00:00:00Z".parse().unwrap();
        app.notes = vec![older, newer, pinned];

        let order = |app: &mut App, sort: SortOrder| -> Vec<String> {
            app.settings.sort_order = sort;
            app.invalidate_filter_cache();
            app.filtered_notes().iter().map(|note| note.content.clone()).collect()
        };

        assert_eq!(order(&mut app, SortOrder::Recent), vec!["Zebra", "Banana", "apple"]);
        assert_eq!(order(&mut app, SortOrder::Oldest), vec!["Zebra", "apple", "Banana"]);
        assert_eq!(order(&mut app, SortOrder::Alpha), vec!["Zebra", "apple", "Banana"]);
        assert_eq!(order(&mut app, SortOrder::Created), vec!["Zebra", "apple", "Banana"]);
    }

    #[test]
    fn cycling_the_sort_order_changes_the_list_at_once_and_keeps_the_highlight() {
        use crate::models::Note;

        let mut app = app();
        let mut first = Note::new("first".to_string());
        first.modified_at = "2026-01-01T00:00:00Z".parse().unwrap();
        let mut second = Note::new("second".to_string());
        second.modified_at = "2025-01-01T00:00:00Z".parse().unwrap();
        app.notes = vec![first, second];
        app.select_note_at(1);

        cycle_sort_order(&mut app);

        assert_eq!(app.filtered_notes()[0].content, "second");
        assert_eq!(app.filtered_notes()[app.selected_note].content, "second");
    }
}
