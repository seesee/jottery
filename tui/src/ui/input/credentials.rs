//! Input handling for sync credentials screens

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::{AppState, InputMode};

/// Handle key events when showing sync credentials
pub fn handle_show_credentials_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
            // Return to previous state
            if let AppState::ShowSyncCredentials { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        _ => {}
    }
    Ok(())
}

/// Handle key events when inputting sync credentials
pub fn handle_input_credentials_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Cancel input
            app.credential_input.clear();
            app.input_mode = InputMode::Normal;
            if let AppState::InputSyncCredentials { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            // Validate credentials format first
            let input = app.credential_input.trim().to_string();
            if input.is_empty() {
                app.error = Some(t!("sync.credentials_empty").to_string());
                return Ok(());
            }

            app.credential_input.clear();

            // Move to device name input
            if let AppState::InputSyncCredentials { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                // Pre-populate device name with hostname
                app.credential_input = hostname::get()
                    .map(|h| h.to_string_lossy().to_string())
                    .unwrap_or_else(|_| "Jottery TUI".to_string());
                app.state = AppState::ImportInputDeviceName {
                    credentials: input,
                    previous,
                };
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

/// Handle key events when inputting device name for imported credentials
pub fn handle_import_device_name_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Cancel input
            app.credential_input.clear();
            app.input_mode = InputMode::Normal;
            if let AppState::ImportInputDeviceName { previous, .. } =
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
            app.credential_input.clear();
            app.input_mode = InputMode::Normal;

            // Get credentials and process them with device name
            if let AppState::ImportInputDeviceName { credentials, previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;

                // Process credentials with device name
                if let Err(e) = operations::settings::process_credentials_input_with_device_name(app, &credentials, &device_name) {
                    app.error = Some(format!("{}: {}", t!("sync.import_failed"), e));
                } else {
                    app.sync_status = Some(t!("sync.paste_success").to_string());
                    app.sync_status_set_at = Some(std::time::Instant::now());
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

/// Handle key events when inputting email for status check
pub fn handle_input_email_for_status_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            // Cancel input
            app.credential_input.clear();
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
            app.credential_input.clear();
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
    match key.code {
        KeyCode::Esc => {
            app.credential_input.clear();
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
            app.credential_input.clear();

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
    match key.code {
        KeyCode::Esc => {
            app.credential_input.clear();
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
            app.credential_input.clear();

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

/// Handle key events when inputting password for registration
pub fn handle_register_input_password_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.credential_input.clear();
            app.input_mode = InputMode::Normal;
            if let AppState::RegisterInputPassword { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Enter => {
            let password = app.credential_input.clone();
            if password.len() < 12 {
                app.error = Some(t!("register.password_too_short").to_string());
                return Ok(());
            }
            app.credential_input.clear();

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
                            app.credential_input.clear();
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
    match key.code {
        KeyCode::Esc => {
            app.credential_input.clear();
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
            app.credential_input.clear();

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
