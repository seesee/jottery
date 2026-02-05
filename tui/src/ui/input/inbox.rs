//! Input handling for the Inbox view

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::ui::app::App;
use crate::ui::state::ViewMode;

/// Handle key events when in Inbox view mode
pub fn handle_inbox_key(app: &mut App, key: KeyEvent) {
    // Handle confirmation modals first
    if app.show_inbox_delete_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.show_inbox_delete_confirm = false;
                if let Err(e) = crate::ui::operations::inbox::delete_item(app, app.selected_inbox_item) {
                    app.error = Some(format!("Failed to delete inbox item: {}", e));
                }
            }
            _ => {
                app.show_inbox_delete_confirm = false;
            }
        }
        return;
    }

    if app.show_inbox_delete_all_confirm {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') => {
                app.show_inbox_delete_all_confirm = false;
                if let Err(e) = crate::ui::operations::inbox::delete_all(app) {
                    app.error = Some(format!("Failed to delete all inbox items: {}", e));
                }
            }
            _ => {
                app.show_inbox_delete_all_confirm = false;
            }
        }
        return;
    }

    match key.code {
        KeyCode::Esc | KeyCode::Char('q') => {
            // Close inbox view
            app.view_mode = ViewMode::NoteList;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            if app.selected_inbox_item > 0 {
                app.selected_inbox_item -= 1;
            }
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !app.inbox_items.is_empty() && app.selected_inbox_item < app.inbox_items.len() - 1 {
                app.selected_inbox_item += 1;
            }
        }
        KeyCode::Enter => {
            // Accept selected item
            if !app.inbox_items.is_empty() {
                if let Err(e) = crate::ui::operations::inbox::accept_item(app, app.selected_inbox_item) {
                    app.error = Some(format!("Failed to accept inbox item: {}", e));
                }
                // If inbox is now empty, return to note list
                if app.inbox_items.is_empty() {
                    app.view_mode = ViewMode::NoteList;
                }
            }
        }
        KeyCode::Char('d') => {
            // Delete selected item (with confirmation)
            if !app.inbox_items.is_empty() {
                app.show_inbox_delete_confirm = true;
            }
        }
        KeyCode::Char('A') => {
            // Accept all items
            if !app.inbox_items.is_empty() {
                if let Err(e) = crate::ui::operations::inbox::accept_all(app) {
                    app.error = Some(format!("Failed to accept all inbox items: {}", e));
                }
                app.view_mode = ViewMode::NoteList;
            }
        }
        KeyCode::Char('D') if key.modifiers.contains(KeyModifiers::SHIFT) || key.code == KeyCode::Char('D') => {
            // Delete all items (with confirmation)
            if !app.inbox_items.is_empty() {
                app.show_inbox_delete_all_confirm = true;
            }
        }
        _ => {}
    }
}
