//! Note management operations (CRUD, versions, trash)

use anyhow::{Context, Result};

use crate::{
    models::Note,
    repository::{NoteRepository, NoteVersionRepository, VersionReason},
};

use super::super::app::App;

/// Load notes from database
pub fn load_notes(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());
        app.notes = repo.list(false, key)?;
        app.selected_note = 0;
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
        }
    }
    Ok(())
}

/// Filter notes based on search query and sort (pinned first, then by modified date)
pub fn filtered_notes(app: &App) -> Vec<&Note> {
    let mut notes: Vec<&Note> = if app.search_input.is_empty() {
        app.notes.iter().collect()
    } else {
        let query = app.search_input.to_lowercase();
        let query_parts: Vec<&str> = query.split_whitespace().collect();

        app.notes
            .iter()
            .filter(|note| {
                let content_lower = note.content.to_lowercase();

                // Check each query part
                for part in &query_parts {
                    if let Some(tag) = part.strip_prefix('#') {
                        // Tag search
                        if !note.tags.iter().any(|t| t.to_lowercase().contains(tag)) {
                            return false;
                        }
                    } else if let Some(neg_word) = part.strip_prefix('-') {
                        // Negation
                        if content_lower.contains(neg_word) {
                            return false;
                        }
                    } else {
                        // Regular text search
                        if !content_lower.contains(part) {
                            return false;
                        }
                    }
                }

                true
            })
            .collect()
    };

    // Sort: pinned first, then by modified_at descending
    notes.sort_by(|a, b| {
        match (a.pinned, b.pinned) {
            (true, false) => std::cmp::Ordering::Less,
            (false, true) => std::cmp::Ordering::Greater,
            _ => b.modified_at.cmp(&a.modified_at),
        }
    });

    notes
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
        }
    }
    Ok(())
}

/// Load deleted notes for recycle bin view
pub fn load_deleted_notes(app: &mut App) -> Result<()> {
    if let (Some(db), Some(key)) = (&app.db, &app.key) {
        let repo = NoteRepository::new(db.connection());
        app.notes = repo.get_deleted(key)?;
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

        // Set success message
        app.sync_status = Some(format!("Permanently deleted {} note{}", count, if count == 1 { "" } else { "s" }));
    }
    Ok(())
}
