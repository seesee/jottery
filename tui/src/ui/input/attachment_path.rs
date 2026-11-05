//! Attachment path input mode handling
//!
//! Handles keyboard input when entering a file path for adding attachments,
//! including path completion with Tab/Shift+Tab.

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::InputMode;

/// Handle key events when in attachment path input mode
///
/// Returns `true` if the event was handled, `false` if it should fall through.
pub fn handle_attachment_path_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if !matches!(app.input_mode, InputMode::AttachmentPath) {
        return Ok(false);
    }

    match key.code {
        KeyCode::Enter => {
            handle_path_enter(app)?;
        }
        KeyCode::Esc => {
            // Cancel attachment input
            app.attachment_path_input.clear();
            app.reset_path_completions();
            app.attachment_to_save = None;
            app.input_mode = InputMode::Normal;
        }
        KeyCode::Tab => {
            // Trigger/cycle path completion
            if app.path_completions.is_empty() {
                // Get new completions
                app.path_completions = operations::attachments::get_path_completions(&app.attachment_path_input);
                app.path_completion_index = 0;
            } else {
                // Cycle to next completion
                app.path_completion_index = (app.path_completion_index + 1) % app.path_completions.len();
            }
        }
        KeyCode::BackTab => {
            // Cycle backwards through completions
            if !app.path_completions.is_empty() {
                if app.path_completion_index == 0 {
                    app.path_completion_index = app.path_completions.len() - 1;
                } else {
                    app.path_completion_index -= 1;
                }
            }
        }
        KeyCode::Down => {
            // Navigate down in completions
            if !app.path_completions.is_empty() {
                app.path_completion_index = (app.path_completion_index + 1) % app.path_completions.len();
            }
        }
        KeyCode::Up => {
            // Navigate up in completions
            if !app.path_completions.is_empty() {
                if app.path_completion_index == 0 {
                    app.path_completion_index = app.path_completions.len() - 1;
                } else {
                    app.path_completion_index -= 1;
                }
            }
        }
        KeyCode::Backspace => {
            app.attachment_path_input.pop();
            // Clear completions when input changes
            app.reset_path_completions();
        }
        KeyCode::Char(c) => {
            app.attachment_path_input.push(c);
            // Clear completions when input changes
            app.reset_path_completions();
        }
        _ => {}
    }

    Ok(true)
}

/// Handle Enter key - either select completion or add attachment
fn handle_path_enter(app: &mut App) -> Result<()> {
    // If completions are showing and one is selected, use it
    if !app.path_completions.is_empty() && app.path_completion_index < app.path_completions.len() {
        app.attachment_path_input = app.path_completions[app.path_completion_index].clone();
        app.reset_path_completions();
        // If it's a directory, show its contents
        if app.attachment_path_input.ends_with('/') {
            app.path_completions = operations::attachments::get_path_completions(&app.attachment_path_input);
        }
    } else {
        // Save to the path, or add the file at it
        let path = app.attachment_path_input.clone();
        let saving = app.attachment_to_save.take();
        app.attachment_path_input.clear();
        app.reset_path_completions();
        app.input_mode = InputMode::Normal;

        if path.is_empty() {
            return Ok(());
        }

        match saving {
            Some(attachment) => match operations::attachments::save_attachment(app, &attachment, &path) {
                Ok(saved) => app.notify(t!("attachment.saved", path = saved.display().to_string()).to_string()),
                Err(e) => app.error = Some(t!("attachment.save_failed", error = format!("{e:#}")).to_string()),
            },
            None => {
                if let Err(e) = operations::attachments::add_attachment_to_current_note(app, &path) {
                    app.error = Some(t!("attachment.add_failed", error = e.to_string()).to_string());
                }
            }
        }
    }

    Ok(())
}
