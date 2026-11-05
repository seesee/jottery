//! Note management operations (CRUD, versions, trash)

use anyhow::{Context, Result};
use rust_i18n::t;

use crate::{
    models::Note,
    repository::{NoteRepository, NoteVersionRepository, VersionReason},
};

use super::super::app::App;
use super::super::state::{AppState, InputMode};

/// Load notes from database, preserving current selection via persisted ID
pub fn load_notes(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());
        app.notes = repo.list(false, key)?;

        // Restore selection to the same note if it still exists (using persisted ID)
        // NOTE: We need to find position in the sorted view (pinned first, then by modified_at)
        // because selected_note is an index into filtered_notes(), not app.notes
        if let Some(note_id) = &app.selected_note_id.clone() {
            // Build sorted view like filtered_notes() does
            let mut sorted_notes: Vec<&Note> = app.notes.iter().collect();
            App::sort_notes_by_pinned_and_date(&mut sorted_notes);

            if let Some(index) = sorted_notes.iter().position(|n| &n.id == note_id) {
                app.selected_note = index;
            } else {
                // Note was deleted, select first note and update ID
                app.selected_note = 0;
                app.selected_note_id = sorted_notes.first().map(|n| n.id.clone());
            }
        } else {
            app.selected_note = 0;
            // Get first note from sorted view
            let mut sorted_notes: Vec<&Note> = app.notes.iter().collect();
            App::sort_notes_by_pinned_and_date(&mut sorted_notes);
            app.selected_note_id = sorted_notes.first().map(|n| n.id.clone());
        }

        // Refresh caches
        app.refresh_all_tags_cache();
        app.invalidate_filter_cache();
    }
    Ok(())
}

/// Save current note
pub fn save_note(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());

        if !app.note_input.is_empty() {
            if let Some(note_id) = &app.editing_note_id {
                // Update existing note
                if let Some(note) = app.notes.iter_mut().find(|n| &n.id == note_id) {
                    // Before editing, create a version snapshot if one doesn't exist
                    // This ensures we can revert to the pre-edit state
                    let version_repo = NoteVersionRepository::new(db.connection());
                    let version_key = format!("{}:{}", note.id, note.version);
                    if version_repo.get_version_by_key(&version_key)?.is_none() {
                        // No snapshot exists for current version, create one
                        version_repo.create_version(
                            note,
                            note.synced_at.unwrap_or_else(chrono::Utc::now),
                            VersionReason::ManualSync,
                            key,
                        )?;
                    }

                    note.content = app.note_input.clone();
                    note.tags = app.current_tags.clone();
                    note.touch();
                    repo.update(note, key)?;
                }
            } else {
                // Create new note
                let mut note = Note::new(app.note_input.clone());
                note.tags = app.current_tags.clone();
                repo.create(&note, key)?;
                app.notes.insert(0, note);
            }
            // Invalidate filter cache since notes changed
            app.invalidate_filter_cache();

            // Trigger sync to push changes to server
            if app.settings.sync_enabled {
                app.trigger_sync();
            }
        }
    }
    Ok(())
}

/// Open the note set up in `note_input` in $EDITOR, then save what comes back.
///
/// A failure leaves a message in `app.error`. If the editor ran but the save failed,
/// the edit stays in `note_input` and the note view opens on it, so the text can be
/// saved again rather than being lost when the list reloads.
pub fn edit_and_save(app: &mut App) {
    match super::attachments::edit_with_external_editor(app) {
        Ok(content) => save_edit(app, content),
        Err(e) => {
            app.error = Some(format!("{e:#}"));
            app.editing_note_id = None;
        }
    }
}

/// Save text that came back from the editor.
fn save_edit(app: &mut App, new_content: String) {
    app.note_input = new_content;
    if let Err(e) = save_note(app) {
        app.error = Some(t!("note.save_failed", error = e.to_string()).to_string());
        app.state = AppState::NoteView;
        app.input_mode = InputMode::Normal;
        return;
    }

    if let Err(e) = load_notes(app) {
        app.error = Some(t!("note.reload_failed", error = e.to_string()).to_string());
    }
    app.editing_note_id = None;
}

/// Load version history for a note
pub fn load_versions_for_note(app: &mut App, note_id: &str) -> Result<()> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    let version_repo = NoteVersionRepository::new(db.connection());
    app.loaded_versions = version_repo.get_versions_for_note(note_id, key)?;
    app.versions_note_id = Some(note_id.to_string());
    app.selected_version = 0;

    Ok(())
}

/// Restore a specific version of a note
pub fn restore_version(app: &mut App, version_number: i32) -> Result<()> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    // Get the note ID from loaded versions context
    let note_id = app.versions_note_id.as_ref()
        .context("No note context for version restore")?
        .clone();

    // Get the version to restore
    let version_repo = NoteVersionRepository::new(db.connection());
    let version = version_repo.get_version(&note_id, version_number, key)?
        .context(format!("Version {} not found", version_number))?;

    // Find the current note in memory
    let note_in_list = app.notes.iter_mut()
        .find(|n| n.id == note_id)
        .context("Note not found in memory")?;

    // Create a snapshot of the current state before restoring
    version_repo.create_version(
        note_in_list,
        chrono::Utc::now(),
        VersionReason::ManualSync,
        key,
    )?;

    // Restore the version data
    note_in_list.content = version.content;
    note_in_list.tags = version.tags;
    note_in_list.attachments = version.attachments;
    note_in_list.syntax_language = version.syntax_language.unwrap_or_default();
    note_in_list.word_wrap = version.word_wrap.unwrap_or(true);
    note_in_list.modified_at = chrono::Utc::now();
    note_in_list.version += 1;

    // Update in database
    let note_repo = NoteRepository::new(db.connection());
    note_repo.update(note_in_list, key)?;

    // Reload versions to show the new snapshot
    load_versions_for_note(app, &note_id)?;

    Ok(())
}

