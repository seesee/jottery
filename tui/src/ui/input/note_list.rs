//! Input handling for the note list screen

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;

use crate::models::SyntaxLanguage;
use crate::repository::NoteRepository;
use crate::ui::app::App;
use crate::ui::input::text_input::{handle_text_input, TextInputResult};
use crate::ui::operations;
use crate::ui::state::{AppState, FocusedPanel, InputMode, ViewMode};

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

/// Cycle through colors in the palette
///
/// # Arguments
/// * `current_color` - The current color, or None if no color is set
/// * `app` - The app state (for accessing cached color palette)
/// * `forward` - true to cycle forward, false to cycle backward
///
/// # Returns
/// The next/previous color, or None to clear the color
fn cycle_color(current_color: Option<&String>, app: &App, forward: bool) -> Option<String> {
    // Use cached color names to avoid allocation on every keypress
    let color_names = app.get_color_names();

    if color_names.is_empty() {
        return None;
    }

    let len = color_names.len();

    match current_color {
        None => {
            // No color set - return first (forward) or last (backward)
            if forward {
                Some(color_names[0].clone())
            } else {
                Some(color_names[len - 1].clone())
            }
        }
        Some(current) => {
            if let Some(pos) = color_names.iter().position(|c| c == current) {
                if forward {
                    let next_pos = (pos + 1) % len;
                    if next_pos == 0 {
                        None // Wrapped around - clear color
                    } else {
                        Some(color_names[next_pos].clone())
                    }
                } else {
                    if pos == 0 {
                        None // At first color - clear color
                    } else {
                        Some(color_names[pos - 1].clone())
                    }
                }
            } else {
                // Current color not found - return first (forward) or last (backward)
                if forward {
                    Some(color_names[0].clone())
                } else {
                    Some(color_names[len - 1].clone())
                }
            }
        }
    }
}

/// Get the next color in the palette (convenience wrapper)
#[inline]
fn cycle_color_forward(current_color: Option<&String>, app: &App) -> Option<String> {
    cycle_color(current_color, app, true)
}

/// Get the previous color in the palette (convenience wrapper)
#[inline]
fn cycle_color_backward(current_color: Option<&String>, app: &App) -> Option<String> {
    cycle_color(current_color, app, false)
}

