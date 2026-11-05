//! Input handling for sync credentials screens

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use rust_i18n::t;
use zeroize::{Zeroize, Zeroizing};

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::{AppState, InputMode};

/// Handle key events for change password - current password input
pub fn handle_change_password_current_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::ChangePasswordCurrent { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let current_password = Zeroizing::new(app.credential_input.clone());
            if current_password.is_empty() {
                app.error = Some(t!("password.change_current_required").to_string());
                return Ok(());
            }
            // Verify current password
            match operations::auth::verify_password(app, &current_password) {
                Ok(true) => {
                    app.credential_input.zeroize();
                    if let AppState::ChangePasswordCurrent { previous } =
                        std::mem::replace(&mut app.state, AppState::Quit)
                    {
                        app.state = AppState::ChangePasswordNew { current_password, previous };
                    }
                }
                Ok(false) | Err(_) => {
                    app.error = Some(t!("password.change_wrong_current").to_string());
                }
            }
        }
        KeyCode::Char(c) => { app.credential_input.push(c); }
        KeyCode::Backspace => { app.credential_input.pop(); }
        _ => {}
    }
    Ok(())
}

/// Handle key events for change password - new password input
pub fn handle_change_password_new_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::ChangePasswordNew { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let new_password = Zeroizing::new(app.credential_input.clone());
            if crate::vault::check_new_password(&new_password).is_err() {
                app.error = Some(t!("password.change_too_short").to_string());
                return Ok(());
            }
            app.credential_input.zeroize();
            if let AppState::ChangePasswordNew { current_password, previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = AppState::ChangePasswordConfirm { current_password, new_password, previous };
            }
        }
        KeyCode::Char(c) => { app.credential_input.push(c); }
        KeyCode::Backspace => { app.credential_input.pop(); }
        _ => {}
    }
    Ok(())
}

/// Handle key events for change password - confirm new password
pub fn handle_change_password_confirm_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::ChangePasswordConfirm { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let confirm = Zeroizing::new(app.credential_input.clone());
            app.credential_input.zeroize();

            if let AppState::ChangePasswordConfirm { current_password, new_password, previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                if confirm != new_password {
                    app.error = Some(t!("password.change_mismatch").to_string());
                    app.state = *previous;
                    app.input_mode = InputMode::Normal;
                    return Ok(());
                }
                let master_key = match app.key {
                    Some(k) => k,
                    None => {
                        app.state = *previous;
                        app.input_mode = InputMode::Normal;
                        return Ok(());
                    }
                };
                // Re-wrap the master key first. If that fails nothing has changed,
                // whereas changing the database password first could leave the two
                // disagreeing, with no password that opens both.
                if let Err(e) = operations::envelope::change_password(app, &current_password, &new_password, &master_key) {
                    app.error = Some(format!("{}: {}", t!("password.change_failed"), e));
                    app.state = *previous;
                    app.input_mode = InputMode::Normal;
                    return Ok(());
                }
                // Change SQLCipher database password
                if let Some(db) = &app.db {
                    if let Err(e) = db.change_password(&new_password) {
                        // Put the wrapped key back, so the old password opens both
                        if let Err(undo) = operations::envelope::change_password(app, &new_password, &current_password, &master_key) {
                            app.debug_log(&format!("Could not restore the wrapped key: {}", undo));
                        }
                        app.error = Some(format!("{}: {}", t!("password.change_failed"), e));
                        app.state = *previous;
                        app.input_mode = InputMode::Normal;
                        return Ok(());
                    }
                }
                app.unlock_password = Some(new_password.clone());
                // Update stored password if remember is enabled
                if app.settings.remember_password {
                    let _ = operations::auth::store_password_for_autounlock(app, &new_password);
                }
                app.sync_status = Some(t!("password.change_success").to_string());
                app.sync_status_set_at = Some(std::time::Instant::now());
                app.state = *previous;
                app.input_mode = InputMode::Normal;
            }
        }
        KeyCode::Char(c) => { app.credential_input.push(c); }
        KeyCode::Backspace => { app.credential_input.pop(); }
        _ => {}
    }
    Ok(())
}

