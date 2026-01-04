//! Input handling for note viewing and editing

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::{AppState, InputMode};

/// Handle key events in note view state
pub fn handle_note_view_key(app: &mut App, key: KeyEvent) -> Result<()> {
    match app.input_mode {
        InputMode::SettingsEdit | InputMode::PasswordVerify => {
            // Settings modes should not be active in note view
            // Reset to normal mode if somehow this happens
            app.input_mode = InputMode::Normal;
        }
        InputMode::Normal => match key.code {
            KeyCode::Char('e') | KeyCode::Enter => {
                // Edit with external $EDITOR
                if let Ok(content) = operations::attachments::edit_with_external_editor(app) {
                    app.note_input = content;
                }
            }
            KeyCode::Char('t') => {
                // Enter tag mode
                app.tag_input.clear();
                app.input_mode = InputMode::Tag;
            }
            KeyCode::Char('?') => {
                // Show help
                let prev = std::mem::replace(&mut app.state, AppState::Quit);
                app.state = AppState::Help {
                    previous: Box::new(prev),
                };
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                // Save and return to list
                operations::notes::save_note(app)?;
                operations::notes::load_notes(app)?;
                app.state = AppState::NoteList;
            }
            _ => {}
        },
        InputMode::Insert => {
            // Insert mode not used in note view - redirect to normal mode
            app.input_mode = InputMode::Normal;
        }
        InputMode::Tag => match key.code {
            KeyCode::Esc => {
                // Exit tag mode
                app.tag_input.clear();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                // Add tag
                let tag = app.tag_input.trim().to_string();
                if !tag.is_empty() && !app.current_tags.contains(&tag) {
                    app.current_tags.push(tag);
                }
                app.tag_input.clear();
            }
            KeyCode::Char(c) => {
                app.tag_input.push(c);
            }
            KeyCode::Backspace => {
                if app.tag_input.is_empty() && !app.current_tags.is_empty() {
                    // Remove last tag if input is empty
                    app.current_tags.pop();
                } else {
                    app.tag_input.pop();
                }
            }
            _ => {}
        },
        InputMode::AttachmentPath => {
            // AttachmentPath mode is handled in note list view, not note view
            // Reset to normal if somehow we end up here
            app.input_mode = InputMode::Normal;
        }
    }
    Ok(())
}
