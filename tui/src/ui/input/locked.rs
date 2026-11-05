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
                if let Err(e) = crate::vault::check_new_password(&app.password_input) {
                    app.error = Some(e.to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    fn app_creating_a_vault(password: &str) -> App {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        app.is_new_database = true;
        app.password_input = password.to_string();
        app.password_confirm = password.to_string();
        app
    }

    fn press_enter(app: &mut App) {
        handle_locked_key(app, KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)).unwrap();
    }

    #[test]
    fn a_new_vault_refuses_a_short_password() {
        let mut app = app_creating_a_vault(&"x".repeat(7));

        press_enter(&mut app);

        assert!(matches!(app.state, AppState::Locked));
        assert!(app.key.is_none());
        assert!(app.error.is_some());
    }

    #[test]
    fn a_new_vault_takes_eight_characters() {
        let mut app = app_creating_a_vault(&uuid::Uuid::new_v4().to_string()[..8]);

        press_enter(&mut app);

        assert!(matches!(app.state, AppState::NoteList), "{:?}", app.error);
    }

    #[test]
    fn unlocking_has_no_minimum_length() {
        // A vault from before the rule, with a password under it
        let short = "x".repeat(3);
        let short = short.as_str();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        {
            let db = crate::db::Database::open(&path, short).unwrap();
            let crypto = crate::crypto::CryptoService::new();
            let key = crypto.generate_master_key();
            crate::vault::store_local_envelope(db.connection(), &crypto, short, &key).unwrap();
        }

        let mut app = App::new(path, None).unwrap();
        app.is_new_database = false;
        app.password_input = short.to_string();
        press_enter(&mut app);

        assert!(matches!(app.state, AppState::NoteList), "{:?}", app.error);
    }
}

