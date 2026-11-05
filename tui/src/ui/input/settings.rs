//! Input handling for the settings screen

use std::time::Instant;

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::{AppState, InputMode};

/// Handle key events in settings screen
pub fn handle_settings_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Disconnecting drops the credentials and any conflicts still waiting, so nothing
    // else gets through until it's answered
    if app.show_disconnect_confirm {
        match key.code {
            KeyCode::Char('y') => {
                app.show_disconnect_confirm = false;
                if let Err(e) = operations::settings::disconnect_from_sync(app) {
                    app.error = Some(format!("{}: {}", t!("sync.disconnect_failed"), e));
                }
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => {
                app.show_disconnect_confirm = false;
            }
            _ => {}
        }
        return Ok(());
    }

    match app.input_mode {
        InputMode::Normal => {
            // Navigation mode
            match key.code {
                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => {
                    // Return to previous state
                    if let AppState::Settings { previous } =
                        std::mem::replace(&mut app.state, AppState::Quit)
                    {
                        app.state = *previous;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    // Move down through settings fields
                    if app.selected_setting < 8 {
                        app.selected_setting += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    // Move up through settings fields
                    if app.selected_setting > 0 {
                        app.selected_setting -= 1;
                    }
                }
                KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    // Edit selected field (backward for cyclic fields)
                    operations::settings::start_editing_setting_backward(app);
                }
                KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                    // Edit selected field (forward for cyclic fields)
                    operations::settings::start_editing_setting(app);
                }
                KeyCode::Char('I') => {
                    // Edit selected field (backward for cyclic fields)
                    operations::settings::start_editing_setting_backward(app);
                }
                KeyCode::Char('y') => {
                    // Trigger manual sync
                    operations::sync::trigger_sync(app);
                }
                KeyCode::Char('d') => {
                    // Disconnect from sync server or clear pending registration
                    if app.settings.sync_enabled || app.settings.sync_endpoint.is_some() {
                        // Fully configured - disconnect, once that's confirmed
                        app.refresh_conflict_cache();
                        app.show_disconnect_confirm = true;
                    } else if operations::settings::has_pending_registration(app) {
                        // Pending registration - clear it
                        if let Err(e) = operations::settings::clear_pending_registration(app) {
                            app.error = Some(format!("{}: {}", t!("sync.disconnect_failed"), e));
                        } else {
                            app.sync_status = Some(t!("register.cleared").to_string());
                            app.sync_status_set_at = Some(std::time::Instant::now());
                        }
                    }
                }
                KeyCode::Char('R') => {
                    // Check registration status
                    // If there's a pending registration, use its endpoint and email directly
                    if let Ok(Some((endpoint, email))) = operations::settings::get_pending_registration(app) {
                        // Check status directly using pending registration info
                        match operations::settings::check_registration_status_with_endpoint(app, &endpoint, &email) {
                            Ok(approved) => {
                                if approved {
                                    // Approved! Go to password input
                                    app.credential_input.clear();
                                    app.sync_status = Some(t!("register.approved").to_string());
                                    app.sync_status_set_at = Some(std::time::Instant::now());
                                    let prev = std::mem::replace(&mut app.state, AppState::Quit);
                                    app.state = AppState::RegisterInputPassword {
                                        endpoint,
                                        email,
                                        previous: Box::new(prev),
                                    };
                                    app.input_mode = InputMode::Insert;
                                } else {
                                    app.sync_status = Some(t!("register.still_pending").to_string());
                                    app.sync_status_set_at = Some(std::time::Instant::now());
                                }
                            }
                            Err(e) => {
                                app.error = Some(format!("{}: {}", t!("sync.status.check_failed"), e));
                            }
                        }
                    } else if app.settings.sync_endpoint.is_some() {
                        // Fully configured - ask for email
                        app.credential_input.clear();
                        let prev = std::mem::replace(&mut app.state, AppState::Quit);
                        app.state = AppState::InputEmailForStatus {
                            previous: Box::new(prev),
                        };
                        app.input_mode = InputMode::Insert;
                    } else {
                        app.error = Some(t!("sync.endpoint_not_configured").to_string());
                    }
                }
                KeyCode::Char('P') => {
                    // Change password (only when unlocked with envelope or legacy encryption)
                    if app.key.is_some() {
                        app.credential_input.clear();
                        let prev = std::mem::replace(&mut app.state, AppState::Quit);
                        app.state = AppState::ChangePasswordCurrent {
                            previous: Box::new(prev),
                        };
                        app.input_mode = InputMode::Insert;
                    }
                }
                KeyCode::Char('r') => {
                    // Start or resume registration flow (only if sync is not fully configured)
                    if !operations::settings::is_sync_fully_configured(app) {
                        // Check if there's a pending registration to resume
                        match operations::settings::get_pending_registration(app) {
                            Ok(Some((endpoint, email))) => {
                                // Resume pending registration - go to pending approval screen
                                let prev = std::mem::replace(&mut app.state, AppState::Quit);
                                app.state = AppState::RegisterPendingApproval {
                                    endpoint,
                                    email,
                                    previous: Box::new(prev),
                                };
                                app.input_mode = InputMode::Normal;
                            }
                            _ => {
                                // No pending registration - start fresh
                                app.credential_input.clear();
                                let prev = std::mem::replace(&mut app.state, AppState::Quit);
                                app.state = AppState::RegisterInputEndpoint {
                                    previous: Box::new(prev),
                                };
                                app.input_mode = InputMode::Insert;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        InputMode::SettingsEdit => {
            // Editing mode
            match key.code {
                KeyCode::Esc => {
                    // Cancel editing
                    app.setting_input.clear();
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Enter => {
                    // The address stays in the field until the warning has been answered
                    if app.selected_setting == 5 {
                        let endpoint = app.setting_input.clone();
                        if !endpoint.is_empty() && !operations::settings::endpoint_confirmed(app, &endpoint) {
                            return Ok(());
                        }
                    }

                    // Save edited value
                    if let Err(e) = operations::settings::save_setting_value(app) {
                        app.error = Some(format!("Failed to save setting: {}", e));
                    }
                    app.setting_input.clear();
                    app.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => {
                    // For boolean and enum fields, handle cycling
                    match app.selected_setting {
                        1 => {
                            // Color Scheme: cycle through Light/Dark/Auto
                            operations::settings::cycle_theme(app);
                            app.input_mode = InputMode::Normal;
                        }
                        2 => {
                            // Sort order: cycle through Recent/Oldest/Alpha/Created
                            operations::settings::cycle_sort_order(app);
                            app.input_mode = InputMode::Normal;
                        }
                        4 => {
                            // Sync enabled: toggle
                            app.settings.sync_enabled = !app.settings.sync_enabled;
                            if let Err(e) = operations::settings::save_settings(app) {
                                app.error = Some(format!("Failed to save settings: {}", e));
                            }
                            app.input_mode = InputMode::Normal;
                        }
                        _ => {
                            // String/number fields: type normally
                            app.setting_input.push(c);
                        }
                    }
                }
                KeyCode::Backspace => {
                    app.setting_input.pop();
                }
                _ => {}
            }
        }
        InputMode::PasswordVerify => {
            // Password verification mode for enabling remember password
            match key.code {
                KeyCode::Esc => {
                    // Cancel password verification
                    app.setting_input.clear();
                    app.input_mode = InputMode::Normal;
                    app.sync_status = Some(t!("password.remember_disabled").to_string());
                    app.sync_status_set_at = Some(Instant::now());
                }
                KeyCode::Enter => {
                    // Verify password and save if correct
                    if app.setting_input.is_empty() {
                        app.error = Some(t!("password.empty_error").to_string());
                    } else {
                        // Verify password by attempting to derive the key
                        match operations::auth::verify_password(
                            app,
                            &app.setting_input,
                        ) {
                            Ok(true) => {
                                // Password is correct - store it
                                let password_to_store = app.setting_input.clone();
                                app.setting_input.clear();
                                app.input_mode = InputMode::Normal;

                                if let Err(e) = operations::auth::store_password_for_autounlock(
                                    app,
                                    &password_to_store,
                                ) {
                                    app.error = Some(format!("Failed to store password: {}", e));
                                } else {
                                    app.sync_status =
                                        Some(t!("password.remember_enabled").to_string());
                                    app.sync_status_set_at = Some(Instant::now());
                                }
                            }
                            Ok(false) => {
                                app.error = Some(t!("password.incorrect").to_string());
                                app.setting_input.clear();
                            }
                            Err(e) => {
                                app.error = Some(
                                    t!("password.verification_failed", error = e.to_string())
                                        .to_string(),
                                );
                                app.setting_input.clear();
                                app.input_mode = InputMode::Normal;
                            }
                        }
                    }
                }
                KeyCode::Char(c) => {
                    // Add character to password input
                    app.setting_input.push(c);
                }
                KeyCode::Backspace => {
                    // Remove last character
                    app.setting_input.pop();
                }
                _ => {}
            }
        }
        _ => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app_in_settings() -> App {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        app.settings.sync_enabled = true;
        app.settings.sync_endpoint = Some("https://notes.example.com".to_string());
        app
    }

    fn press(app: &mut App, c: char) {
        handle_settings_key(app, KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE)).unwrap();
    }

    #[test]
    fn one_key_does_not_disconnect() {
        let mut app = app_in_settings();

        press(&mut app, 'd');

        assert!(app.show_disconnect_confirm);
        assert!(app.settings.sync_enabled);
    }

    #[test]
    fn declining_keeps_the_connection_and_other_keys_wait() {
        let mut app = app_in_settings();
        press(&mut app, 'd');

        press(&mut app, 'j');
        assert!(app.show_disconnect_confirm);
        assert_eq!(app.selected_setting, 0);

        press(&mut app, 'n');
        assert!(!app.show_disconnect_confirm);
        assert!(app.settings.sync_enabled);
    }
}
