//! Input handling for the list of saved searches

use crossterm::event::{KeyCode, KeyEvent};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::operations::saved_searches;
use crate::ui::state::ViewMode;

/// Handle key events when the list of saved searches is showing
pub fn handle_saved_searches_key(app: &mut App, key: KeyEvent) {
    if app.show_saved_search_delete_confirm {
        app.show_saved_search_delete_confirm = false;
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            if let Err(e) = saved_searches::delete_selected(app) {
                app.error = Some(t!("saved_search.failed", error = e.to_string()).to_string());
            }
        }
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            app.view_mode = ViewMode::NoteList;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            app.selected_saved_search = app.selected_saved_search.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if app.selected_saved_search + 1 < app.saved_searches.len() {
                app.selected_saved_search += 1;
            }
        }
        KeyCode::Enter => saved_searches::apply_selected(app),
        KeyCode::Char('d') if !app.saved_searches.is_empty() => {
            app.show_saved_search_delete_confirm = true;
        }
        _ => {}
    }
}