/// Delete a note (soft delete)
pub fn delete_note(app: &mut App) -> Result<()> {
    if let Some(db) = &app.db {
        if !app.notes.is_empty() && app.selected_note < app.notes.len() {
            let note = &app.notes[app.selected_note];
            let repo = NoteRepository::new(db.connection());
            repo.delete(&note.id)?;
            app.notes.remove(app.selected_note);
            if app.selected_note >= app.notes.len() && app.selected_note > 0 {
                app.selected_note -= 1;
            }
            app.invalidate_filter_cache();

            // Trigger sync to push deletion to server
            if app.settings.sync_enabled {
                app.trigger_sync();
            }
        }
    }
    Ok(())
}

/// Load deleted notes for recycle bin view
pub fn load_deleted_notes(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());
        app.notes = repo.get_deleted(key)?;
        app.invalidate_filter_cache();
    }
    Ok(())
}

/// Restore a deleted note
pub fn restore_note(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        if !app.notes.is_empty() && app.selected_note < app.notes.len() {
            let note_id = app.notes[app.selected_note].id.clone();

            // Restore the note by setting deleted = false
            if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
                note.restore();

                // Save to database
                let repo = NoteRepository::new(db.connection());
                repo.update(note, key)?;
            }

            // Reload deleted notes to refresh the list
            load_deleted_notes(app)?;

            // Adjust selection after restore
            if app.selected_note >= app.notes.len() && app.selected_note > 0 {
                app.selected_note -= 1;
            }

            // Trigger sync to push restore to server
            if app.settings.sync_enabled {
                app.trigger_sync();
            }
        }
    }
    Ok(())
}

/// Permanently delete all notes in recycle bin
pub fn empty_trash(app: &mut App) -> Result<()> {
    if let Some(db) = &app.db {
        let repo = NoteRepository::new(db.connection());
        let count = repo.empty_trash()?;

        // Clear the notes list
        app.notes.clear();
        app.selected_note = 0;
        app.invalidate_filter_cache();

        app.notify(t!("note.bin_emptied", count = count).to_string());

        // The deletions reach the server and other devices with the next sync
        if app.settings.sync_enabled {
            app.trigger_sync();
        }
    }
    Ok(())
}

/// View note content in read-only mode using best available pager
///
/// This is a convenience wrapper around [`super::pager::view_with_pager`].
pub fn view_note_readonly(app: &mut App, content: &str, syntax: crate::models::SyntaxLanguage) -> Result<()> {
    super::pager::view_with_pager(app, content, syntax)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::ui::input::note_view::handle_note_view_key;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    /// An unlocked app holding one note, whose saves can be made to fail.
    fn unlocked_app() -> App {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);

        let key = crate::crypto::CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let note = Note::new("Original".to_string());
        NoteRepository::new(db.connection()).create(&note, &key).unwrap();

        app.editing_note_id = Some(note.id.clone());
        app.notes.push(note);
        app.db = Some(db);
        app.key = Some(key);
        app
    }

    fn break_saving(app: &App) {
        app.db.as_ref().unwrap().connection().execute_batch("DROP TABLE notes").unwrap();
    }

    fn press(app: &mut App, code: KeyCode) {
        handle_note_view_key(app, KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
    }

    #[test]
    fn a_failed_save_keeps_the_edit_and_says_so() {
        let mut app = unlocked_app();
        break_saving(&app);

        save_edit(&mut app, "My edit".to_string());

        assert_eq!(app.note_input, "My edit");
        assert!(app.editing_note_id.is_some());
        assert!(matches!(app.state, AppState::NoteView));
        assert!(app.error.is_some());
    }

    #[test]
    fn a_successful_save_leaves_nothing_behind() {
        let mut app = unlocked_app();

        save_edit(&mut app, "My edit".to_string());

        assert_eq!(app.notes[0].content, "My edit");
        assert!(app.editing_note_id.is_none());
        assert!(app.error.is_none());
    }

    #[test]
    fn saving_from_the_note_view_stays_put_when_it_fails() {
        let mut app = unlocked_app();
        app.state = AppState::NoteView;
        app.note_input = "My edit".to_string();
        break_saving(&app);

        press(&mut app, KeyCode::Char('q'));

        assert!(matches!(app.state, AppState::NoteView));
        assert_eq!(app.note_input, "My edit");
        assert!(app.error.is_some());
    }

    #[test]
    fn escape_gives_up_on_an_edit_that_will_not_save() {
        let mut app = unlocked_app();
        app.state = AppState::NoteView;
        app.note_input = "My edit".to_string();
        break_saving(&app);

        press(&mut app, KeyCode::Char('q'));
        press(&mut app, KeyCode::Esc);

        assert!(matches!(app.state, AppState::NoteList));
        assert!(app.error.is_none());
    }
}
