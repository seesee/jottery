//! Search mode input handling
//!
//! Handles keyboard input when search mode is active, including:
//! - Text input for search queries
//! - Tag completion with Tab/Shift+Tab
//! - Navigation through search results
//! - Preview scrolling

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::FocusedPanel;

/// Get the tag partial from search input (text after the last # that doesn't contain space)
fn get_search_tag_partial(input: &str) -> Option<(String, usize)> {
    let last_hash = input.rfind('#')?;
    let after_hash = &input[last_hash + 1..];
    // If there's a space after the #, there's no active tag partial
    if after_hash.contains(' ') {
        return None;
    }
    Some((after_hash.to_string(), last_hash))
}

/// Get search tag completions matching the current partial
fn get_search_tag_completions(app: &App, partial: &str) -> Vec<String> {
    app.get_matching_tags(partial)
}

/// Handle key events when search mode is active
///
/// Returns `true` if the event was handled, `false` if it should fall through
/// to normal mode handling.
pub fn handle_search_key(app: &mut App, key: KeyEvent) -> Result<bool> {
    if !app.search_active {
        return Ok(false);
    }

    match key.code {
        KeyCode::Esc => {
            app.search_active = false;
            app.search_input.clear();
            app.invalidate_filter_cache();
            app.selected_note = 0;
            // Clear tag completions
            app.search_tag_completions.clear();
            app.search_tag_completion_index = 0;
        }
        KeyCode::Enter => {
            handle_search_enter(app)?;
        }
        // Preview scrolling controls (must come before generic Char(c) pattern)
        KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl-d: scroll preview down half page (10 lines)
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_add(10);
        }
        KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl-u: scroll preview up half page (10 lines)
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_sub(10);
        }
        KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl-f: scroll preview down full page (20 lines)
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_add(20);
        }
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            // Ctrl-b: scroll preview up full page (20 lines)
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_sub(20);
        }
        KeyCode::Char(c) => {
            app.search_input.push(c);
            app.invalidate_filter_cache();
            app.selected_note = 0; // Reset selection when search changes
            let filtered = app.filtered_notes();
            app.selected_note_id = filtered.first().map(|n| n.id.clone());
            // Clear tag completions when input changes
            app.search_tag_completions.clear();
            app.search_tag_completion_index = 0;
        }
        KeyCode::Backspace => {
            app.search_input.pop();
            app.invalidate_filter_cache();
            app.selected_note = 0;
            let filtered = app.filtered_notes();
            app.selected_note_id = filtered.first().map(|n| n.id.clone());
            // Clear tag completions when input changes
            app.search_tag_completions.clear();
            app.search_tag_completion_index = 0;
        }
        KeyCode::Down => {
            let filtered_count = app.filtered_notes().len();
            if filtered_count > 0 && app.selected_note < filtered_count - 1 {
                app.selected_note += 1;
                // Update selected note ID after index change
                let filtered = app.filtered_notes();
                app.selected_note_id = filtered.get(app.selected_note).map(|n| n.id.clone());
            }
        }
        KeyCode::Up => {
            if app.selected_note > 0 {
                app.selected_note -= 1;
                // Update selected note ID after index change
                let filtered = app.filtered_notes();
                app.selected_note_id = filtered.get(app.selected_note).map(|n| n.id.clone());
            }
        }
        KeyCode::Tab => {
            handle_search_tab(app);
        }
        KeyCode::BackTab => {
            handle_search_backtab(app);
        }
        KeyCode::PageDown => {
            // Page Down: scroll preview down full page (20 lines)
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_add(20);
        }
        KeyCode::PageUp => {
            // Page Up: scroll preview up full page (20 lines)
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_sub(20);
        }
        _ => {}
    }

    Ok(true)
}

