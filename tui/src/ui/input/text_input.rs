//! Shared text input handling helpers
//!
//! Provides common functionality for text input modes like bulk add tags,
//! bulk export path, etc.

use crossterm::event::{KeyCode, KeyEvent};

/// Result of handling a text input key
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextInputResult {
    /// Input was submitted (Enter pressed)
    Submit,
    /// Input was cancelled (Esc pressed)
    Cancel,
    /// Key was handled, continue accepting input
    Continue,
    /// Key was not handled by text input handler
    Unhandled,
}

/// Handle basic text input keys on a mutable string buffer.
///
/// Handles:
/// - Enter: Returns Submit (caller should process the input)
/// - Esc: Clears the input and returns Cancel
/// - Backspace: Removes last character, returns Continue
/// - Char(c): Appends character, returns Continue
/// - Other keys: Returns Unhandled
///
/// # Example
/// ```ignore
/// match handle_text_input(&key, &mut app.bulk_tags_input) {
///     TextInputResult::Submit => {
///         // Process the input
///         do_something(&app.bulk_tags_input);
///         app.bulk_tags_input.clear();
///         app.input_mode = InputMode::Normal;
///     }
///     TextInputResult::Cancel => {
///         app.input_mode = InputMode::Normal;
///     }
///     TextInputResult::Continue => {
///         // Input updated, continue
///     }
///     TextInputResult::Unhandled => {
///         // Handle other keys if needed
///     }
/// }
/// ```
pub fn handle_text_input(key: &KeyEvent, input: &mut String) -> TextInputResult {
    match key.code {
        KeyCode::Enter => TextInputResult::Submit,
        KeyCode::Esc => {
            input.clear();
            TextInputResult::Cancel
        }
        KeyCode::Backspace => {
            input.pop();
            TextInputResult::Continue
        }
        KeyCode::Char(c) => {
            input.push(c);
            TextInputResult::Continue
        }
        _ => TextInputResult::Unhandled,
    }
}

