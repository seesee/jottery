//! Input handling for the note list screen

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;

use crate::repository::NoteRepository;
use crate::ui::app::App;
use crate::ui::input::text_input::{handle_text_input, TextInputResult};
use crate::ui::operations;
use crate::ui::state::{AppState, FocusedPanel, InputMode, ViewMode};

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

/// Export the selection, stopping to ask if that would replace a file.
fn export_selection(app: &mut App, path: std::path::PathBuf, overwrite: bool) {
    match operations::bulk::export_selected(app, &path, overwrite) {
        Ok(count) => {
            app.sync_status = Some(t!("bulk.exported", count = count, path = path.display().to_string()).to_string());
            app.clear_multi_selection();
        }
        Err(e) if e.is::<crate::export::ExportFileExists>() => app.export_overwrite_path = Some(path),
        Err(e) => app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string()),
    }
}

/// Handle key events in note list state
pub fn handle_note_list_key(app: &mut App, key: KeyEvent) -> Result<()> {
    // Clear sync status on any key (except 'y' which sets it)
    if key.code != KeyCode::Char('y') {
        app.sync_status = None;
    }

    // An error stays on screen until the next key
    app.error = None;

    // The keyboard's place among the links lasts until it's used or another key is pressed
    if !matches!(key.code, KeyCode::Char('l') | KeyCode::Enter) {
        app.selected_link = None;
    }

    // Naming a search, whichever list is underneath
    if matches!(app.input_mode, InputMode::SaveSearchName) {
        match handle_text_input(&key, &mut app.saved_search_name_input) {
            TextInputResult::Submit => {
                if let Err(e) = operations::saved_searches::save_current(app) {
                    app.error = Some(t!("saved_search.failed", error = e.to_string()).to_string());
                }
            }
            TextInputResult::Cancel => app.input_mode = InputMode::Normal,
            TextInputResult::Continue | TextInputResult::Unhandled => {}
        }
        return Ok(());
    }

    if matches!(app.view_mode, ViewMode::SavedSearches) {
        super::saved_searches::handle_saved_searches_key(app, key);
        return Ok(());
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

    // Emptying the bin can't be undone, so nothing else gets through while it's asked
    if app.show_empty_bin_confirm {
        match key.code {
            KeyCode::Char('y') => {
                app.show_empty_bin_confirm = false;
                if let Err(e) = operations::notes::empty_trash(app) {
                    app.error = Some(t!("note.delete_failed", error = e.to_string()).to_string());
                }
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => {
                app.show_empty_bin_confirm = false;
            }
            _ => {}
        }
        return Ok(());
    }

    // The server wants a lot of notes gone. Nothing else gets through until that's
    // answered, and leaving it unanswered keeps the notes where they are.
    if app.held_deletions > 0 {
        let answered = match key.code {
            KeyCode::Char('y') => Some(operations::sync::apply_held_deletions(app)),
            KeyCode::Char('n') => Some(operations::sync::refuse_held_deletions(app)),
            KeyCode::Char('q') | KeyCode::Esc => {
                app.held_deletions = 0;
                None
            }
            _ => None,
        };
        if let Some(Err(e)) = answered {
            app.error = Some(t!("note.delete_failed", error = e.to_string()).to_string());
        }
        return Ok(());
    }

    // An attachment that won't be opened: the only offer is to save it
    if app.attachment_to_confirm.is_some() {
        match key.code {
            KeyCode::Char('y') => {
                if let Some(attachment) = app.attachment_to_confirm.take() {
                    operations::attachments::begin_save_attachment(app, attachment);
                }
            }
            KeyCode::Char('n') | KeyCode::Char('q') | KeyCode::Esc => {
                app.attachment_to_confirm = None;
            }
            _ => {}
        }
        return Ok(());
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

    // Handle bulk remove tags input mode
    if matches!(app.input_mode, InputMode::BulkRemoveTags) {
        match handle_text_input(&key, &mut app.bulk_tags_input) {
            TextInputResult::Submit => {
                let tags: Vec<String> = app.bulk_tags_input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();

                if !tags.is_empty() {
                    match operations::bulk::remove_tags_from_selected(app, &tags) {
                        Ok(count) => app.notify(t!("bulk.tags_removed", count = count, tags = tags.join(", ")).to_string()),
                        Err(e) => app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string()),
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

    // Handle bulk colour input mode
    if matches!(app.input_mode, InputMode::BulkSetColour) {
        match handle_text_input(&key, &mut app.bulk_tags_input) {
            TextInputResult::Submit => {
                let colour = app.bulk_tags_input.trim().to_lowercase();
                match operations::bulk::set_colour_of_selected(app, &colour) {
                    Ok(count) => app.notify(t!("bulk.colour_set", count = count).to_string()),
                    Err(e) => app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string()),
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

    // An export would replace a file, which can't be undone
    if let Some(path) = app.export_overwrite_path.take() {
        if matches!(key.code, KeyCode::Char('y') | KeyCode::Char('Y')) {
            export_selection(app, path, true);
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
                    export_selection(app, path, false);
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
    if super::attachment_path::handle_attachment_path_key(app, key)? {
        return Ok(());
    }

    // Handle search mode
    if super::search::handle_search_key(app, key)? {
        return Ok(());
    }

    // A held Ctrl or Alt makes a key something other than its letter. Only these have a
    // meaning with Ctrl, which the arms below look for.
    if let KeyCode::Char(c) = key.code {
        let unbound_ctrl = key.modifiers.contains(KeyModifiers::CONTROL) && !matches!(c, 'q' | 'd' | 'u' | 'f' | 'b' | 'a');
        if unbound_ctrl || key.modifiers.contains(KeyModifiers::ALT) {
            return Ok(());
        }
    }

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
                            app.select_note_at(0);
                            app.preview_scroll_offset = 0;
                            // Reload normal notes
                            if let Err(e) = operations::notes::load_notes(app) {
                                app.error = Some(format!("Failed to reload notes: {}", e));
                            }
                        }
                        ViewMode::AttachmentViewer | ViewMode::VersionHistory | ViewMode::ConflictResolution
                        | ViewMode::Inbox | ViewMode::SavedSearches => {
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
                        .find(|n| !n.deleted && !n.archived && n.is_blank())
                        .map(|n| n.id.clone());

                    if let Some(blank_id) = existing_blank {
                        // Reuse existing blank note, which only changes if text goes in it
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
                        app.note_syntax = app.settings.default_note_type;
                        app.current_tags.clear();
                        app.editing_note_id = None;
                    }

                    // Open external editor
                    operations::notes::edit_and_save(app);
                }
            }
            KeyCode::Enter if app.selected_link.is_some() && operations::notes::follow_focused_link(app) => {}
            KeyCode::Char('i') | KeyCode::Enter => {
                // In version history: restore selected version
                if matches!(app.view_mode, ViewMode::VersionHistory) {
                    if !app.loaded_versions.is_empty() && app.selected_version < app.loaded_versions.len() {
                        let version_number = app.loaded_versions[app.selected_version].version;
                        match operations::notes::restore_version(app, version_number) {
                            Ok(()) => {
                                app.notify(t!("version.restored", number = version_number).to_string());
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
                            operations::notes::edit_and_save(app);
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

                            // Pinning changes the order
                            app.invalidate_filter_cache();
                            app.keep_selection_on_note();

                            // Trigger sync to push pin change to server
                            if app.settings.sync_enabled {
                                app.trigger_sync();
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
                        note.set_syntax(note.syntax_language.next());

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
                        note.set_syntax(note.syntax_language.prev());

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
            KeyCode::Char('T') if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) => {
                // In multi-select mode: remove tags from selected notes
                app.input_mode = InputMode::BulkRemoveTags;
                app.bulk_tags_input.clear();
            }
            KeyCode::Char('C') if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) => {
                // In multi-select mode: set the colour of selected notes
                app.input_mode = InputMode::BulkSetColour;
                app.bulk_tags_input.clear();
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
                        app.select_note_at(0);
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
                    ViewMode::AttachmentViewer | ViewMode::VersionHistory | ViewMode::ConflictResolution
                    | ViewMode::Inbox | ViewMode::SavedSearches => {
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
                    app.select_note_at(0);
                    app.preview_scroll_offset = 0;
                }
            }
            KeyCode::Char('a') if app.archive_mode && key.modifiers.is_empty() => {
                // 'a' in archive mode: Unarchive selected note (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    operations::notes::toggle_archive_selected(app);
                }
            }
            KeyCode::Char('z') => {
                // Archive the selected notes, or bring one back in archive mode
                if matches!(app.view_mode, ViewMode::NoteList) {
                    if app.is_multi_select_mode {
                        match operations::bulk::archive_selected(app) {
                            Ok(count) => app.notify(t!("bulk.archived", count = count).to_string()),
                            Err(e) => app.error = Some(t!("bulk.operation_failed", error = e.to_string()).to_string()),
                        }
                    } else {
                        operations::notes::toggle_archive_selected(app);
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

                            // Locking touches the note, which changes the order
                            app.invalidate_filter_cache();
                            app.keep_selection_on_note();
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
            KeyCode::Char('D') if matches!(app.view_mode, ViewMode::NoteList) => {
                if let Err(e) = operations::notes::duplicate_selected(app) {
                    app.error = Some(e.to_string());
                }
            }
            KeyCode::Char('o') if matches!(app.view_mode, ViewMode::NoteList) => {
                if let Err(e) = operations::notes::copy_selected(app) {
                    app.error = Some(e.to_string());
                }
            }
            KeyCode::Char('w') if matches!(app.view_mode, ViewMode::NoteList) => {
                if let Err(e) = operations::notes::toggle_word_wrap(app) {
                    app.error = Some(e.to_string());
                }
            }
            KeyCode::Char('l') if matches!(app.view_mode, ViewMode::NoteList) => {
                operations::notes::focus_next_link(app);
            }
            KeyCode::Char('g') if matches!(app.view_mode, ViewMode::NoteList) => {
                if let Err(e) = operations::saved_searches::open_picker(app) {
                    app.error = Some(t!("saved_search.failed", error = e.to_string()).to_string());
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
                // Empty recycle bin, once confirmed (only in recycle bin view)
                if matches!(app.view_mode, ViewMode::RecycleBin) && !app.notes.is_empty() {
                    app.show_empty_bin_confirm = true;
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
                    app.select_note_at(0);
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
                    app.select_note_at(0);
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
                        app.error = Some(t!("attachment.delete_failed", error = e.to_string()).to_string());
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

                        let note_id = filtered[app.selected_note].id.clone();
                        operations::notes::delete_note(app, &note_id)?;
                    }
                }
            }
            KeyCode::Char('c') if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) => {
                // In multi-select mode: show bulk combine confirmation (requires 2+ notes)
                if app.editable_selection().len() >= 2 {
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
            KeyCode::Char('+') => {
                // Attach a file to the selected note
                if matches!(app.view_mode, ViewMode::NoteList | ViewMode::AttachmentViewer) && !app.is_multi_select_mode {
                    if let Err(e) = operations::attachments::begin_add_attachment(app) {
                        app.error = Some(t!("attachment.add_failed", error = e.to_string()).to_string());
                    }
                }
            }
            KeyCode::Char('S') => {
                // Save the selected attachment to a file
                let in_attachments = matches!(app.view_mode, ViewMode::AttachmentViewer)
                    || (matches!(app.view_mode, ViewMode::NoteList) && app.focused_panel == FocusedPanel::Attachments);
                if in_attachments {
                    if let Some(attachment) = operations::attachments::selected_attachment(app) {
                        operations::attachments::begin_save_attachment(app, attachment);
                    }
                }
            }
            KeyCode::Char('X') => {
                // Remove selected attachment (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    if let Err(e) = operations::attachments::remove_attachment_from_current_note(app) {
                        app.error = Some(t!("attachment.delete_failed", error = e.to_string()).to_string());
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

    /// A vault of `count` notes, in the app and the database, in the order given.
    fn app_with_notes(contents: &[&str]) -> App {
        let mut app = create_test_app();
        let key = crate::crypto::CryptoService::new().generate_master_key();
        let db = crate::db::Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        for (index, content) in contents.iter().enumerate() {
            let mut note = Note::new(content.to_string());
            note.modified_at = chrono::Utc::now() - chrono::Duration::minutes(index as i64);
            NoteRepository::new(db.connection()).create(&note, &key).unwrap();
            app.notes.push(note);
        }
        app.db = Some(db);
        app.key = Some(key);
        app.invalidate_filter_cache();
        app.select_note_at(0);
        app
    }

    fn highlighted(app: &App) -> String {
        app.filtered_notes()[app.selected_note].content.clone()
    }

    #[test]
    fn deleting_moves_the_highlight_to_the_next_note() {
        let mut app = app_with_notes(&["first", "second", "third"]);
        app.select_note_at(1);

        handle_note_list_key(&mut app, key(KeyCode::Char('d'))).unwrap();

        assert_eq!(highlighted(&app), "third");
        handle_note_list_key(&mut app, key(KeyCode::Char('d'))).unwrap();
        assert_eq!(highlighted(&app), "first");
    }

    /// `notes` and the visible list are in different orders. The index used to be swapped
    /// between them, so a repeated delete could hit a note the user never highlighted.
    #[test]
    fn a_repeated_delete_only_takes_what_was_highlighted() {
        let mut app = app_with_notes(&["first", "second", "third", "fourth"]);
        app.notes.reverse();
        app.invalidate_filter_cache();
        app.select_note_at(1);
        assert_eq!(highlighted(&app), "second");

        handle_note_list_key(&mut app, key(KeyCode::Char('d'))).unwrap();
        assert_eq!(highlighted(&app), "third");

        handle_note_list_key(&mut app, key(KeyCode::Char('d'))).unwrap();
        assert_eq!(highlighted(&app), "fourth");

        let left: Vec<&str> = app.filtered_notes().iter().map(|n| n.content.as_str()).collect();
        assert_eq!(left, vec!["first", "fourth"]);
    }

    #[test]
    fn pinning_keeps_the_highlight_on_the_note() {
        let mut app = app_with_notes(&["first", "second", "third"]);
        app.select_note_at(2);

        handle_note_list_key(&mut app, key(KeyCode::Char('p'))).unwrap();

        assert_eq!(app.selected_note, 0);
        assert_eq!(highlighted(&app), "third");
    }

    #[test]
    fn a_reload_keeps_the_highlight_on_the_note_however_it_got_there() {
        let mut app = app_with_notes(&["first", "second", "third"]);
        handle_note_list_key(&mut app, key(KeyCode::Char('j'))).unwrap();
        assert_eq!(highlighted(&app), "second");

        // A sync brings in a newer note above it
        let db = app.db.as_ref().unwrap();
        let key_bytes = app.key.unwrap();
        let mut newest = Note::new("newest".to_string());
        newest.modified_at = chrono::Utc::now() + chrono::Duration::hours(1);
        NoteRepository::new(db.connection()).create(&newest, &key_bytes).unwrap();
        operations::notes::load_notes(&mut app).unwrap();

        assert_eq!(highlighted(&app), "second");
    }

    fn bin_with(app: &mut App, contents: &[&str]) {
        let db = app.db.as_ref().unwrap();
        let key_bytes = app.key.unwrap();
        for (index, content) in contents.iter().enumerate() {
            let mut note = Note::new(content.to_string());
            note.deleted = true;
            note.deleted_at = Some(chrono::Utc::now());
            note.modified_at = chrono::Utc::now() - chrono::Duration::minutes(index as i64);
            NoteRepository::new(db.connection()).create(&note, &key_bytes).unwrap();
        }
        operations::notes::load_deleted_notes(app).unwrap();
        app.view_mode = ViewMode::RecycleBin;
        app.select_note_at(0);
    }

    /// The bin is drawn in display order, and restoring used to index the stored order.
    #[test]
    fn restoring_from_the_bin_takes_the_note_highlighted() {
        let mut app = app_with_notes(&[]);
        bin_with(&mut app, &["binned first", "binned second", "binned third"]);
        let chosen = highlighted(&app);
        assert_ne!(chosen, app.notes[0].content, "the two orders have to differ for this to mean anything");

        operations::notes::restore_note(&mut app).unwrap();

        let db = app.db.as_ref().unwrap();
        let live = NoteRepository::new(db.connection()).list(false, &app.key.unwrap()).unwrap();
        assert_eq!(live.len(), 1);
        assert_eq!(live[0].content, chosen);
    }

    /// Enter opens the editor, and quitting it unchanged is the usual way to read a note.
    #[test]
    fn opening_a_note_and_closing_it_changes_nothing() {
        let mut app = app_with_notes(&["unchanged"]);
        let before = app.notes[0].clone();
        let db = app.db.as_ref().unwrap();
        let key_bytes = app.key.unwrap();
        let mut synced = NoteRepository::new(db.connection()).get(&before.id, &key_bytes).unwrap().unwrap();
        synced.needs_sync = false;
        NoteRepository::new(db.connection()).store(&synced, &key_bytes).unwrap();

        app.note_input = before.content.clone();
        app.current_tags = before.tags.clone();
        app.editing_note_id = Some(before.id.clone());
        operations::notes::save_note(&mut app).unwrap();

        let db = app.db.as_ref().unwrap();
        let after = NoteRepository::new(db.connection()).get(&before.id, &key_bytes).unwrap().unwrap();
        assert_eq!(after.modified_at, before.modified_at);
        assert_eq!(after.version, before.version);
        assert!(!after.needs_sync);
        assert_eq!(app.notes[0].version, before.version);
    }

    #[test]
    fn a_real_edit_still_saves() {
        let mut app = app_with_notes(&["before"]);
        let id = app.notes[0].id.clone();

        app.note_input = "after".to_string();
        app.editing_note_id = Some(id.clone());
        operations::notes::save_note(&mut app).unwrap();

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&id, &app.key.unwrap()).unwrap().unwrap();
        assert_eq!(stored.content, "after");
        assert!(stored.needs_sync);
    }

    #[test]
    fn a_large_deletion_waits_for_an_answer_before_any_other_key() {
        let mut app = app_with_notes(&["first"]);
        app.held_deletions = 12;

        handle_note_list_key(&mut app, key(KeyCode::Char('d'))).unwrap();
        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.held_deletions, 12);

        handle_note_list_key(&mut app, key(KeyCode::Esc)).unwrap();
        assert_eq!(app.held_deletions, 0);
    }

    /// Ctrl+C used to recolour the note, Ctrl+P pin it, Ctrl+S open Settings and Ctrl+R
    /// open the bin, because the letter arms didn't look at modifiers.
    #[test]
    fn a_held_modifier_makes_a_key_something_other_than_its_letter() {
        for modifier in [KeyModifiers::CONTROL, KeyModifiers::ALT] {
            for letter in ['p', 's', 'r', 'n', 'z', 'L', 'C', 'y', '/', 't', 'v'] {
                let mut app = create_test_app();
                app.notes.push(Note::new("Note".to_string()));
                app.invalidate_filter_cache();

                handle_note_list_key(&mut app, KeyEvent::new(KeyCode::Char(letter), modifier)).unwrap();

                assert!(matches!(app.state, AppState::NoteList), "{letter} with {modifier:?}");
                assert!(matches!(app.view_mode, ViewMode::NoteList), "{letter} with {modifier:?}");
                assert!(!app.notes[0].pinned, "{letter} with {modifier:?}");
                assert!(!app.search_active, "{letter} with {modifier:?}");
            }
        }
    }

    #[test]
    fn the_ctrl_shortcuts_still_work() {
        let mut app = create_test_app();
        handle_note_list_key(&mut app, KeyEvent::new(KeyCode::Char('d'), KeyModifiers::CONTROL)).unwrap();
        assert_eq!(app.preview_scroll_offset, 10);

        let mut app = create_test_app();
        handle_note_list_key(&mut app, KeyEvent::new(KeyCode::Char('q'), KeyModifiers::CONTROL)).unwrap();
        assert!(matches!(app.state, AppState::Quit));
    }

    #[test]
    fn ctrl_c_quits_from_anywhere() {
        let ctrl_c = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);

        let mut app = create_test_app();
        app.handle_key(ctrl_c).unwrap();
        assert!(matches!(app.state, AppState::Quit));

        let mut app = create_test_app();
        app.state = AppState::Locked;
        let typed = uuid::Uuid::new_v4().to_string();
        app.password_input = typed.clone();
        app.handle_key(ctrl_c).unwrap();
        assert!(matches!(app.state, AppState::Quit));
        assert_eq!(app.password_input, typed);
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

    #[test]
    fn an_export_asks_before_replacing_a_file() {
        let mut app = app_with_notes(&["Selected"]);
        let id = app.notes[0].id.clone();
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("export.json");
        std::fs::write(&path, "already here").unwrap();

        let export = |app: &mut App| {
            app.is_multi_select_mode = true;
            app.selected_note_ids.insert(id.clone());
            app.input_mode = InputMode::BulkExportPath;
            app.bulk_export_path_input = path.to_str().unwrap().to_string();
            handle_note_list_key(app, key(KeyCode::Enter)).unwrap();
        };

        export(&mut app);
        assert!(app.export_overwrite_path.is_some());
        handle_note_list_key(&mut app, key(KeyCode::Char('n'))).unwrap();
        assert!(app.export_overwrite_path.is_none());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "already here");

        export(&mut app);
        handle_note_list_key(&mut app, key(KeyCode::Char('y'))).unwrap();
        assert!(std::fs::read_to_string(&path).unwrap().contains("Selected"));
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

    // ==================== Emptying the Bin ====================

    /// A connected-looking app in the bin, holding `count` deleted notes.
    fn app_in_the_bin(count: usize) -> App {
        let mut app = create_test_app();
        let db = crate::db::Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let note_key = crate::crypto::CryptoService::new().generate_master_key();

        for i in 0..count {
            let mut note = Note::new(format!("Deleted {i}"));
            note.deleted = true;
            NoteRepository::new(db.connection()).create(&note, &note_key).unwrap();
            app.notes.push(note);
        }

        app.db = Some(db);
        app.key = Some(note_key);
        app.view_mode = ViewMode::RecycleBin;
        app
    }

    fn notes_in_database(app: &App) -> i64 {
        app.db.as_ref().unwrap().count_notes(true).unwrap()
    }

    #[test]
    fn shift_e_asks_before_emptying_the_bin() {
        let mut app = app_in_the_bin(2);

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('E')));

        assert!(app.show_empty_bin_confirm);
        assert_eq!(notes_in_database(&app), 2);
    }

    #[test]
    fn confirming_empties_the_bin() {
        let mut app = app_in_the_bin(2);

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('E')));
        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('y')));

        assert!(!app.show_empty_bin_confirm);
        assert_eq!(notes_in_database(&app), 0);
    }

    #[test]
    fn declining_keeps_the_bin() {
        for answer in [KeyCode::Char('n'), KeyCode::Char('q'), KeyCode::Esc] {
            let mut app = app_in_the_bin(2);

            let _ = handle_note_list_key(&mut app, key(KeyCode::Char('E')));
            let _ = handle_note_list_key(&mut app, key(answer));

            assert!(!app.show_empty_bin_confirm);
            assert_eq!(notes_in_database(&app), 2);
        }
    }

    #[test]
    fn other_keys_do_not_answer_the_question() {
        let mut app = app_in_the_bin(1);

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('E')));
        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('E')));
        let _ = handle_note_list_key(&mut app, key(KeyCode::Enter));

        assert!(app.show_empty_bin_confirm);
        assert_eq!(notes_in_database(&app), 1);
    }

    #[test]
    fn an_empty_bin_does_not_ask() {
        let mut app = app_in_the_bin(0);

        let _ = handle_note_list_key(&mut app, key(KeyCode::Char('E')));

        assert!(!app.show_empty_bin_confirm);
    }
}
