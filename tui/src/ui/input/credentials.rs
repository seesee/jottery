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
            // Process the credentials
            let input = app.credential_input.trim().to_string();
            app.credential_input.clear();
            app.input_mode = InputMode::Normal;

            // Return to previous state
            if let AppState::InputSyncCredentials { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }

            // Try to process credentials
            if let Err(e) = operations::settings::process_credentials_input(app, &input) {
                app.error = Some(format!("Failed to paste credentials: {}", e));
            } else {
                app.sync_status = Some(t!("sync.paste_success").to_string());
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