/// Handle key events when inputting email for status check
pub fn handle_input_email_for_status_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Cancel input
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::InputEmailForStatus { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            // Check the status
            let email = app.credential_input.trim().to_string();
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;

            // Return to previous state first
            let previous = if let AppState::InputEmailForStatus { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                previous
            } else {
                Box::new(AppState::NoteList)
            };

            // Try to check status
            match operations::settings::check_registration_status(app, &email) {
                Ok(status_message) => {
                    app.state = AppState::ShowRegistrationStatus {
                        status_message,
                        previous,
                    };
                }
                Err(e) => {
                    app.state = *previous;
                    app.error = Some(format!("{}: {}", t!("sync.status.check_failed"), e));
                }
            }
        }
        KeyCode::Char(c) => {
            app.credential_input.push(c);
        }
        KeyCode::Backspace => {
            app.credential_input.pop();
        }
        _ => {}
    }
    Ok(())
}

/// Handle key events when showing registration status
pub fn handle_show_registration_status_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
            // Return to previous state
            if let AppState::ShowRegistrationStatus { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle key events when inputting sync server endpoint for registration
pub fn handle_register_input_endpoint_key(app: &mut App, key: KeyEvent) -> Result<()> {
    app.error = None;

    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::RegisterInputEndpoint { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let endpoint = app.credential_input.trim().to_string();
            if endpoint.is_empty() {
                app.error = Some(t!("register.endpoint_required").to_string());
                return Ok(());
            }
            app.credential_input.zeroize();

            // Validate endpoint format
            let endpoint = if !endpoint.starts_with("http://") && !endpoint.starts_with("https://") {
                format!("https://{}", endpoint)
            } else {
                endpoint
            };

            // Move to email input
            let previous = if let AppState::RegisterInputEndpoint { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                previous
            } else {
                Box::new(AppState::NoteList)
            };

            app.state = AppState::RegisterInputEmail { endpoint, previous };
        }
        KeyCode::Char(c) => {
            app.credential_input.push(c);
        }
        KeyCode::Backspace => {
            app.credential_input.pop();
        }
        _ => {}
    }
    Ok(())
}

/// Handle key events when inputting email for registration
pub fn handle_register_input_email_key(app: &mut App, key: KeyEvent) -> Result<()> {
    app.error = None;

    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::RegisterInputEmail { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let email = app.credential_input.trim().to_string();
            if email.is_empty() || !email.contains('@') {
                app.error = Some(t!("register.email_invalid").to_string());
                return Ok(());
            }
            app.credential_input.zeroize();

            // Move to password input
            if let AppState::RegisterInputEmail { endpoint, previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = AppState::RegisterInputPassword { endpoint, email, previous };
            }
        }
        KeyCode::Char(c) => {
            app.credential_input.push(c);
        }
        KeyCode::Backspace => {
            app.credential_input.pop();
        }
        _ => {}
    }
    Ok(())
}

/// Refuse an account password that's being chosen now, if it's the notes password.
///
/// This prompt also takes the password of an account that already exists, where the
/// match is old news and refusing would stop the device joining. Connecting warns about
/// that instead. If the server can't say the account exists, it's treated as new.
fn check_account_password(app: &mut App, password: &str) -> Result<()> {
    let Some(notes_password) = app.unlock_password.as_deref() else {
        return Ok(());
    };
    let refusal = crate::vault::check_passwords_differ(notes_password, password);
    if refusal.is_ok() {
        return Ok(());
    }

    let AppState::RegisterInputPassword { endpoint, email, .. } = &app.state else {
        return refusal;
    };
    let (endpoint, email) = (endpoint.clone(), email.clone());

    match operations::settings::account_exists(&endpoint, &email) {
        Ok(true) => Ok(()),
        _ => refusal,
    }
}

