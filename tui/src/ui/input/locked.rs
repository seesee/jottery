//! Input handling for the locked (password entry) screen

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;
use zeroize::Zeroize;

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::AppState;

/// Handle key events in locked screen
pub fn handle_locked_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc => {
            app.state = AppState::Quit;
        }
        KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.state = AppState::Quit;
        }
        KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Toggle remember password checkbox
            app.remember_password_checkbox = !app.remember_password_checkbox;
        }
        KeyCode::Tab if app.is_new_database => {
            // Switch between password and confirm fields
            app.password_confirm_focused = !app.password_confirm_focused;
        }
        KeyCode::Enter => {
            // Try to unlock/create
            app.error = None;

            // Validate password confirmation for new databases
            if app.is_new_database {
                if app.password_input.is_empty() {
                    app.error = Some(t!("password.empty_error").to_string());
                    return Ok(());
                }
                if app.password_input != app.password_confirm {
                    app.error = Some(t!("password.mismatch_error").to_string());
                    return Ok(());
                }
            }

            if let Err(e) = operations::auth::unlock(app) {
                app.error =
                    Some(t!("password.unlock_failed", error = e.to_string()).to_string());
                app.password_input.zeroize();
                app.password_confirm.zeroize();
            }
        }
        KeyCode::Char(c) => {
            if app.is_new_database && app.password_confirm_focused {
                app.password_confirm.push(c);
            } else {
                app.password_input.push(c);
            }
        }
        KeyCode::Backspace => {
            if app.is_new_database && app.password_confirm_focused {
                app.password_confirm.pop();
            } else {
                app.password_input.pop();
            }
        }
        _ => {}
    }
    Ok(())
}
