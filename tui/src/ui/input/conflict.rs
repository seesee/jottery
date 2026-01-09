//! Input handling for conflict resolution modal

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use rust_i18n::t;
use std::time::Instant;

use crate::ui::app::App;
use crate::ui::operations::sync;
use crate::ui::state::ViewMode;

/// Handle key events in conflict resolution modal
pub fn handle_conflict_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match key.code {
        // Cancel - return to note list
        KeyCode::Esc | KeyCode::Char('q') => {
            app.view_mode = ViewMode::NoteList;
            app.conflict_note_id = None;
            app.conflict_data = None;
            app.conflict_local_content.clear();
            app.conflict_server_content.clear();
            app.conflict_local_tags.clear();
            app.conflict_server_tags.clear();
            app.conflict_local_scroll = 0;
            app.conflict_server_scroll = 0;
            app.conflict_focus_server = false;
        }

        // Keep Mine (local version)
        KeyCode::Char('1') => {
            match sync::resolve_keep_local(app) {
                Ok(()) => {
                    app.sync_status = Some(t!("conflict.resolved_mine").to_string());
                    app.sync_status_set_at = Some(Instant::now());
                }
                Err(e) => {
                    app.error = Some(format!("{}: {}", t!("conflict.resolve_failed"), e));
                }
            }
        }

        // Keep Server (remote version)
        KeyCode::Char('2') => {
            match sync::resolve_keep_server(app) {
                Ok(()) => {
                    app.sync_status = Some(t!("conflict.resolved_server").to_string());
                    app.sync_status_set_at = Some(Instant::now());
                }
                Err(e) => {
                    app.error = Some(format!("{}: {}", t!("conflict.resolve_failed"), e));
                }
            }
        }

        // Keep Both (create duplicate from server)
        KeyCode::Char('3') => {
            match sync::resolve_keep_both(app) {
                Ok(_new_note_id) => {
                    app.sync_status = Some(t!("conflict.resolved_both").to_string());
                    app.sync_status_set_at = Some(Instant::now());
                }
                Err(e) => {
                    app.error = Some(format!("{}: {}", t!("conflict.resolve_failed"), e));
                }
            }
        }

        // Switch between panes
        KeyCode::Tab => {
            app.conflict_focus_server = !app.conflict_focus_server;
        }

        // Scroll down
        KeyCode::Char('j') | KeyCode::Down => {
            if app.conflict_focus_server {
                app.conflict_server_scroll = app.conflict_server_scroll.saturating_add(1);
            } else {
                app.conflict_local_scroll = app.conflict_local_scroll.saturating_add(1);
            }
        }

        // Scroll up
        KeyCode::Char('k') | KeyCode::Up => {
            if app.conflict_focus_server {
                app.conflict_server_scroll = app.conflict_server_scroll.saturating_sub(1);
            } else {
                app.conflict_local_scroll = app.conflict_local_scroll.saturating_sub(1);
            }
        }

        // Page down
        KeyCode::PageDown => {
            if app.conflict_focus_server {
                app.conflict_server_scroll = app.conflict_server_scroll.saturating_add(10);
            } else {
                app.conflict_local_scroll = app.conflict_local_scroll.saturating_add(10);
            }
        }

        // Page up
        KeyCode::PageUp => {
            if app.conflict_focus_server {
                app.conflict_server_scroll = app.conflict_server_scroll.saturating_sub(10);
            } else {
                app.conflict_local_scroll = app.conflict_local_scroll.saturating_sub(10);
            }
        }

        // Home - scroll to top
        KeyCode::Home => {
            if app.conflict_focus_server {
                app.conflict_server_scroll = 0;
            } else {
                app.conflict_local_scroll = 0;
            }
        }

        _ => {}
    }

    Ok(())
}