/// Handle key events when inputting password for registration
pub fn handle_register_input_password_key(app: &mut App, key: KeyEvent) -> Result<()> {
    app.error = None;

    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::RegisterInputPassword { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let password = Zeroizing::new(app.credential_input.clone());
            if password.len() < 12 {
                app.error = Some(t!("register.password_too_short").to_string());
                return Ok(());
            }
            if let Err(e) = check_account_password(app, &password) {
                app.credential_input.zeroize();
                app.error = Some(e.to_string());
                return Ok(());
            }
            app.credential_input.zeroize();

            // Try to register user
            if let AppState::RegisterInputPassword { endpoint, email, previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                match operations::settings::register_user(app, &endpoint, &email, &password) {
                    Ok(needs_approval) => {
                        if needs_approval {
                            // User registered but needs approval - save pending state to database
                            if let Err(e) = operations::settings::save_pending_registration(app, &endpoint, &email) {
                                app.debug_log(&format!("Failed to save pending registration: {}", e));
                            }
                            app.state = AppState::RegisterPendingApproval { endpoint, email, previous };
                            app.input_mode = InputMode::Normal;
                        } else {
                            // User already approved, proceed to device registration
                            app.state = AppState::RegisterInputDeviceName { endpoint, email, password, previous };
                        }
                    }
                    Err(e) => {
                        app.state = *previous;
                        app.input_mode = InputMode::Normal;
                        app.error = Some(format!("{}: {}", t!("register.failed"), e));
                    }
                }
            }
        }
        KeyCode::Char(c) => {
            app.credential_input.push(c);
        }
        KeyCode::Backspace => {
            app.credential_input.pop();
        }
        _ => {}
    }
    Ok(())
}