/// Handle key events in note list state
pub fn handle_note_list_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Debug: log which key was pressed
    if let KeyCode::Char(c) = key.code {
        app.debug_log(&format!("Key pressed: '{}'", c));
    }

    // Clear sync status on any key (except 'y' which sets it)
    if key.code != KeyCode::Char('y') {
        app.sync_status = None;
    }

    // Handle inbox view mode
    if matches!(app.view_mode, ViewMode::Inbox) {
        super::inbox::handle_inbox_key(app, key);
        return Ok(());
    }

    // Handle conflict resolution view mode
    if matches!(app.view_mode, ViewMode::ConflictResolution) {
        return super::conflict::handle_conflict_key(app, key);
    }

    // Handle bulk add tags input mode
    if matches!(app.input_mode, InputMode::BulkAddTags) {
        match handle_text_input(&key, &mut app.bulk_tags_input) {
            TextInputResult::Submit => {
                // Add tags to selected notes
                if !app.bulk_tags_input.is_empty() {
                    let tags: Vec<String> = app.bulk_tags_input
                        .split(',')
                        .map(|s| s.trim().to_string())
                        .filter(|s| !s.is_empty())
                        .collect();

                    if !tags.is_empty() {
                        match operations::bulk::add_tags_to_selected(app, &tags) {
                            Ok(count) => {
                                app.sync_status = Some(t!("bulk.tags_added", count = count, tags = tags.join(", ")).to_string());
                            }
                            Err(e) => {
                                app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                }
                app.bulk_tags_input.clear();
                app.input_mode = InputMode::Normal;
            }
            TextInputResult::Cancel => {
                app.input_mode = InputMode::Normal;
            }
            TextInputResult::Continue | TextInputResult::Unhandled => {}
        }
        return Ok(());
    }

    // Handle bulk export path input mode
    if matches!(app.input_mode, InputMode::BulkExportPath) {
        match handle_text_input(&key, &mut app.bulk_export_path_input) {
            TextInputResult::Submit => {
                // Export selected notes to file
                if !app.bulk_export_path_input.is_empty() {
                    let path = std::path::PathBuf::from(&app.bulk_export_path_input);
                    match operations::bulk::export_selected(app, &path) {
                        Ok(count) => {
                            app.sync_status = Some(t!("bulk.exported", count = count, path = app.bulk_export_path_input.clone()).to_string());
                            app.clear_multi_selection();
                        }
                        Err(e) => {
                            app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string());
                        }
                    }
                }
                app.bulk_export_path_input.clear();
                app.input_mode = InputMode::Normal;
            }
            TextInputResult::Cancel => {
                app.input_mode = InputMode::Normal;
            }
            TextInputResult::Continue | TextInputResult::Unhandled => {}
        }
        return Ok(());
    }

    // Handle attachment path input mode
    if matches!(app.input_mode, InputMode::AttachmentPath) {
        match key.code {
            KeyCode::Enter => {
                // If completions are showing and one is selected, use it
                if !app.path_completions.is_empty() && app.path_completion_index < app.path_completions.len() {
                    app.attachment_path_input = app.path_completions[app.path_completion_index].clone();
                    app.reset_path_completions();
                    // If it's a directory, show its contents
                    if app.attachment_path_input.ends_with('/') {
                        app.path_completions = operations::attachments::get_path_completions(&app.attachment_path_input);
                    }
                } else {
                    // Add attachment from file path
                    let path = app.attachment_path_input.clone();
                    app.attachment_path_input.clear();
                    app.reset_path_completions();
                    app.input_mode = InputMode::Normal;

                    if !path.is_empty() {
                        if let Err(e) = operations::attachments::add_attachment_to_current_note(app, &path) {
                            app.error = Some(t!("attachment.add_failed", error = e.to_string()).to_string());
                        }
                    }
                }
            }
            KeyCode::Esc => {
                // Cancel attachment input
                app.attachment_path_input.clear();
                app.reset_path_completions();
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
        return Ok(());
    }

    // Handle search mode
    if app.search_active {
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
                // Exit search and view/edit selected note directly
                if !app.filtered_notes().is_empty() {
                    let filtered = app.filtered_notes();
                    if app.selected_note < filtered.len() {
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
                    }
                }
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
            KeyCode::BackTab => {
                // Shift+Tab - cycle backward through tag completions
                if !app.search_tag_completions.is_empty() {
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
    } else {
        // Normal note list mode
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.state = AppState::Quit;
            }
            KeyCode::Char('q') => {
                // 'q' closes modals/dialogs first, then returns to NoteList, then quits
                // Never quits from input modes (search_active is already handled above)
                if app.show_note_info {
                    app.show_note_info = false;
                } else if app.show_bulk_delete_confirm {
                    app.show_bulk_delete_confirm = false;
                } else if app.show_bulk_combine_confirm {
                    app.show_bulk_combine_confirm = false;
                } else if app.show_force_sync_confirm {
                    app.show_force_sync_confirm = false;
                } else if app.is_multi_select_mode {
                    // Exit multi-select mode
                    app.clear_multi_selection();
                } else {
                    match app.view_mode {
                        ViewMode::NoteList => {
                            app.state = AppState::Quit;
                        }
                        ViewMode::RecycleBin => {
                            app.view_mode = ViewMode::NoteList;
                            app.selected_note = 0;
                            app.preview_scroll_offset = 0;
                            // Reload normal notes
                            if let Err(e) = operations::notes::load_notes(app) {
                                app.error = Some(format!("Failed to reload notes: {}", e));
                            }
                        }
                        ViewMode::AttachmentViewer | ViewMode::VersionHistory | ViewMode::ConflictResolution | ViewMode::Inbox => {
                            app.view_mode = ViewMode::NoteList;
                        }
                    }
                }
            }
            KeyCode::Char('?') => {
                // Show help
                let prev = std::mem::replace(&mut app.state, AppState::Quit);
                app.state = AppState::Help {
                    previous: Box::new(prev),
                    scroll_offset: 0,
                };
            }
            KeyCode::Char('s') => {
                // Show settings
                let prev = std::mem::replace(&mut app.state, AppState::Quit);
                app.state = AppState::Settings {
                    previous: Box::new(prev),
                };
                app.input_mode = InputMode::Normal;
                app.selected_setting = 0;
                app.setting_input.clear();
                app.error = None;
            }
            KeyCode::Char('y') => {
                // Handle based on context
                if app.show_bulk_delete_confirm {
                    // Confirm bulk delete
                    app.show_bulk_delete_confirm = false;
                    match operations::bulk::delete_selected(app) {
                        Ok(deleted) => {
                            app.sync_status = Some(t!("bulk.deleted", count = deleted).to_string());
                        }
                        Err(e) => {
                            app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string());
                        }
                    }
                } else if app.show_bulk_combine_confirm {
                    // Confirm bulk combine
                    app.show_bulk_combine_confirm = false;
                    match operations::bulk::combine_selected(app) {
                        Ok(combined) => {
                            app.sync_status = Some(t!("bulk.combined", count = combined).to_string());
                        }
                        Err(e) => {
                            app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string());
                        }
                    }
                } else if app.show_force_sync_confirm {
                    // Confirm force full sync
                    app.show_force_sync_confirm = false;
                    operations::sync::force_full_sync(app);
                } else {
                    // Normal sync
                    operations::sync::trigger_sync(app);
                }
            }
            KeyCode::Char('Y') => {
                // Show force full sync confirmation
                app.show_force_sync_confirm = true;
            }
            KeyCode::Char('n') if app.show_force_sync_confirm || app.show_bulk_delete_confirm || app.show_bulk_combine_confirm => {
                // Cancel confirmations
                app.show_force_sync_confirm = false;
                app.show_bulk_delete_confirm = false;
                app.show_bulk_combine_confirm = false;
            }
            KeyCode::Char('/') => {
                // Enter search mode (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.search_active = true;
                    app.search_input.clear();
                    app.invalidate_filter_cache();
                }
            }
            KeyCode::Char('n') => {
                // New note - open editor immediately (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    // Check for existing blank note first
                    let existing_blank = app.notes.iter()
                        .find(|n| !n.deleted && n.is_blank())
                        .map(|n| n.id.clone());

                    if let Some(blank_id) = existing_blank {
                        // Reuse existing blank note
                        if let Some(note) = app.notes.iter_mut().find(|n| n.id == blank_id) {
                            note.touch(); // Update timestamps
                            // Update in database
                            if let (Some(db), Some(key)) = (&app.db, &app.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    app.error = Some(t!("note.save_failed", error = e.to_string()).to_string());
                                }
                            }
                        }
                        // Set up for editing this note
                        app.editing_note_id = Some(blank_id.clone());
                        if let Some(note) = app.notes.iter().find(|n| n.id == blank_id) {
                            app.note_input = note.content.clone();
                            app.note_syntax = note.syntax_language;
                            app.current_tags = note.tags.clone();
                        }
                    } else {
                        // Create new note - clear inputs
                        app.note_input.clear();
                        app.note_syntax = SyntaxLanguage::default();
                        app.current_tags.clear();
                        app.editing_note_id = None;
                    }

                    // Open external editor
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
                }
            }
            KeyCode::Char('i') | KeyCode::Enter => {
                // In version history: restore selected version
                if matches!(app.view_mode, ViewMode::VersionHistory) {
                    if !app.loaded_versions.is_empty() && app.selected_version < app.loaded_versions.len() {
                        let version_number = app.loaded_versions[app.selected_version].version;
                        match operations::notes::restore_version(app, version_number) {
                            Ok(()) => {
                                app.error = Some(t!("version.restored", number = version_number).to_string());
                                app.view_mode = ViewMode::NoteList;
                                app.loaded_versions.clear();
                                app.versions_note_id = None;
                                app.version_preview_scroll_offset = 0;
                            }
                            Err(e) => {
                                app.error = Some(t!("version.restore_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                }
                // In attachment viewer or when attachment panel is focused: view selected attachment
                else if matches!(app.view_mode, ViewMode::AttachmentViewer)
                    || (matches!(app.view_mode, ViewMode::NoteList) && app.focused_panel == FocusedPanel::Attachments)
                {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note = filtered[app.selected_note];
                        if app.selected_attachment < note.attachments.len() {
                            let attachment = note.attachments[app.selected_attachment].clone();
                            if let Err(e) = operations::attachments::view_attachment(app, &attachment) {
                                app.error = Some(t!("attachment.view_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                }
                // Edit selected note directly with external editor (only in note list view)
                else if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        // Clone data before modifying self
                        let note_id = filtered[app.selected_note].id.clone();
                        let is_locked = filtered[app.selected_note].locked;
                        let content = filtered[app.selected_note].content.clone();
                        let syntax_lang = filtered[app.selected_note].syntax_language;

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
                            let tags = filtered[app.selected_note].tags.clone();

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
                    }
                }
            }
            KeyCode::Tab => {
                // Toggle focus between note content and attachments panel
                if matches!(app.view_mode, ViewMode::NoteList) && !app.is_multi_select_mode {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note = filtered[app.selected_note];
                        // Only allow focus on attachments if note has attachments
                        if !note.attachments.is_empty() {
                            app.focused_panel = match app.focused_panel {
                                FocusedPanel::NoteContent => FocusedPanel::Attachments,
                                FocusedPanel::Attachments => FocusedPanel::NoteContent,
                            };
                            // Reset attachment selection when switching to attachments
                            if app.focused_panel == FocusedPanel::Attachments {
                                app.selected_attachment = 0;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Navigate attachments down (Shift+j) - legacy, kept for compatibility
                let filtered = app.filtered_notes();
                if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note = filtered[app.selected_note];
                    if !note.attachments.is_empty() && app.selected_attachment < note.attachments.len() - 1 {
                        app.selected_attachment += 1;
                    }
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Navigate attachments up (Shift+k) - legacy, kept for compatibility
                if app.selected_attachment > 0 {
                    app.selected_attachment -= 1;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if matches!(app.view_mode, ViewMode::AttachmentViewer) {
                    // Navigate attachments in viewer
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note = filtered[app.selected_note];
                        if app.selected_attachment < note.attachments.len().saturating_sub(1) {
                            app.selected_attachment += 1;
                        }
                    }
                } else if matches!(app.view_mode, ViewMode::VersionHistory) {
                    // Navigate versions in viewer
                    if app.selected_version < app.loaded_versions.len().saturating_sub(1) {
                        app.selected_version += 1;
                        app.version_preview_scroll_offset = 0; // Reset scroll when changing versions
                    }
                } else if app.focused_panel == FocusedPanel::Attachments {
                    // Navigate attachments when attachment panel is focused
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note = filtered[app.selected_note];
                        if !note.attachments.is_empty() && app.selected_attachment < note.attachments.len() - 1 {
                            app.selected_attachment += 1;
                        }
                    }
                } else {
                    // Navigate notes
                    let note_count = app.filtered_notes().len();
                    if note_count > 0 && app.selected_note < note_count - 1 {
                        app.selected_note += 1;
                        // Update selected note ID after index change
                        app.selected_note_id = app.filtered_notes().get(app.selected_note).map(|n| n.id.clone());
                        app.debug_log(&format!("Navigate down: selected_note={}, selected_note_id={:?}",
                            app.selected_note, app.selected_note_id));
                        app.preview_scroll_offset = 0;
                        // Reset focused panel when changing notes
                        app.focused_panel = FocusedPanel::NoteContent;
                    }
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if matches!(app.view_mode, ViewMode::AttachmentViewer) {
                    // Navigate attachments in viewer
                    if app.selected_attachment > 0 {
                        app.selected_attachment -= 1;
                    }
                } else if matches!(app.view_mode, ViewMode::VersionHistory) {
                    // Navigate versions in viewer
                    if app.selected_version > 0 {
                        app.selected_version -= 1;
                        app.version_preview_scroll_offset = 0; // Reset scroll when changing versions
                    }
                } else if app.focused_panel == FocusedPanel::Attachments {
                    // Navigate attachments when attachment panel is focused
                    if app.selected_attachment > 0 {
                        app.selected_attachment -= 1;
                    }
                } else {
                    // Navigate notes
                    if app.selected_note > 0 {
                        app.selected_note -= 1;
                        // Update selected note ID after index change
                        app.selected_note_id = app.filtered_notes().get(app.selected_note).map(|n| n.id.clone());
                        app.debug_log(&format!("Navigate up: selected_note={}, selected_note_id={:?}",
                            app.selected_note, app.selected_note_id));
                        app.preview_scroll_offset = 0;
                        // Reset focused panel when changing notes
                        app.focused_panel = FocusedPanel::NoteContent;
                    }
                }
            }
            KeyCode::Char('J') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Scroll preview content down in version history mode
                if matches!(app.view_mode, ViewMode::VersionHistory) {
                    app.version_preview_scroll_offset = app.version_preview_scroll_offset.saturating_add(3);
                }
            }
            KeyCode::Char('K') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Scroll preview content up in version history mode
                if matches!(app.view_mode, ViewMode::VersionHistory) {
                    app.version_preview_scroll_offset = app.version_preview_scroll_offset.saturating_sub(3);
                }
            }
            KeyCode::Char('p') => {
                // Toggle pin on selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note_id = filtered[app.selected_note].id.clone();
                        if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                        note.pinned = !note.pinned;
                        // Note: Don't update modified_at for UI state changes (pinning)
                        // Only content/tag modifications should trigger timestamp updates

                        // Save to database
                        if let (Some(db), Some(key)) = (&app.db, &app.key) {
                            let repo = NoteRepository::new(db.connection());
                            if let Err(e) = repo.update(note, key) {
                                app.error = Some(t!("note.pin_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                    }
                }
            }
            KeyCode::Char('t') => {
                // In multi-select mode: add tags to selected notes
                if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) {
                    app.input_mode = InputMode::BulkAddTags;
                    app.bulk_tags_input.clear();
                }
                // Edit tags for selected note (only in note list view)
                else if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        // Check if note is locked
                        if filtered[app.selected_note].locked {
                            app.sync_status = Some(t!("note.locked_cannot_tag").to_string());
                            return Ok(());
                        }

                        let content = filtered[app.selected_note].content.clone();
                        let note_id = filtered[app.selected_note].id.clone();
                        let syntax_lang = filtered[app.selected_note].syntax_language;
                        let tags = filtered[app.selected_note].tags.clone();

                        // Set up for tag editing
                        app.note_input = content;
                        app.note_syntax = syntax_lang;
                        app.current_tags = tags;
                        app.editing_note_id = Some(note_id);
                        app.state = AppState::NoteView;
                        app.input_mode = InputMode::Tag;
                        app.tag_input.clear();
                    }
                }
            }
            KeyCode::Char(']') => {
                // Cycle syntax language forward for selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note_id = filtered[app.selected_note].id.clone();
                    if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                        note.syntax_language = note.syntax_language.next();

                        // Save to database
                        if let (Some(db), Some(key)) = (&app.db, &app.key) {
                            let repo = NoteRepository::new(db.connection());
                            if let Err(e) = repo.update(note, key) {
                                app.error = Some(t!("syntax.change_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                    }
                }
            }
            KeyCode::Char('[') => {
                // Cycle syntax language backward for selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note_id = filtered[app.selected_note].id.clone();
                    if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                        note.syntax_language = note.syntax_language.prev();

                        // Save to database
                        if let (Some(db), Some(key)) = (&app.db, &app.key) {
                            let repo = NoteRepository::new(db.connection());
                            if let Err(e) = repo.update(note, key) {
                                app.error = Some(t!("syntax.change_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                    }
                }
            }
            KeyCode::Char('C') => {
                // Cycle color/category forward for selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note_id = filtered[app.selected_note].id.clone();

                        // Get current color before mutably borrowing
                        let current_color = app.notes.iter()
                            .find(|n| n.id == note_id)
                            .and_then(|n| n.color.as_ref())
                            .cloned();

                        // Calculate new color
                        let new_color = cycle_color_forward(current_color.as_ref(), app);

                        // Update note
                        if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                            note.color = new_color.clone();

                            // Save to database
                            if let (Some(db), Some(key)) = (&app.db, &app.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    app.error = Some(format!("Failed to update note color: {}", e));
                                }
                            }
                        }

                        // Show status message
                        if let Some(color) = new_color {
                            app.sync_status = Some(format!("Color: {}", color));
                            app.sync_status_set_at = Some(std::time::Instant::now());
                        } else {
                            app.sync_status = Some("Color: None".to_string());
                            app.sync_status_set_at = Some(std::time::Instant::now());
                        }
                    }
                }
            }
            KeyCode::Char('r') => {
                // Toggle recycle bin view or restore note
                match app.view_mode {
                    ViewMode::NoteList => {
                        // Switch to recycle bin view
                        app.view_mode = ViewMode::RecycleBin;
                        app.selected_note = 0;
                        app.preview_scroll_offset = 0;
                        if let Err(e) = operations::notes::load_deleted_notes(app) {
                            app.error = Some(t!("note.reload_failed", error = e.to_string()).to_string());
                        }
                    }
                    ViewMode::RecycleBin => {
                        // Restore selected note
                        if let Err(e) = operations::notes::restore_note(app) {
                            app.error = Some(t!("note.reload_failed", error = e.to_string()).to_string());
                        }
                    }
                    ViewMode::AttachmentViewer | ViewMode::VersionHistory | ViewMode::ConflictResolution | ViewMode::Inbox => {
                        // 'r' does nothing in these views
                    }
                }
            }
            KeyCode::Char('c') if !app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) => {
                // Cycle color/category backward for selected note (only in note list view, not in multi-select)
                let filtered = app.filtered_notes();
                if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note_id = filtered[app.selected_note].id.clone();

                    // Get current color before mutably borrowing
                    let current_color = app.notes.iter()
                        .find(|n| n.id == note_id)
                        .and_then(|n| n.color.as_ref())
                        .cloned();

                    // Calculate new color (backward)
                    let new_color = cycle_color_backward(current_color.as_ref(), app);

                    // Update note
                    if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                        note.color = new_color.clone();

                        // Save to database
                        if let (Some(db), Some(key)) = (&app.db, &app.key) {
                            let repo = NoteRepository::new(db.connection());
                            if let Err(e) = repo.update(note, key) {
                                app.error = Some(format!("Failed to update note color: {}", e));
                            }
                        }
                    }

                    // Show status message
                    if let Some(color) = new_color {
                        app.sync_status = Some(format!("Color: {}", color));
                        app.sync_status_set_at = Some(std::time::Instant::now());
                    } else {
                        app.sync_status = Some("Color: None".to_string());
                        app.sync_status_set_at = Some(std::time::Instant::now());
                    }
                }
            }
            KeyCode::Char('A') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Shift+A: Toggle archive mode (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.archive_mode = !app.archive_mode;
                    app.invalidate_filter_cache();
                    app.selected_note = 0;
                    app.preview_scroll_offset = 0;
                }
            }
            KeyCode::Char('a') if app.archive_mode && key.modifiers.is_empty() => {
                // 'a' in archive mode: Unarchive selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note_id = filtered[app.selected_note].id.clone();
                        if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                            note.archived = false;
                            note.touch();
                            // Save to database
                            if let (Some(db), Some(key)) = (&app.db, &app.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    app.error = Some(t!("note.archive_failed", error = e.to_string()).to_string());
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('L') => {
                // Toggle lock on selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note_id = filtered[app.selected_note].id.clone();
                        if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                            note.toggle_lock();
                            // Save to database
                            if let (Some(db), Some(key)) = (&app.db, &app.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    app.error = Some(t!("note.lock_failed", error = e.to_string()).to_string());
                                }
                            }
                        }
                    }
                }
            }
            KeyCode::Char('I') => {
                // Toggle note info modal (only in note list view with a selected note)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    if app.show_note_info {
                        app.show_note_info = false;
                    } else if !app.filtered_notes().is_empty() {
                        app.show_note_info = true;
                    }
                }
            }
            KeyCode::Char('b') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Open inbox (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.view_mode = ViewMode::Inbox;
                    app.selected_inbox_item = 0;
                    app.show_inbox_delete_confirm = false;
                    app.show_inbox_delete_all_confirm = false;
                }
            }
            KeyCode::Char('E') => {
                // Empty recycle bin (only in recycle bin view)
                if matches!(app.view_mode, ViewMode::RecycleBin) {
                    if let Err(e) = operations::notes::empty_trash(app) {
                        app.error = Some(t!("note.delete_failed", error = e.to_string()).to_string());
                    }
                }
            }
            KeyCode::Esc => {
                // Cancel confirmations/modals if showing
                if app.show_note_info {
                    app.show_note_info = false;
                } else if app.show_force_sync_confirm || app.show_bulk_delete_confirm || app.show_bulk_combine_confirm {
                    app.show_force_sync_confirm = false;
                    app.show_bulk_delete_confirm = false;
                    app.show_bulk_combine_confirm = false;
                } else if app.is_multi_select_mode {
                    // Clear multi-selection
                    app.clear_multi_selection();
                } else if app.archive_mode {
                    // Exit archive mode
                    app.archive_mode = false;
                    app.invalidate_filter_cache();
                    app.selected_note = 0;
                    app.preview_scroll_offset = 0;
                } else if matches!(app.view_mode, ViewMode::AttachmentViewer) {
                    // Exit attachment viewer
                    app.view_mode = ViewMode::NoteList;
                } else if matches!(app.view_mode, ViewMode::VersionHistory) {
                    // Exit version history viewer
                    app.view_mode = ViewMode::NoteList;
                    app.loaded_versions.clear();
                    app.versions_note_id = None;
                    app.version_preview_scroll_offset = 0;
                } else if matches!(app.view_mode, ViewMode::RecycleBin) {
                    // Exit recycle bin view
                    app.view_mode = ViewMode::NoteList;
                    app.selected_note = 0;
                    app.preview_scroll_offset = 0;
                    // Reload normal notes
                    if let Err(e) = operations::notes::load_notes(app) {
                        app.error = Some(format!("Failed to reload notes: {}", e));
                    }
                }
            }
            // Vim-style preview scrolling (must come before plain 'd' key)
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
            KeyCode::PageDown => {
                // Page Down: scroll preview down full page (20 lines)
                app.preview_scroll_offset = app.preview_scroll_offset.saturating_add(20);
            }
            KeyCode::PageUp => {
                // Page Up: scroll preview up full page (20 lines)
                app.preview_scroll_offset = app.preview_scroll_offset.saturating_sub(20);
            }
            KeyCode::Char('d') | KeyCode::Delete => {
                // Handle bulk delete confirmation
                if app.show_bulk_delete_confirm {
                    // This is a no-op here; 'y' confirms, 'n'/Esc cancels
                    return Ok(());
                }
                // In attachment viewer: delete selected attachment
                if matches!(app.view_mode, ViewMode::AttachmentViewer) {
                    if let Err(e) = operations::attachments::delete_current_attachment(app) {
                        app.error = Some(t!("attachment.add_failed", error = e.to_string()).to_string());
                    }
                }
                // Delete attachment when attachment panel is focused
                else if matches!(app.view_mode, ViewMode::NoteList) && app.focused_panel == FocusedPanel::Attachments {
                    if let Err(e) = operations::attachments::delete_current_attachment(app) {
                        app.error = Some(t!("attachment.delete_failed", error = e.to_string()).to_string());
                    }
                    // If no more attachments, switch focus back to content
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty()
                        && app.selected_note < filtered.len()
                        && filtered[app.selected_note].attachments.is_empty()
                    {
                        app.focused_panel = FocusedPanel::NoteContent;
                    }
                }
                // In multi-select mode: show bulk delete confirmation
                else if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) {
                    app.show_bulk_delete_confirm = true;
                }
                // Delete selected note (only in note list view)
                else if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        // Check if note is locked
                        if filtered[app.selected_note].locked {
                            app.sync_status = Some(t!("note.locked_cannot_delete").to_string());
                            return Ok(());
                        }

                        // Find the actual note in the full list
                        let note_to_delete = filtered[app.selected_note];
                        if let Some(pos) = app.notes.iter().position(|n| n.id == note_to_delete.id) {
                            app.selected_note = pos;
                            operations::notes::delete_note(app)?;
                            // Adjust selection after delete
                            let new_count = app.filtered_notes().len();
                            if app.selected_note >= new_count && app.selected_note > 0 {
                                app.selected_note -= 1;
                            }
                        }
                    }
                }
            }
            KeyCode::Char('c') if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) => {
                // In multi-select mode: show bulk combine confirmation (requires 2+ notes)
                if app.selected_note_ids.len() >= 2 {
                    app.show_bulk_combine_confirm = true;
                } else {
                    app.error = Some(t!("bulk.combine_requires_two").to_string());
                }
            }
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+A: Select all filtered notes (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.select_all_filtered();
                }
            }
            KeyCode::Char('a') if !app.archive_mode && key.modifiers.is_empty() => {
                // 'a' in normal mode: Open attachment viewer modal
                app.debug_log("'a' key pressed - attempting to open attachment viewer");
                let filtered = app.filtered_notes();
                app.debug_log(&format!("  filtered.len() = {}, selected_note = {}", filtered.len(), app.selected_note));
                if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note = filtered[app.selected_note];
                    app.debug_log(&format!("  note has {} attachments", note.attachments.len()));
                    if !note.attachments.is_empty() {
                        app.debug_log("  Setting view_mode to AttachmentViewer");
                        app.view_mode = ViewMode::AttachmentViewer;
                        app.selected_attachment = 0;
                    } else {
                        app.debug_log("  No attachments - setting error");
                        app.error = Some(t!("attachment.no_attachments").to_string());
                    }
                } else {
                    app.debug_log("  Filtered is empty or selected_note out of bounds");
                }
            }
            KeyCode::Char('v') => {
                // Open version history viewer modal
                let filtered = app.filtered_notes();
                if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note_id = filtered[app.selected_note].id.clone();
                    if let Err(e) = operations::notes::load_versions_for_note(app, &note_id) {
                        app.error = Some(t!("version.restore_failed", error = e.to_string()).to_string());
                    } else if app.loaded_versions.is_empty() {
                        app.error = Some(t!("version.no_versions").to_string());
                    } else {
                        app.view_mode = ViewMode::VersionHistory;
                        app.selected_version = 0;
                    }
                }
            }
            KeyCode::Char(c @ '1'..='9') => {
                // In attachment viewer: select and view attachment by number (1-9)
                if matches!(app.view_mode, ViewMode::AttachmentViewer) {
                    let attachment_index = (c as usize) - ('1' as usize);
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note = filtered[app.selected_note];
                        if attachment_index < note.attachments.len() {
                            let attachment = note.attachments[attachment_index].clone();
                            if let Err(e) = operations::attachments::view_attachment(app, &attachment) {
                                app.error = Some(t!("attachment.view_failed", error = e.to_string()).to_string());
                            }
                        }
                    }
                }
            }
            KeyCode::Char('X') => {
                // Remove selected attachment (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    if let Err(e) = operations::attachments::remove_attachment_from_current_note(app) {
                        app.error = Some(t!("attachment.add_failed", error = e.to_string()).to_string());
                    }
                }
            }
            KeyCode::Char(' ') => {
                // Toggle multi-select for current note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let selected = app.selected_note;
                    app.toggle_note_selection(selected);
                }
            }
            KeyCode::Char('V') => {
                // Shift+V: Range select from last selected to current (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let selected = app.selected_note;
                    app.select_range(selected);
                }
            }
            KeyCode::Char('e') => {
                // In multi-select mode: export selected notes (only in note list view)
                if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) {
                    // Generate default filename
                    let timestamp = chrono::Local::now().format("%Y-%m-%d").to_string();
                    app.bulk_export_path_input = format!("jottery-export-{}.json", timestamp);
                    app.input_mode = InputMode::BulkExportPath;
                }
            }
            KeyCode::Char('x') => {
                // Open conflict resolution for selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        let note_id = filtered[app.selected_note].id.clone();
                        // Check if this note has a conflict and open resolution
                        if let Err(e) = operations::sync::open_conflict_resolution(app, &note_id) {
                            // If there's no conflict, this just shows a message
                            if e.to_string().contains("No conflict") {
                                app.error = Some(t!("conflict.no_conflict").to_string());
                            } else {
                                app.error = Some(format!("{}: {}", t!("conflict.resolve_failed"), e));
                            }
                        }
                    }
                }
            }
            _ => {
                // Reset 'a' key flag for any other key
                app.last_key_was_a = false;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use tempfile::tempdir;
    use crate::models::Note;
    use crate::ui::state::FocusedPanel;

    /// Create a test app with minimal setup
    fn create_test_app() -> App {
        let temp_dir = tempdir().expect("Failed to create temp dir");
        let db_path = temp_dir.path().join("test.db");
        let mut app = App::new(db_path, None).expect("Failed to create app");
        app.state = AppState::NoteList;
        // Leak the temp_dir so it stays alive for the test
        std::mem::forget(temp_dir);
        app
    }

    /// Create a key event with no modifiers
    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::NONE)
    }

    /// Create a key event with Ctrl modifier
    fn ctrl_key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::CONTROL)
    }

    // ==================== Syntax Cycling Tests ====================

    #[test]
    fn test_bracket_right_cycles_syntax_forward() {
        let mut app = create_test_app();
        // Add a test note
        let mut note = Note::new("Test content".to_string());
        note.syntax_language = crate::models::SyntaxLanguage::Plain;
        app.notes.push(note);
        app.view_mode = ViewMode::NoteList;

        let initial_syntax = app.notes[0].syntax_language;
        let _ = handle_note_list_key(&mut app, key(KeyCode::Char(']')));

        // Syntax should have changed to next value
        assert_ne!(app.notes[0].syntax_language, initial_syntax);
    }

    #[test]
    fn test_bracket_left_cycles_syntax_backward() {
        let mut app = create_test_app();
        // Add a test note with a non-default syntax
        let mut note = Note::new("Test content".to_string());
        note.syntax_language = crate::models::SyntaxLanguage::Python;
        app.notes.push(note);
        app.view_mode = ViewMode::NoteList;

        let initial_syntax = app.notes[0].syntax_language;
        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('[')));

        // Syntax should have changed to previous value
        assert_ne!(app.notes[0].syntax_language, initial_syntax);
    }

    #[test]
    fn test_syntax_cycling_only_works_in_note_list_view() {
        let mut app = create_test_app();
        let mut note = Note::new("Test content".to_string());
        note.syntax_language = crate::models::SyntaxLanguage::Plain;
        app.notes.push(note);

        // Set to RecycleBin view
        app.view_mode = ViewMode::RecycleBin;
        let initial_syntax = app.notes[0].syntax_language;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char(']')));

        // Syntax should NOT have changed
        assert_eq!(app.notes[0].syntax_language, initial_syntax);
    }

    // ==================== Search Mode Tab Tests ====================

    #[test]
    fn test_tab_exits_search_mode() {
        let mut app = create_test_app();
        app.search_active = true;
        app.search_input = "test".to_string();

        let _ = handle_note_list_key(&mut app, key(KeyCode::Tab));

        assert!(!app.search_active);
    }

    #[test]
    fn test_tab_in_search_focuses_attachments_when_available() {
        let mut app = create_test_app();
        // Add a note with attachments
        let mut note = Note::new("Test content".to_string());
        note.attachments.push(crate::models::Attachment {
            id: "test-attachment".to_string(),
            filename: "test.txt".to_string(),
            mime_type: "text/plain".to_string(),
            size: 100,
            data: String::new(),
            thumbnail_data: None,
        });
        app.notes.push(note);
        app.search_active = true;
        app.focused_panel = FocusedPanel::NoteContent;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Tab));

        assert!(!app.search_active);
        assert_eq!(app.focused_panel, FocusedPanel::Attachments);
        assert_eq!(app.selected_attachment, 0);
    }

    #[test]
    fn test_tab_in_search_stays_content_when_no_attachments() {
        let mut app = create_test_app();
        // Add a note without attachments
        app.notes.push(Note::new("Test content".to_string()));
        app.search_active = true;
        app.focused_panel = FocusedPanel::NoteContent;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Tab));

        assert!(!app.search_active);
        assert_eq!(app.focused_panel, FocusedPanel::NoteContent);
    }

    #[test]
    fn test_typing_q_in_search_adds_to_query() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.search_active = true;
        app.search_input = "test".to_string();

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        // Should add 'q' to search, NOT quit
        assert!(app.search_active);
        assert_eq!(app.search_input, "testq");
        assert!(!matches!(app.state, AppState::Quit));
    }

    // ==================== PageUp/PageDown Tests ====================

    #[test]
    fn test_page_down_scrolls_preview() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.preview_scroll_offset = 0;

        let _ = handle_note_list_key(&mut app, key(KeyCode::PageDown));

        assert_eq!(app.preview_scroll_offset, 20);
    }

    #[test]
    fn test_page_up_scrolls_preview() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.preview_scroll_offset = 40;

        let _ = handle_note_list_key(&mut app, key(KeyCode::PageUp));

        assert_eq!(app.preview_scroll_offset, 20);
    }

    #[test]
    fn test_page_up_does_not_go_negative() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.preview_scroll_offset = 5;

        let _ = handle_note_list_key(&mut app, key(KeyCode::PageUp));

        assert_eq!(app.preview_scroll_offset, 0);
    }

    #[test]
    fn test_ctrl_d_scrolls_half_page() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.preview_scroll_offset = 0;

        let _ = handle_note_list_key(&mut app, ctrl_key(KeyCode::Char('d')));

        assert_eq!(app.preview_scroll_offset, 10);
    }

    #[test]
    fn test_ctrl_u_scrolls_half_page() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.preview_scroll_offset = 20;

        let _ = handle_note_list_key(&mut app, ctrl_key(KeyCode::Char('u')));

        assert_eq!(app.preview_scroll_offset, 10);
    }

    // ==================== 'q' Key Layered Close Tests ====================

    #[test]
    fn test_q_closes_bulk_delete_confirm_first() {
        let mut app = create_test_app();
        app.show_bulk_delete_confirm = true;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(!app.show_bulk_delete_confirm);
        assert!(!matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_q_closes_bulk_combine_confirm_first() {
        let mut app = create_test_app();
        app.show_bulk_combine_confirm = true;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(!app.show_bulk_combine_confirm);
        assert!(!matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_q_closes_force_sync_confirm_first() {
        let mut app = create_test_app();
        app.show_force_sync_confirm = true;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(!app.show_force_sync_confirm);
        assert!(!matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_q_exits_multi_select_mode_first() {
        let mut app = create_test_app();
        app.is_multi_select_mode = true;
        app.selected_note_ids.insert("test-id".to_string());
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(!app.is_multi_select_mode);
        assert!(app.selected_note_ids.is_empty());
        assert!(!matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_q_returns_from_recycle_bin_to_note_list() {
        let mut app = create_test_app();
        app.view_mode = ViewMode::RecycleBin;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(matches!(app.view_mode, ViewMode::NoteList));
        assert!(!matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_q_returns_from_version_history_to_note_list() {
        let mut app = create_test_app();
        app.view_mode = ViewMode::VersionHistory;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(matches!(app.view_mode, ViewMode::NoteList));
        assert!(!matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_q_quits_from_note_list_view() {
        let mut app = create_test_app();
        app.view_mode = ViewMode::NoteList;
        app.is_multi_select_mode = false;
        app.show_bulk_delete_confirm = false;
        app.show_bulk_combine_confirm = false;
        app.show_force_sync_confirm = false;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('q')));

        assert!(matches!(app.state, AppState::Quit));
    }

    #[test]
    fn test_ctrl_q_always_quits() {
        let mut app = create_test_app();
        app.view_mode = ViewMode::RecycleBin;
        app.is_multi_select_mode = true;
        app.show_bulk_delete_confirm = true;

        let _ = handle_note_list_key(&mut app, ctrl_key(KeyCode::Char('q')));

        assert!(matches!(app.state, AppState::Quit));
    }

    // ==================== Multi-Select Mode Tests ====================

    #[test]
    fn test_space_enters_multi_select_mode() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.view_mode = ViewMode::NoteList;
        app.is_multi_select_mode = false;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char(' ')));

        assert!(app.is_multi_select_mode);
    }

    #[test]
    fn test_space_toggles_note_selection() {
        let mut app = create_test_app();
        let note = Note::new("Test content".to_string());
        let note_id = note.id.clone();
        app.notes.push(note);
        app.view_mode = ViewMode::NoteList;
        app.is_multi_select_mode = true;
        app.selected_note = 0;

        // First space selects
        let _ = handle_note_list_key(&mut app, key(KeyCode::Char(' ')));
        assert!(app.selected_note_ids.contains(&note_id));

        // Second space deselects
        let _ = handle_note_list_key(&mut app, key(KeyCode::Char(' ')));
        assert!(!app.selected_note_ids.contains(&note_id));
    }

    #[test]
    fn test_ctrl_a_selects_all_notes() {
        let mut app = create_test_app();
        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());
        let note3 = Note::new("Note 3".to_string());
        let id1 = note1.id.clone();
        let id2 = note2.id.clone();
        let id3 = note3.id.clone();
        app.notes.push(note1);
        app.notes.push(note2);
        app.notes.push(note3);
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, ctrl_key(KeyCode::Char('a')));

        assert!(app.is_multi_select_mode);
        assert!(app.selected_note_ids.contains(&id1));
        assert!(app.selected_note_ids.contains(&id2));
        assert!(app.selected_note_ids.contains(&id3));
        assert_eq!(app.selected_note_ids.len(), 3);
    }

    #[test]
    fn test_esc_clears_multi_selection() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.view_mode = ViewMode::NoteList;
        app.is_multi_select_mode = true;
        app.selected_note_ids.insert("test-id".to_string());

        let _ = handle_note_list_key(&mut app, key(KeyCode::Esc));

        assert!(!app.is_multi_select_mode);
        assert!(app.selected_note_ids.is_empty());
    }

    // ==================== BulkAddTags Modal Tests ====================

    #[test]
    fn test_t_in_multi_select_opens_bulk_tags_modal() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.view_mode = ViewMode::NoteList;
        app.is_multi_select_mode = true;
        app.selected_note_ids.insert("test-id".to_string());

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('t')));

        assert!(matches!(app.input_mode, InputMode::BulkAddTags));
        assert!(app.bulk_tags_input.is_empty());
    }

    #[test]
    fn test_esc_cancels_bulk_tags_input() {
        let mut app = create_test_app();
        app.input_mode = InputMode::BulkAddTags;
        app.bulk_tags_input = "tag1, tag2".to_string();

        let _ = handle_note_list_key(&mut app, key(KeyCode::Esc));

        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    // ==================== BulkExportPath Modal Tests ====================

    #[test]
    fn test_e_in_multi_select_opens_export_path_modal() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.view_mode = ViewMode::NoteList;
        app.is_multi_select_mode = true;
        app.selected_note_ids.insert("test-id".to_string());

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('e')));

        assert!(matches!(app.input_mode, InputMode::BulkExportPath));
        // Should have a default filename with timestamp
        assert!(app.bulk_export_path_input.starts_with("jottery-export-"));
        assert!(app.bulk_export_path_input.ends_with(".json"));
    }

    #[test]
    fn test_esc_cancels_bulk_export_input() {
        let mut app = create_test_app();
        app.input_mode = InputMode::BulkExportPath;
        app.bulk_export_path_input = "export.json".to_string();

        let _ = handle_note_list_key(&mut app, key(KeyCode::Esc));

        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    // ==================== Navigation Tests ====================

    #[test]
    fn test_j_moves_down() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Note 1".to_string()));
        app.notes.push(Note::new("Note 2".to_string()));
        app.notes.push(Note::new("Note 3".to_string()));
        app.selected_note = 0;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('j')));

        assert_eq!(app.selected_note, 1);
    }

    #[test]
    fn test_k_moves_up() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Note 1".to_string()));
        app.notes.push(Note::new("Note 2".to_string()));
        app.notes.push(Note::new("Note 3".to_string()));
        app.selected_note = 2;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('k')));

        assert_eq!(app.selected_note, 1);
    }

    #[test]
    fn test_down_arrow_moves_down() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Note 1".to_string()));
        app.notes.push(Note::new("Note 2".to_string()));
        app.selected_note = 0;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Down));

        assert_eq!(app.selected_note, 1);
    }

    #[test]
    fn test_up_arrow_moves_up() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Note 1".to_string()));
        app.notes.push(Note::new("Note 2".to_string()));
        app.selected_note = 1;
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Up));

        assert_eq!(app.selected_note, 0);
    }

    // ==================== Search Mode Entry Tests ====================

    #[test]
    fn test_slash_enters_search_mode() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.view_mode = ViewMode::NoteList;
        app.search_active = false;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('/')));

        assert!(app.search_active);
    }

    #[test]
    fn test_esc_exits_search_mode() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.search_active = true;
        app.search_input = "test query".to_string();

        let _ = handle_note_list_key(&mut app, key(KeyCode::Esc));

        assert!(!app.search_active);
        assert!(app.search_input.is_empty());
    }

    // ==================== Help Screen Tests ====================

    #[test]
    fn test_question_mark_shows_help() {
        let mut app = create_test_app();
        app.notes.push(Note::new("Test content".to_string()));
        app.view_mode = ViewMode::NoteList;

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('?')));

        assert!(matches!(app.state, AppState::Help { .. }));
    }
}
