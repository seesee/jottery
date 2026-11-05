//! Note management operations (CRUD, versions, trash)

use anyhow::{Context, Result};
use rust_i18n::t;

use crate::{
    models::Note,
    repository::{NoteRepository, NoteVersionRepository, VersionReason},
};

use super::super::app::App;
use super::super::state::{AppState, InputMode, ViewMode};

/// Load notes from database, preserving current selection via persisted ID
///
/// Archived notes come too. The list shows one group or the other, and sync needs
/// to know about both.
pub fn load_notes(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());
        let list = repo.list_not_deleted(key)?;
        app.notes = list.notes;
        app.notes_loaded();

        // A count that grew is news. One that stays the same is on the list's title.
        if list.unreadable > app.unreadable_notes {
            app.error = Some(t!("note.unreadable", count = list.unreadable).to_string());
        }
        app.unreadable_notes = list.unreadable;

        // Refresh caches
        app.refresh_all_tags_cache();
        app.invalidate_filter_cache();

        app.keep_selection_on_note();
    }
    Ok(())
}

/// Reload whichever list is showing: the bin holds different notes from the rest.
pub fn load_current_view(app: &mut App) -> Result<()> {
    if matches!(app.view_mode, ViewMode::RecycleBin) {
        load_deleted_notes(app)?;
        app.notes_loaded();
        app.keep_selection_on_note();
        return Ok(());
    }
    load_notes(app)
}

/// Save current note
///
/// An existing note saves whatever the buffer holds, empty included: clearing a note is
/// an edit like any other, and leaves a version behind. A new note with nothing in it
/// isn't a note.
pub fn save_note(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());

        if let Some(note_id) = &app.editing_note_id {
            // Update existing note
            let Some(note) = app.notes.iter_mut().find(|n| &n.id == note_id) else {
                return Ok(());
            };

            // Opening a note and closing it again isn't an edit. Saving it anyway
            // would date it now, and let it beat a newer edit from elsewhere.
            if note.content == app.note_input && note.tags == app.current_tags {
                return Ok(());
            }

            if note.locked {
                anyhow::bail!("{}", t!("note.locked_cannot_edit"));
            }

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
        } else if !app.note_input.is_empty() {
            // Create new note
            let mut note = Note::new(app.note_input.clone());
            note.tags = app.current_tags.clone();
            note.set_syntax(app.settings.default_note_type);
            repo.create(&note, key)?;
            app.notes.insert(0, note);
        } else {
            return Ok(());
        }

        // Invalidate filter cache since notes changed
        app.invalidate_filter_cache();

        // Trigger sync to push changes to server
        if app.settings.sync_enabled {
            app.trigger_sync();
        }
    }
    Ok(())
}

/// Go to the note a link points at. If the search hides it, the search goes.
pub fn follow_link(app: &mut App, target_note_id: String) {
    let position = |app: &App| app.filtered_notes().iter().position(|note| note.id == target_note_id);

    if position(app).is_none() {
        app.search_input.clear();
        app.invalidate_filter_cache();
        app.search_active = false;
    }

    if let Some(index) = position(app) {
        app.selected_note = index;
        app.selected_note_id = Some(target_note_id);
        app.preview_scroll_offset = 0;
        app.focused_panel = super::super::state::FocusedPanel::NoteContent;
    }
}

/// Move the keyboard to the next link on screen, coming back round to the first.
pub fn focus_next_link(app: &mut App) {
    let count = app.note_link_positions.len();
    if count == 0 {
        app.notify(t!("note.no_links").to_string());
        return;
    }

    app.selected_link = Some(app.selected_link.map_or(0, |index| (index + 1) % count));
}

/// Follow the link the keyboard is on, if it's on one.
pub fn follow_focused_link(app: &mut App) -> bool {
    let target = app.selected_link.take()
        .and_then(|index| app.note_link_positions.get(index))
        .map(|(note_id, ..)| note_id.clone());

    match target {
        Some(note_id) => {
            follow_link(app, note_id);
            true
        }
        None => false,
    }
}

/// What a terminal will reliably take through OSC 52, in bytes of text.
const MAX_CLIPBOARD_BYTES: usize = 100_000;

/// The escape sequence that asks the terminal to put `text` on the clipboard. It goes
/// through the terminal rather than a local clipboard, so it also works over SSH.
fn clipboard_sequence(text: &str) -> String {
    use base64::Engine as _;
    format!("\x1b]52;c;{}\x07", base64::engine::general_purpose::STANDARD.encode(text))
}