/// Handle Enter key in search mode - opens selected note for editing
fn handle_search_enter(app: &mut App) -> Result<()> {
    if app.filtered_notes().is_empty() {
        return Ok(());
    }

    let filtered = app.filtered_notes();
    if app.selected_note >= filtered.len() {
        return Ok(());
    }

    // Clone all data we need before modifying self
    let is_locked = filtered[app.selected_note].locked;
    let content = filtered[app.selected_note].content.clone();
    let note_id = filtered[app.selected_note].id.clone();
    let syntax_lang = filtered[app.selected_note].syntax_language;
    let tags = filtered[app.selected_note].tags.clone();

    // Now we can clear search state
    app.search_input.clear();
    app.search_active = false;

    // Check if note has a conflict - open conflict resolution instead of editor
    if app.conflict_note_ids.contains(&note_id) {
        if let Err(e) = operations::sync::open_conflict_resolution(app, &note_id) {
            app.error = Some(format!("{}: {}", t!("conflict.resolve_failed"), e));
        }
    } else if is_locked {
        // View with pager (read-only)
        if let Err(e) = operations::notes::view_note_readonly(app, &content, syntax_lang) {
            app.error = Some(format!("Failed to view note: {}", e));
        }
    } else {
        // Set up for editing
        app.note_input = content;
        app.note_syntax = syntax_lang;
        app.current_tags = tags;
        app.editing_note_id = Some(note_id.clone());

        // Open external editor immediately
        if let Ok(new_content) = operations::attachments::edit_with_external_editor(app) {
            app.note_input = new_content;
            // Save the note
            if let Err(e) = operations::notes::save_note(app) {
                app.error = Some(t!("note.save_failed", error = e.to_string()).to_string());
            }
            // Reload notes to refresh the list
            if let Err(e) = operations::notes::load_notes(app) {
                app.error = Some(t!("note.reload_failed", error = e.to_string()).to_string());
            }
        }

        // Clear editing state
        app.editing_note_id = None;
    }

    Ok(())
}

/// Handle Tab key in search mode - tag completion or panel switch
fn handle_search_tab(app: &mut App) {
    // Check if there's a tag partial to complete
    if let Some((partial, hash_index)) = get_search_tag_partial(&app.search_input) {
        if app.search_tag_completions.is_empty() {
            // Get new completions
            app.search_tag_completions = get_search_tag_completions(app, &partial);
            app.search_tag_completion_index = 0;
        } else {
            // Cycle to next
            app.search_tag_completion_index =
                (app.search_tag_completion_index + 1) % app.search_tag_completions.len();
        }
        // Update input to show current completion
        if !app.search_tag_completions.is_empty() {
            let completion = &app.search_tag_completions[app.search_tag_completion_index];
            // Replace the partial with the full tag
            app.search_input = format!("{}#{}", &app.search_input[..hash_index], completion);
            app.invalidate_filter_cache();
            app.selected_note = 0;
            let filtered = app.filtered_notes();
            app.selected_note_id = filtered.first().map(|n| n.id.clone());
        }
    } else {
        // No tag partial - exit search mode and toggle to attachments panel
        app.search_active = false;
        let filtered = app.filtered_notes();
        if !filtered.is_empty() && app.selected_note < filtered.len() {
            let note = &filtered[app.selected_note];
            if !note.attachments.is_empty() {
                app.focused_panel = FocusedPanel::Attachments;
                app.selected_attachment = 0;
            }
        }
    }
}

/// Handle Shift+Tab key in search mode - cycle backward through tag completions
fn handle_search_backtab(app: &mut App) {
    if app.search_tag_completions.is_empty() {
        return;
    }

    if let Some((_, hash_index)) = get_search_tag_partial(&app.search_input) {
        if app.search_tag_completion_index == 0 {
            app.search_tag_completion_index = app.search_tag_completions.len() - 1;
        } else {
            app.search_tag_completion_index -= 1;
        }
        let completion = &app.search_tag_completions[app.search_tag_completion_index];
        app.search_input = format!("{}#{}", &app.search_input[..hash_index], completion);
        app.invalidate_filter_cache();
        app.selected_note = 0;
        let filtered = app.filtered_notes();
        app.selected_note_id = filtered.first().map(|n| n.id.clone());
    }
}
