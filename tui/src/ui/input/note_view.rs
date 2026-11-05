//! Input handling for note viewing and editing

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use rust_i18n::t;
use std::collections::HashSet;

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::{AppState, InputMode};

/// Collect all unique tags from notes
fn collect_all_tags(app: &App) -> Vec<String> {
    let mut all_tags: HashSet<String> = HashSet::new();
    for note in &app.notes {
        for tag in &note.tags {
            all_tags.insert(tag.clone());
        }
    }
    let mut tags: Vec<String> = all_tags.into_iter().collect();
    tags.sort();
    tags
}

/// Get tag completions matching the current input
fn get_tag_completions(app: &App) -> Vec<String> {
    let query = app.tag_input.to_lowercase();
    // Require at least one character to start completing
    if query.is_empty() {
        return Vec::new();
    }
    collect_all_tags(app)
        .into_iter()
        .filter(|tag| {
            let tag_lower = tag.to_lowercase();
            tag_lower.starts_with(&query) && !app.current_tags.contains(tag)
        })
        .collect()
}

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
                match operations::attachments::edit_with_external_editor(app) {
                    Ok(content) => {
                        app.note_input = content;
                        app.error = None;
                    }
                    Err(e) => app.error = Some(format!("{e:#}")),
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
                    scroll_offset: 0,
                };
            }
            // After a failed save, Esc gives up on the edit. Otherwise the only way
            // out of this screen would be a save that may keep failing.
            KeyCode::Esc if app.error.is_some() => {
                app.error = None;
                app.editing_note_id = None;
                app.state = AppState::NoteList;
            }
            KeyCode::Char('q') | KeyCode::Esc => {
                // Save and return to list. A failed save stays here with the text
                // intact, as leaving would throw the edit away.
                match operations::notes::save_note(app) {
                    Ok(()) => {
                        app.error = None;
                        app.editing_note_id = None;
                        app.state = AppState::NoteList;
                        if let Err(e) = operations::notes::load_notes(app) {
                            app.error = Some(t!("note.reload_failed", error = e.to_string()).to_string());
                        }
                    }
                    Err(e) => {
                        app.error = Some(t!("note.save_failed", error = e.to_string()).to_string());
                    }
                }
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
                app.tag_completions.clear();
                app.tag_completion_index = 0;
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Enter => {
                // Add tag (either from completion or typed)
                let tag = if !app.tag_completions.is_empty() && app.tag_completion_index < app.tag_completions.len() {
                    app.tag_completions[app.tag_completion_index].clone()
                } else {
                    app.tag_input.trim().to_string()
                };
                if !tag.is_empty() && !app.current_tags.contains(&tag) {
                    app.current_tags.push(tag);
                }
                app.tag_input.clear();
                app.tag_completions.clear();
                app.tag_completion_index = 0;
            }
            KeyCode::Tab => {
                // Tab completion - cycle forward through matching tags
                if app.tag_completions.is_empty() {
                    // Get new completions
                    app.tag_completions = get_tag_completions(app);
                    app.tag_completion_index = 0;
                } else {
                    // Cycle to next
                    app.tag_completion_index = (app.tag_completion_index + 1) % app.tag_completions.len();
                }
                // Update input to show current completion
                if !app.tag_completions.is_empty() {
                    app.tag_input = app.tag_completions[app.tag_completion_index].clone();
                }
            }
            KeyCode::BackTab => {
                // Shift+Tab - cycle backward through completions
                if !app.tag_completions.is_empty() {
                    if app.tag_completion_index == 0 {
                        app.tag_completion_index = app.tag_completions.len() - 1;
                    } else {
                        app.tag_completion_index -= 1;
                    }
                    app.tag_input = app.tag_completions[app.tag_completion_index].clone();
                }
            }
            KeyCode::Char(c) => {
                app.tag_input.push(c);
                // Clear completions when input changes
                app.tag_completions.clear();
                app.tag_completion_index = 0;
            }
            KeyCode::Backspace => {
                if app.tag_input.is_empty() && !app.current_tags.is_empty() {
                    // Remove last tag if input is empty
                    app.current_tags.pop();
                } else {
                    app.tag_input.pop();
                }
                // Clear completions when input changes
                app.tag_completions.clear();
                app.tag_completion_index = 0;
            }
            _ => {}
        },
        InputMode::SaveSearchName | InputMode::AttachmentPath | InputMode::BulkAddTags | InputMode::BulkRemoveTags
        | InputMode::BulkSetColour | InputMode::BulkExportPath => {
            // These modes are handled in note list view, not note view
            // Reset to normal if somehow we end up here
            app.input_mode = InputMode::Normal;
        }
    }
    Ok(())
}