/// The note the list has highlighted.
fn highlighted_note(app: &App) -> Option<&Note> {
    app.filtered_notes().get(app.selected_note).copied()
}

/// Copy the highlighted note's text to the clipboard.
pub fn copy_selected(app: &mut App) -> Result<()> {
    use std::io::Write as _;

    let Some(note) = highlighted_note(app) else {
        return Ok(());
    };

    if note.content.len() > MAX_CLIPBOARD_BYTES {
        app.notify(t!("note.copy_too_large").to_string());
        return Ok(());
    }

    let sequence = clipboard_sequence(&note.content);
    let mut stdout = std::io::stdout();
    stdout.write_all(sequence.as_bytes())?;
    stdout.flush()?;

    app.notify(t!("note.copied").to_string());
    Ok(())
}

/// Turn word wrap on or off for the highlighted note.
pub fn toggle_word_wrap(app: &mut App) -> Result<()> {
    let Some(note_id) = highlighted_note(app).map(|note| note.id.clone()) else {
        return Ok(());
    };

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let note = app.notes.iter_mut()
        .find(|note| note.id == note_id)
        .context("Note not found in memory")?;

    if note.locked {
        anyhow::bail!("{}", t!("note.locked_cannot_edit"));
    }

    note.word_wrap = !note.word_wrap;
    let wrapped = note.word_wrap;
    NoteRepository::new(db.connection()).update(note, key)?;

    app.notify(if wrapped { t!("note.word_wrap_on") } else { t!("note.word_wrap_off") }.to_string());
    if app.settings.sync_enabled {
        app.trigger_sync();
    }
    Ok(())
}