/// Handle key events when waiting for registration approval
pub fn handle_register_pending_approval_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            // Cancel and return to previous state
            if let AppState::RegisterPendingApproval { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Char('r') | KeyCode::Enter => {
            // Check approval status
            if let AppState::RegisterPendingApproval { ref endpoint, ref email, .. } = app.state {
                let endpoint = endpoint.clone();
                let email = email.clone();

                match operations::settings::check_registration_status_with_endpoint(app, &endpoint, &email) {
                    Ok(approved) => {
                        if approved {
                            // Approved! Move to password input for device registration
                            // Need to get password again since we don't store it
                            app.credential_input.zeroize();
                            app.sync_status = Some(t!("register.approved").to_string());
                            app.sync_status_set_at = Some(std::time::Instant::now());

                            if let AppState::RegisterPendingApproval { endpoint, email, previous } =
                                std::mem::replace(&mut app.state, AppState::Quit)
                            {
                                // Go back to password input to complete device registration
                                app.state = AppState::RegisterInputPassword { endpoint, email, previous };
                                app.input_mode = InputMode::Insert;
                            }
                        } else {
                            app.sync_status = Some(t!("register.still_pending").to_string());
                            app.sync_status_set_at = Some(std::time::Instant::now());
                        }
                    }
                    Err(e) => {
                        app.error = Some(format!("{}: {}", t!("sync.status.check_failed"), e));
                    }
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle key events when inputting device name for registration
pub fn handle_register_input_device_name_key(app: &mut App, key: KeyEvent) -> Result<()> {
    app.error = None;

    match key.code {
        KeyCode::Esc => {
            app.credential_input.zeroize();
            app.input_mode = InputMode::Normal;
            if let AppState::RegisterInputDeviceName { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let device_name = app.credential_input.trim().to_string();
            let device_name = if device_name.is_empty() {
                // Use hostname as default
                hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "Jottery TUI".to_string())
            } else {
                device_name
            };
            app.credential_input.zeroize();

            // Register device
            if let AppState::RegisterInputDeviceName { endpoint, email, password, previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                match operations::settings::register_device(app, &endpoint, &email, &password, &device_name) {
                    Ok(()) => {
                        app.state = *previous;
                        app.input_mode = InputMode::Normal;
                        app.sync_status = Some(t!("register.success").to_string());
                    }
                    Err(e) => {
                        app.state = *previous;
                        app.input_mode = InputMode::Normal;
                        app.error = Some(format!("{}: {}", t!("register.device_failed"), e));
                    }
                }
            }
        }
        KeyCode::Char(c) => {
            app.credential_input.push(c);
        }
        KeyCode::Backspace => {
            app.credential_input.pop();
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::KeyModifiers;

    /// An app waiting for the account password, with `notes_password` as its unlock password.
    fn app_at_the_password_prompt(notes_password: &str) -> App {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        app.unlock_password = Some(Zeroizing::new(notes_password.to_string()));
        app.state = AppState::RegisterInputPassword {
            // Nothing listens here, so reaching the network would fail the test
            endpoint: "http://127.0.0.1:1".to_string(),
            email: "someone@example.com".to_string(),
            previous: Box::new(AppState::NoteList),
        };
        app
    }

    fn app_choosing_a_new_password(input: &str) -> App {
        let mut app = app_at_the_password_prompt("unused");
        app.credential_input = input.to_string();
        app.state = AppState::ChangePasswordNew {
            current_password: Zeroizing::new("unused".to_string()),
            previous: Box::new(AppState::NoteList),
        };
        app
    }

    #[test]
    fn a_new_notes_password_needs_eight_characters() {
        let enter = KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE);

        let mut app = app_choosing_a_new_password("seven77");
        handle_change_password_new_key(&mut app, enter).unwrap();
        assert!(matches!(app.state, AppState::ChangePasswordNew { .. }));
        assert!(app.error.is_some());

        let mut app = app_choosing_a_new_password("eight888");
        handle_change_password_new_key(&mut app, enter).unwrap();
        assert!(matches!(app.state, AppState::ChangePasswordConfirm { .. }));
    }

    #[test]
    fn a_new_account_password_cannot_be_the_notes_password() {
        let password = uuid::Uuid::new_v4().to_string();
        let mut app = app_at_the_password_prompt(&password);
        app.credential_input = password;

        handle_register_input_password_key(&mut app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();

        assert!(matches!(app.state, AppState::RegisterInputPassword { .. }));
        assert_eq!(app.error, Some(t!("password.same_as_account").to_string()));
        assert!(app.credential_input.is_empty());
    }

    /// A server that answers every request the same way.
    fn server_replying(body: &'static str) -> String {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut request = [0u8; 8192];
                let _ = stream.read(&mut request);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len(),
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });
        endpoint
    }

    /// The account already has this password, so refusing wouldn't protect anything.
    /// It would only stop this person adding a device.
    #[test]
    fn an_existing_account_with_the_notes_password_is_not_refused() {
        let password = uuid::Uuid::new_v4().to_string();
        let endpoint = server_replying(r#"{"exists":true,"isApproved":true,"isActive":true}"#);
        let mut app = app_at_the_password_prompt(&password);
        app.state = AppState::RegisterInputPassword {
            endpoint: endpoint.clone(),
            email: "someone@example.com".to_string(),
            previous: Box::new(AppState::NoteList),
        };

        assert!(check_account_password(&mut app, &password).is_ok());
    }

    #[test]
    fn a_server_that_has_never_heard_of_the_account_means_a_new_password() {
        let password = uuid::Uuid::new_v4().to_string();
        let endpoint = server_replying(r#"{"exists":false,"isApproved":false,"isActive":false}"#);
        let mut app = app_at_the_password_prompt(&password);
        app.state = AppState::RegisterInputPassword {
            endpoint: endpoint.clone(),
            email: "someone@example.com".to_string(),
            previous: Box::new(AppState::NoteList),
        };

        assert!(check_account_password(&mut app, &password).is_err());
    }
}
