//! Input handling for the help screen

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::app::App;
use crate::ui::state::AppState;

/// Handle key events in help screen
pub fn handle_help_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
            // Return to previous state
            if let AppState::Help { previous } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        _ => {}
    }
    Ok(())
}