/// Make a new note out of the highlighted one, with everything but its history. The two
/// share attachments, as notes can. A locked original gives an unlocked copy, which is
/// the way to change what it says.
pub fn duplicate_selected(app: &mut App) -> Result<()> {
    let Some(original) = highlighted_note(app).cloned() else {
        return Ok(());
    };

    let mut copy = Note::new(original.content.clone());
    copy.tags = original.tags.clone();
    copy.attachments = original.attachments.clone();
    copy.attachments_complete = true;
    copy.pinned = original.pinned;
    copy.word_wrap = original.word_wrap;
    copy.show_preview = original.show_preview;
    copy.syntax_language = original.syntax_language;
    copy.syntax_unknown = original.syntax_unknown.clone();
    copy.color = original.color.clone();

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    NoteRepository::new(db.connection()).create(&copy, key)?;

    app.selected_note_id = Some(copy.id.clone());
    app.notes.push(copy);
    app.invalidate_filter_cache();
    app.keep_selection_on_note();

    app.notify(t!("note.duplicated").to_string());
    if app.settings.sync_enabled {
        app.trigger_sync();
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
    // A version pulled by an older build can still hold encrypted filenames
    for attachment in &mut note_in_list.attachments {
        if let Ok(filename) = super::sync::filename_from_sync(&app.crypto, &attachment.filename, key) {
            attachment.filename = filename;
        }
    }
    // A version of a type this client doesn't know leaves the note's type as it is
    if let Some(language) = version.syntax_language {
        note_in_list.set_syntax(language);
    }
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

/// Move a note in or out of the archive.
pub fn set_archived(app: &mut App, note_id: &str, archived: bool) -> Result<()> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    let note = app.notes.iter_mut()
        .find(|n| n.id == note_id)
        .context("Note not found in memory")?;

    note.archived = archived;
    note.archived_at = archived.then(chrono::Utc::now);
    note.touch();
    NoteRepository::new(db.connection()).update(note, key)?;

    app.invalidate_filter_cache();
    app.keep_selection_on_note();

    // Trigger sync to push the change to the server
    if app.settings.sync_enabled {
        app.trigger_sync();
    }
    Ok(())
}

/// Archive the selected note, or bring it back if the archive is showing.
pub fn toggle_archive_selected(app: &mut App) {
    let filtered = app.filtered_notes();
    let Some(note) = filtered.get(app.selected_note) else {
        return;
    };
    let (note_id, locked, archived) = (note.id.clone(), note.locked, note.archived);

    if locked {
        app.notify(t!("note.locked_cannot_archive").to_string());
        return;
    }

    match set_archived(app, &note_id, !archived) {
        Ok(()) if archived => app.notify(t!("archive.unarchived").to_string()),
        Ok(()) => app.notify(t!("archive.archived").to_string()),
        Err(e) => app.error = Some(t!("archive.failed", error = e.to_string()).to_string()),
    }
}

/// Delete a note (soft delete). The highlight moves to the note that takes its place.
pub fn delete_note(app: &mut App, note_id: &str) -> Result<()> {
    if let Some(db) = &app.db {
        NoteRepository::new(db.connection()).delete(note_id)?;
        app.notes.retain(|note| note.id != note_id);
        app.invalidate_filter_cache();
        app.keep_selection_on_note();

        // Trigger sync to push deletion to server
        if app.settings.sync_enabled {
            app.trigger_sync();
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

/// Restore the note highlighted in the bin
pub fn restore_note(app: &mut App) -> Result<()> {
    // The bin is drawn from the filtered list, which isn't in the order of `notes`
    let Some(note_id) = app.filtered_notes().get(app.selected_note).map(|note| note.id.clone()) else {
        return Ok(());
    };

    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        // Restore the note by setting deleted = false
        if let Some(note) = app.notes.iter_mut().find(|n| n.id == note_id) {
            note.restore();

            // Save to database
            let repo = NoteRepository::new(db.connection());
            repo.update(note, key)?;
        }

        // Reload deleted notes to refresh the list
        load_deleted_notes(app)?;
        app.keep_selection_on_note();

        // Trigger sync to push restore to server
        if app.settings.sync_enabled {
            app.trigger_sync();
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
        app.select_note_at(0);
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

    fn stored_note(app: &App, id: &str) -> Note {
        let db = app.db.as_ref().unwrap();
        NoteRepository::new(db.connection()).get(id, app.key.as_ref().unwrap()).unwrap().unwrap()
    }

    #[test]
    fn archiving_keeps_the_note_and_moves_it_out_of_the_list() {
        let mut app = unlocked_app();
        let id = app.notes[0].id.clone();

        toggle_archive_selected(&mut app);

        assert!(app.filtered_notes().is_empty());
        let stored = stored_note(&app, &id);
        assert!(stored.archived);
        assert!(stored.archived_at.is_some());
    }

    #[test]
    fn archived_notes_show_in_archive_mode_after_a_reload() {
        let mut app = unlocked_app();
        toggle_archive_selected(&mut app);

        load_notes(&mut app).unwrap();
        assert!(app.filtered_notes().is_empty());

        app.archive_mode = true;
        app.invalidate_filter_cache();
        assert_eq!(app.filtered_notes().len(), 1);
    }

    #[test]
    fn a_note_comes_back_out_of_the_archive() {
        let mut app = unlocked_app();
        let id = app.notes[0].id.clone();
        toggle_archive_selected(&mut app);
        app.archive_mode = true;
        app.invalidate_filter_cache();

        toggle_archive_selected(&mut app);

        let stored = stored_note(&app, &id);
        assert!(!stored.archived);
        assert!(stored.archived_at.is_none());
    }

    /// Clearing a note used to look like success while the old text stayed and synced.
    #[test]
    fn clearing_a_note_saves_it_empty_and_keeps_a_version() {
        let mut app = unlocked_app();
        let id = app.notes[0].id.clone();
        let before = app.notes[0].content.clone();
        app.editing_note_id = Some(id.clone());
        app.current_tags = app.notes[0].tags.clone();
        app.note_input.clear();

        save_note(&mut app).unwrap();

        assert_eq!(stored_note(&app, &id).content, "");
        let db = app.db.as_ref().unwrap();
        let versions = NoteVersionRepository::new(db.connection())
            .get_versions_for_note(&id, app.key.as_ref().unwrap())
            .unwrap();
        assert!(versions.iter().any(|version| version.content == before));
    }

    #[test]
    fn a_new_note_with_nothing_in_it_is_not_a_note() {
        let mut app = unlocked_app();
        let count = app.notes.len();
        app.editing_note_id = None;
        app.note_input.clear();

        save_note(&mut app).unwrap();

        assert_eq!(app.notes.len(), count);
    }

    #[test]
    fn a_locked_note_refuses_a_save_and_keeps_its_text() {
        let mut app = unlocked_app();
        let id = app.notes[0].id.clone();
        let before = app.notes[0].content.clone();
        app.notes[0].locked = true;
        app.editing_note_id = Some(id.clone());
        app.current_tags = app.notes[0].tags.clone();
        app.note_input = "Changed through a side door".to_string();

        assert!(save_note(&mut app).is_err());

        assert_eq!(stored_note(&app, &id).content, before);
    }

    #[test]
    fn the_keyboard_can_reach_and_follow_a_link() {
        let mut app = unlocked_app();
        let target = Note::new("The target".to_string());
        app.notes.push(target.clone());
        app.invalidate_filter_cache();
        app.note_link_positions = vec![
            ("elsewhere".to_string(), 2, 1, 5),
            (target.id.clone(), 3, 1, 5),
        ];

        focus_next_link(&mut app);
        assert_eq!(app.selected_link, Some(0));
        focus_next_link(&mut app);
        assert_eq!(app.selected_link, Some(1));

        assert!(follow_focused_link(&mut app));
        assert_eq!(app.selected_note_id.as_deref(), Some(target.id.as_str()));
        assert_eq!(app.selected_link, None);

        // Round again, then nothing to follow once it's been cleared
        assert!(!follow_focused_link(&mut app));
    }

    #[test]
    fn the_clipboard_sequence_is_what_terminals_expect() {
        assert_eq!(clipboard_sequence("hi"), "\x1b]52;c;aGk=\x07");
    }

    #[test]
    fn a_duplicate_keeps_everything_but_its_identity() {
        let mut app = unlocked_app();
        let original_id = app.notes[0].id.clone();
        app.notes[0].tags = vec!["kept".to_string()];
        app.notes[0].set_syntax(crate::models::SyntaxLanguage::Python);
        app.notes[0].color = Some("red".to_string());
        app.notes[0].word_wrap = false;
        app.notes[0].locked = true;

        duplicate_selected(&mut app).unwrap();

        assert_eq!(app.notes.len(), 2);
        let copy = app.notes.iter().find(|note| note.id != original_id).unwrap().clone();
        assert_eq!(app.selected_note_id.as_deref(), Some(copy.id.as_str()));
        let stored = stored_note(&app, &copy.id);
        assert_eq!(stored.content, app.notes.iter().find(|n| n.id == original_id).unwrap().content);
        assert_eq!(stored.tags, ["kept"]);
        assert_eq!(stored.syntax_language, crate::models::SyntaxLanguage::Python);
        assert_eq!(stored.color.as_deref(), Some("red"));
        assert!(!stored.word_wrap);
        assert!(!stored.locked);
    }

    #[test]
    fn word_wrap_turns_off_and_on_again() {
        let mut app = unlocked_app();
        let id = app.notes[0].id.clone();
        assert!(app.notes[0].word_wrap);

        toggle_word_wrap(&mut app).unwrap();
        assert!(!stored_note(&app, &id).word_wrap);

        toggle_word_wrap(&mut app).unwrap();
        assert!(stored_note(&app, &id).word_wrap);
    }

    #[test]
    fn a_new_note_starts_as_the_default_type() {
        let mut app = unlocked_app();
        app.settings.default_note_type = crate::models::SyntaxLanguage::Python;
        app.editing_note_id = None;
        app.note_input = "print('hello')".to_string();
        app.current_tags.clear();

        save_note(&mut app).unwrap();

        assert_eq!(app.notes[0].syntax_language, crate::models::SyntaxLanguage::Python);
    }

    #[test]
    fn a_locked_note_stays_where_it_is() {
        let mut app = unlocked_app();
        app.notes[0].locked = true;
        let id = app.notes[0].id.clone();

        toggle_archive_selected(&mut app);

        assert!(!stored_note(&app, &id).archived);
    }

    #[test]
    fn archiving_a_note_changes_what_gets_pushed() {
        let mut app = unlocked_app();
        let before = app.notes[0].modified_at;
        let id = app.notes[0].id.clone();

        toggle_archive_selected(&mut app);

        assert!(stored_note(&app, &id).modified_at > before);
    }

    #[test]
    fn notes_that_cannot_be_read_are_mentioned() {
        let mut app = unlocked_app();
        let other_key = crate::crypto::CryptoService::new().generate_master_key();
        let db = app.db.as_ref().unwrap();
        NoteRepository::new(db.connection()).create(&Note::new("Elsewhere".to_string()), &other_key).unwrap();

        load_notes(&mut app).unwrap();

        assert_eq!(app.unreadable_notes, 1);
        assert!(app.error.is_some());
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
