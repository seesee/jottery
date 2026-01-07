//! Input handling for the note list screen

use anyhow::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;

use crate::models::SyntaxLanguage;
use crate::repository::NoteRepository;
use crate::ui::app::App;
use crate::ui::operations;
use crate::ui::state::{AppState, InputMode, ViewMode};

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

    // Handle bulk add tags input mode
    if matches!(app.input_mode, InputMode::BulkAddTags) {
        match key.code {
            KeyCode::Enter => {
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
            KeyCode::Esc => {
                app.bulk_tags_input.clear();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                app.bulk_tags_input.pop();
            }
            KeyCode::Char(c) => {
                app.bulk_tags_input.push(c);
            }
            _ => {}
        }
        return Ok(());
    }

    // Handle bulk export path input mode
    if matches!(app.input_mode, InputMode::BulkExportPath) {
        match key.code {
            KeyCode::Enter => {
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
            KeyCode::Esc => {
                app.bulk_export_path_input.clear();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                app.bulk_export_path_input.pop();
            }
            KeyCode::Char(c) => {
                app.bulk_export_path_input.push(c);
            }
            _ => {}
        }
        return Ok(());
    }

    // Handle attachment path input mode
    if matches!(app.input_mode, InputMode::AttachmentPath) {
        match key.code {
            KeyCode::Enter => {
                // Add attachment from file path
                let path = app.attachment_path_input.clone();
                app.attachment_path_input.clear();
                app.input_mode = InputMode::Normal;

                if !path.is_empty() {
                    if let Err(e) = operations::attachments::add_attachment_to_current_note(app, &path) {
                        app.error = Some(t!("attachment.add_failed", error = e.to_string()).to_string());
                    }
                }
            }
            KeyCode::Esc => {
                // Cancel attachment input
                app.attachment_path_input.clear();
                app.input_mode = InputMode::Normal;
            }
            KeyCode::Backspace => {
                app.attachment_path_input.pop();
            }
            KeyCode::Char(c) => {
                app.attachment_path_input.push(c);
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
                app.selected_note = 0;
            }
            KeyCode::Enter => {
                // Exit search and edit selected note directly
                if !app.filtered_notes().is_empty() {
                    let filtered = app.filtered_notes();
                    if app.selected_note < filtered.len() {
                        // Clone the data we need before modifying self
                        let content = filtered[app.selected_note].content.clone();
                        let note_id = filtered[app.selected_note].id.clone();
                        let syntax_lang = filtered[app.selected_note].syntax_language;
                        let tags = filtered[app.selected_note].tags.clone();

                        // Set up for editing
                        app.note_input = content;
                        app.note_syntax = syntax_lang;
                        app.current_tags = tags;
                        app.editing_note_id = Some(note_id.clone());
                        app.search_input.clear();
                        app.search_active = false;

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
            KeyCode::Char(c) => {
                app.search_input.push(c);
                app.selected_note = 0; // Reset selection when search changes
            }
            KeyCode::Backspace => {
                app.search_input.pop();
                app.selected_note = 0;
            }
            KeyCode::Down => {
                let filtered_count = app.filtered_notes().len();
                if filtered_count > 0 && app.selected_note < filtered_count - 1 {
                    app.selected_note += 1;
                }
            }
            KeyCode::Up => {
                if app.selected_note > 0 {
                    app.selected_note -= 1;
                }
            }
            _ => {}
        }
    } else {
        // Normal note list mode
        match key.code {
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                app.state = AppState::Quit;
            }
            KeyCode::Char('?') => {
                // Show help
                let prev = std::mem::replace(&mut app.state, AppState::Quit);
                app.state = AppState::Help {
                    previous: Box::new(prev),
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
            KeyCode::Char('n') if app.show_force_sync_confirm || app.show_bulk_delete_confirm => {
                // Cancel confirmations
                app.show_force_sync_confirm = false;
                app.show_bulk_delete_confirm = false;
            }
            KeyCode::Char('/') => {
                // Enter search mode (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.search_active = true;
                    app.search_input.clear();
                }
            }
            KeyCode::Char('n') => {
                // New note - open editor immediately (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.note_input.clear();
                    app.note_syntax = SyntaxLanguage::default();
                    app.current_tags.clear();
                    app.editing_note_id = None;

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
                // In attachment viewer: view selected attachment
                else if matches!(app.view_mode, ViewMode::AttachmentViewer) {
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
                    let content = filtered[app.selected_note].content.clone();
                    let note_id = filtered[app.selected_note].id.clone();
                    let syntax_lang = filtered[app.selected_note].syntax_language;
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
            KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Navigate attachments down (Shift+j)
                let filtered = app.filtered_notes();
                if !filtered.is_empty() && app.selected_note < filtered.len() {
                    let note = filtered[app.selected_note];
                    if !note.attachments.is_empty() && app.selected_attachment < note.attachments.len() - 1 {
                        app.selected_attachment += 1;
                    }
                }
            }
            KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                // Navigate attachments up (Shift+k)
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
                } else {
                    // Navigate notes
                    let note_count = app.filtered_notes().len();
                    if note_count > 0 && app.selected_note < note_count - 1 {
                        app.selected_note += 1;
                        app.preview_scroll_offset = 0;
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
                } else {
                    // Navigate notes
                    if app.selected_note > 0 {
                        app.selected_note -= 1;
                        app.preview_scroll_offset = 0;
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
                        // Update modified_at to match web client behavior (triggers sync)
                        note.modified_at = chrono::Utc::now();
                        // Increment version for optimistic locking
                        note.version += 1;

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
            KeyCode::Char('l') => {
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
            KeyCode::Char('L') => {
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
                    ViewMode::AttachmentViewer => {
                        // 'r' does nothing in attachment viewer
                    }
                    ViewMode::VersionHistory => {
                        // 'r' does nothing in version history
                    }
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
                // Cancel confirmations if showing
                if app.show_force_sync_confirm || app.show_bulk_delete_confirm {
                    app.show_force_sync_confirm = false;
                    app.show_bulk_delete_confirm = false;
                } else if app.is_multi_select_mode {
                    // Clear multi-selection
                    app.clear_multi_selection();
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
                // In multi-select mode: show bulk delete confirmation
                else if app.is_multi_select_mode && matches!(app.view_mode, ViewMode::NoteList) {
                    app.show_bulk_delete_confirm = true;
                }
                // Delete selected note (only in note list view)
                else if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
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
            KeyCode::Char('a') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Ctrl+A: Select all filtered notes (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    app.select_all_filtered();
                }
            }
            KeyCode::Char('a') => {
                // Open attachment viewer modal
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
            KeyCode::Char('A') => {
                // Enter attachment path input mode (only in note list view)
                if matches!(app.view_mode, ViewMode::NoteList) {
                    let filtered = app.filtered_notes();
                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                        app.input_mode = InputMode::AttachmentPath;
                        app.attachment_path_input.clear();
                    } else {
                        app.error = Some(t!("note.no_notes").to_string());
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
            _ => {
                // Reset 'a' key flag for any other key
                app.last_key_was_a = false;
            }
        }
    }
    Ok(())
}
