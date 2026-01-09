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
                    if app.selected_setting < 7 {
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
                KeyCode::Char('p') => {
                    // Show text input for sync credentials (try clipboard first as convenience)
                    // Try clipboard paste as convenience (may fail over SSH)
                    if let Ok(mut clipboard) = arboard::Clipboard::new() {
                        if let Ok(text) = clipboard.get_text() {
                            app.credential_input = text;
                        }
                    }

                    // Show input modal for manual paste
                    let prev = std::mem::replace(&mut app.state, AppState::Quit);
                    app.state = AppState::InputSyncCredentials {
                        previous: Box::new(prev),
                    };
                    app.input_mode = InputMode::Insert;
                }
                KeyCode::Char('c') => {
                    // Show sync credentials as text (encrypted format)
                    match operations::settings::generate_sync_credentials_text(app, false) {
                        Ok(creds_text) => {
                            // Try clipboard copy (best effort - may fail over SSH)
                            let _ = arboard::Clipboard::new()
                                .and_then(|mut clip| clip.set_text(&creds_text));

                            // Always show text modal for manual copy
                            let prev = std::mem::replace(&mut app.state, AppState::Quit);
                            app.state = AppState::ShowSyncCredentials {
                                credentials: creds_text,
                                previous: Box::new(prev),
                            };
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to generate credentials: {}", e));
                        }
                    }
                }
                KeyCode::Char('C') => {
                    // Show sync credentials as text (legacy unencrypted format for older versions)
                    match operations::settings::generate_sync_credentials_text(app, true) {
                        Ok(creds_text) => {
                            // Try clipboard copy (best effort - may fail over SSH)
                            let _ = arboard::Clipboard::new()
                                .and_then(|mut clip| clip.set_text(&creds_text));

                            // Show warning about legacy format
                            app.sync_status = Some(t!("sync.legacy_format_warning").to_string());
                            app.sync_status_set_at = Some(std::time::Instant::now());

                            // Always show text modal for manual copy
                            let prev = std::mem::replace(&mut app.state, AppState::Quit);
                            app.state = AppState::ShowSyncCredentials {
                                credentials: creds_text,
                                previous: Box::new(prev),
                            };
                        }
                        Err(e) => {
                            app.error = Some(format!("Failed to generate credentials: {}", e));
                        }
                    }
                }
                KeyCode::Char('y') => {
                    // Trigger manual sync
                    operations::sync::trigger_sync(app);
                }
                KeyCode::Char('d') => {
                    // Disconnect from sync server (only if sync is configured)
                    if app.settings.sync_enabled || app.settings.sync_endpoint.is_some() {
                        if let Err(e) = operations::settings::disconnect_from_sync(app) {
                            app.error = Some(format!("{}: {}", t!("sync.disconnect_failed"), e));
                        }
                    }
                }
                KeyCode::Char('R') => {
                    // Check registration status (need to enter email)
                    if app.settings.sync_endpoint.is_some() {
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
                        match operations::auth::verify_password_for_remember(
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
