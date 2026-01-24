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
            if let AppState::Help { previous, .. } =
                std::mem::replace(&mut app.state, AppState::Quit)
            {
                app.state = *previous;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            // Scroll down
            if let AppState::Help { scroll_offset, .. } = &mut app.state {
                *scroll_offset = scroll_offset.saturating_add(1);
            }
        }
        KeyCode::Up | KeyCode::Char('k') => {
            // Scroll up
            if let AppState::Help { scroll_offset, .. } = &mut app.state {
                *scroll_offset = scroll_offset.saturating_sub(1);
            }
        }
        KeyCode::PageDown => {
            // Scroll down one page
            if let AppState::Help { scroll_offset, .. } = &mut app.state {
                *scroll_offset = scroll_offset.saturating_add(10);
            }
        }
        KeyCode::PageUp => {
            // Scroll up one page
            if let AppState::Help { scroll_offset, .. } = &mut app.state {
                *scroll_offset = scroll_offset.saturating_sub(10);
            }
        }
        KeyCode::Home => {
            // Scroll to top
            if let AppState::Help { scroll_offset, .. } = &mut app.state {
                *scroll_offset = 0;
            }
        }
        KeyCode::End => {
            // Scroll to bottom (will be clamped in rendering)
            if let AppState::Help { scroll_offset, .. } = &mut app.state {
                *scroll_offset = usize::MAX;
            }
        }
        _ => {}
    }
    Ok(())
}
