//! Bulk operations for multi-note actions

use anyhow::{Context, Result};
use chrono::Utc;
use rust_i18n::t;
use std::path::Path;

use crate::{
    export::{export_note, ExportData},
    repository::{attachment::AttachmentRepository, NoteRepository},
};

use super::super::app::App;

/// Change every selected note that may be changed, returning how many really did.
/// `change` says whether it did anything to a note.
fn change_selected(app: &mut App, mut change: impl FnMut(&mut crate::models::Note) -> bool) -> Result<usize> {
    let ids = app.editable_selection();

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let repo = NoteRepository::new(db.connection());

    let mut changed = 0;
    for note in app.notes.iter_mut().filter(|note| ids.contains(&note.id)) {
        if change(note) {
            note.touch();
            repo.update(note, key)?;
            changed += 1;
        }
    }

    app.clear_multi_selection();
    app.invalidate_filter_cache();
    app.keep_selection_on_note();

    // Trigger sync to push the changes to the server
    if changed > 0 && app.settings.sync_enabled {
        app.trigger_sync();
    }

    Ok(changed)
}

/// Add tags to all selected notes
pub fn add_tags_to_selected(app: &mut App, tags: &[String]) -> Result<usize> {
    if tags.is_empty() {
        return Ok(0);
    }

    change_selected(app, |note| {
        let before = note.tags.len();
        for tag in tags {
            if !note.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                note.tags.push(tag.clone());
            }
        }
        note.tags.len() != before
    })
}

/// Remove tags from all selected notes
pub fn remove_tags_from_selected(app: &mut App, tags: &[String]) -> Result<usize> {
    if tags.is_empty() {
        return Ok(0);
    }

    change_selected(app, |note| {
        let before = note.tags.len();
        note.tags.retain(|existing| !tags.iter().any(|tag| tag.eq_ignore_ascii_case(existing)));
        note.tags.len() != before
    })
}

/// Set the colour of all selected notes, by its name in the palette. Nothing clears it.
pub fn set_colour_of_selected(app: &mut App, colour: &str) -> Result<usize> {
    let colour = (!colour.is_empty()).then(|| colour.to_string());

    if let Some(name) = &colour {
        if !app.settings.get_color_palette().contains_key(name) {
            anyhow::bail!("{}", t!("bulk.unknown_colour", colour = name));
        }
    }

    change_selected(app, |note| {
        let changed = note.color != colour;
        note.color = colour.clone();
        changed
    })
}

/// Delete all selected notes (soft delete)
pub fn delete_selected(app: &mut App) -> Result<usize> {
    let selected_ids = app.editable_selection();
    if selected_ids.is_empty() {
        return Ok(0);
    }

    let db = app.db.as_ref().context("Database not available")?;
    let repo = NoteRepository::new(db.connection());

    let transaction = db.connection().unchecked_transaction()?;
    for note_id in &selected_ids {
        repo.delete(note_id)?;
    }
    transaction.commit()?;

    // Remove deleted notes from the list
    app.notes.retain(|n| !selected_ids.contains(&n.id));
    app.clear_multi_selection();
    app.invalidate_filter_cache();
    app.keep_selection_on_note();

    // Trigger sync to push deletions to server
    if app.settings.sync_enabled {
        app.trigger_sync();
    }

    Ok(selected_ids.len())
}

/// Archive all selected notes, leaving locked ones where they are, and return how
/// many moved.
pub fn archive_selected(app: &mut App) -> Result<usize> {
    change_selected(app, |note| {
        if note.archived {
            return false;
        }
        note.archived = true;
        note.archived_at = Some(Utc::now());
        true
    })
}

/// Export selected notes to a JSON file
pub fn export_selected(app: &App, path: &Path, overwrite: bool) -> Result<usize> {
    if app.selected_note_ids.is_empty() {
        return Ok(0);
    }

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let attachments = AttachmentRepository::new(db.connection());

    let notes = app.visible_selection().into_iter()
        .map(|note| export_note(note, &attachments, key))
        .collect();

    let export_data = ExportData {
        version: "1.0".to_string(),
        export_date: Utc::now().to_rfc3339(),
        notes,
    };

    let count = export_data.notes.len();
    let json = serde_json::to_vec_pretty(&export_data)?;
    crate::export::write_private(path, &json, overwrite)?;

    Ok(count)
}

/// Build combined content from notes (used by combine_selected)
/// Notes are expected to be sorted by creation date (oldest first)
pub fn build_combined_content(notes: &[&crate::models::Note]) -> String {
    notes
        .iter()
        .map(|n| n.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n---\n\n")
}

/// Merge all unique tags from multiple notes
pub fn merge_tags(notes: &[&crate::models::Note]) -> Vec<String> {
    let mut combined_tags: std::collections::HashSet<String> = std::collections::HashSet::new();
    for note in notes {
        for tag in &note.tags {
            combined_tags.insert(tag.clone());
        }
    }
    combined_tags.into_iter().collect()
}

/// Combine all selected notes into a single note
/// - Content is joined with horizontal rule separator
/// - Notes are ordered by creation date (oldest first)
/// - Tags and attachments are merged from all notes
/// - Original notes are soft-deleted, in the same transaction, so a failure can't leave
///   the combined note alongside its originals
pub fn combine_selected(app: &mut App) -> Result<usize> {
    let selected_ids = app.editable_selection();
    if selected_ids.len() < 2 {
        anyhow::bail!("{}", t!("bulk.combine_requires_two"));
    }

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let repo = NoteRepository::new(db.connection());

    // Get selected notes sorted by creation date (oldest first)
    let mut selected_notes: Vec<&crate::models::Note> = app
        .notes
        .iter()
        .filter(|n| selected_ids.contains(&n.id))
        .collect();
    selected_notes.sort_by(|a, b| a.created_at.cmp(&b.created_at));

    // Combine content with horizontal rule separator
    let combined_content = build_combined_content(&selected_notes);

    // Merge all unique tags
    let combined_tags = merge_tags(&selected_notes);

    // Combine all attachments. Notes can share one.
    let mut combined_attachments: Vec<crate::models::Attachment> = Vec::new();
    for attachment in selected_notes.iter().flat_map(|n| n.attachments.iter()) {
        if !combined_attachments.iter().any(|known| known.id == attachment.id) {
            combined_attachments.push(attachment.clone());
        }
    }

    // Create new combined note
    let mut new_note = crate::models::Note::new(combined_content);
    new_note.tags = combined_tags;
    new_note.attachments = combined_attachments;
    new_note.syntax_language = crate::models::SyntaxLanguage::Markdown;

    let transaction = db.connection().unchecked_transaction()?;
    repo.create(&new_note, key)?;
    for note_id in &selected_ids {
        repo.delete(note_id)?;
    }
    transaction.commit()?;

    let count = selected_ids.len();
    let new_note_id = new_note.id.clone();

    // Remove deleted notes from the list
    app.notes.retain(|n| !selected_ids.contains(&n.id));
    app.notes.push(new_note);

    // Clear multi-selection and select the new note
    app.clear_multi_selection();
    app.selected_note_id = Some(new_note_id);
    app.invalidate_filter_cache();
    app.keep_selection_on_note();

    if app.settings.sync_enabled {
        app.trigger_sync();
    }

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Note;

    #[test]
    fn archiving_a_selection_skips_locked_notes() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        let key = crate::crypto::CryptoService::new().generate_master_key();
        let db = crate::db::Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();

        let open = Note::new("Open".to_string());
        let mut locked = Note::new("Locked".to_string());
        locked.locked = true;
        for note in [&open, &locked] {
            NoteRepository::new(db.connection()).create(note, &key).unwrap();
        }
        app.selected_note_ids.extend([open.id.clone(), locked.id.clone()]);
        app.notes = vec![open.clone(), locked.clone()];
        app.db = Some(db);
        app.key = Some(key);

        assert_eq!(archive_selected(&mut app).unwrap(), 1);

        let conn = app.db.as_ref().unwrap().connection();
        assert!(NoteRepository::new(conn).get(&open.id, &key).unwrap().unwrap().archived);
        assert!(!NoteRepository::new(conn).get(&locked.id, &key).unwrap().unwrap().archived);
    }

    /// A selection of these notes, the first of them locked.
    fn app_selecting(contents: &[&str]) -> (App, [u8; 32], Vec<String>) {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        let key = crate::crypto::CryptoService::new().generate_master_key();
        let db = crate::db::Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();

        let mut ids = Vec::new();
        for (index, content) in contents.iter().enumerate() {
            let mut note = Note::new(content.to_string());
            note.locked = index == 0;
            note.tags = vec!["keep".to_string(), "drop".to_string()];
            NoteRepository::new(db.connection()).create(&note, &key).unwrap();
            ids.push(note.id.clone());
            app.notes.push(note);
        }

        app.selected_note_ids.extend(ids.iter().cloned());
        app.is_multi_select_mode = true;
        app.db = Some(db);
        app.key = Some(key);
        (app, key, ids)
    }

    fn stored(app: &App, key: &[u8; 32], id: &str) -> Note {
        let conn = app.db.as_ref().unwrap().connection();
        NoteRepository::new(conn).get(id, key).unwrap().unwrap()
    }

    #[test]
    fn deleting_a_selection_skips_locked_notes() {
        let (mut app, key, ids) = app_selecting(&["locked", "open"]);

        assert_eq!(delete_selected(&mut app).unwrap(), 1);

        assert!(!stored(&app, &key, &ids[0]).deleted);
        assert!(stored(&app, &key, &ids[1]).deleted);
    }

    #[test]
    fn tagging_a_selection_skips_locked_notes() {
        let (mut app, key, ids) = app_selecting(&["locked", "open"]);

        add_tags_to_selected(&mut app, &["new".to_string()]).unwrap();

        assert!(!stored(&app, &key, &ids[0]).tags.contains(&"new".to_string()));
        assert!(stored(&app, &key, &ids[1]).tags.contains(&"new".to_string()));
    }

    /// The selection outlives a search, and an action shouldn't reach what's out of view.
    #[test]
    fn a_note_out_of_view_is_not_touched() {
        let (mut app, key, ids) = app_selecting(&["locked", "visible", "hidden"]);
        app.search_input = "visible".to_string();
        app.invalidate_filter_cache();

        assert_eq!(delete_selected(&mut app).unwrap(), 1);

        assert!(stored(&app, &key, &ids[1]).deleted);
        assert!(!stored(&app, &key, &ids[2]).deleted);
    }

    #[test]
    fn combining_skips_locked_notes_and_needs_two_that_can_change() {
        let (mut app, _key, _ids) = app_selecting(&["locked", "open"]);
        assert!(combine_selected(&mut app).is_err());

        let (mut app, key, ids) = app_selecting(&["locked", "one", "two"]);
        assert_eq!(combine_selected(&mut app).unwrap(), 2);

        assert!(!stored(&app, &key, &ids[0]).deleted);
        assert!(stored(&app, &key, &ids[1]).deleted);
        assert_eq!(app.filtered_notes()[app.selected_note].content, "one\n\n---\n\ntwo");
    }

    #[test]
    fn a_failed_combine_leaves_the_originals_as_they_were() {
        let (mut app, key, ids) = app_selecting(&["locked", "one", "two"]);
        let conn = app.db.as_ref().unwrap().connection();
        conn.execute_batch(&format!(
            "CREATE TRIGGER refuse_delete BEFORE UPDATE OF deleted ON notes
             WHEN NEW.id = '{}'
             BEGIN SELECT RAISE(ABORT, 'refused'); END;",
            ids[2],
        )).unwrap();

        assert!(combine_selected(&mut app).is_err());

        assert!(!stored(&app, &key, &ids[1]).deleted);
        let conn = app.db.as_ref().unwrap().connection();
        assert_eq!(NoteRepository::new(conn).count(false).unwrap(), 3);
    }

    #[test]
    fn tags_can_be_removed_from_a_selection() {
        let (mut app, key, ids) = app_selecting(&["locked", "one", "two"]);

        assert_eq!(remove_tags_from_selected(&mut app, &["DROP".to_string()]).unwrap(), 2);

        assert_eq!(stored(&app, &key, &ids[1]).tags, vec!["keep"]);
        assert_eq!(stored(&app, &key, &ids[0]).tags, vec!["keep", "drop"]);
    }

    #[test]
    fn a_selection_can_be_coloured_by_name_and_cleared() {
        let (mut app, key, ids) = app_selecting(&["locked", "one"]);
        let colour = app.settings.get_color_palette().keys().next().unwrap().clone();

        assert_eq!(set_colour_of_selected(&mut app, &colour).unwrap(), 1);
        assert_eq!(stored(&app, &key, &ids[1]).color, Some(colour));
        assert_eq!(stored(&app, &key, &ids[0]).color, None);

        app.selected_note_ids.insert(ids[1].clone());
        assert_eq!(set_colour_of_selected(&mut app, "").unwrap(), 1);
        assert_eq!(stored(&app, &key, &ids[1]).color, None);
    }

    #[test]
    fn a_colour_that_is_not_in_the_palette_is_refused() {
        let (mut app, _key, _ids) = app_selecting(&["locked", "one"]);

        assert!(set_colour_of_selected(&mut app, "no-such-colour").is_err());
        assert!(app.is_multi_select_mode);
    }

    #[test]
    fn test_build_combined_content_two_notes() {
        let note1 = Note::new("First note content".to_string());
        let note2 = Note::new("Second note content".to_string());
        let notes: Vec<&Note> = vec![&note1, &note2];

        let result = build_combined_content(&notes);

        assert_eq!(result, "First note content\n\n---\n\nSecond note content");
    }

    #[test]
    fn test_build_combined_content_three_notes() {
        let note1 = Note::new("AAA".to_string());
        let note2 = Note::new("BBB".to_string());
        let note3 = Note::new("CCC".to_string());
        let notes: Vec<&Note> = vec![&note1, &note2, &note3];

        let result = build_combined_content(&notes);

        assert_eq!(result, "AAA\n\n---\n\nBBB\n\n---\n\nCCC");
    }

    #[test]
    fn test_build_combined_content_preserves_multiline() {
        let note1 = Note::new("Line 1\nLine 2".to_string());
        let note2 = Note::new("Line 3\nLine 4".to_string());
        let notes: Vec<&Note> = vec![&note1, &note2];

        let result = build_combined_content(&notes);

        assert_eq!(result, "Line 1\nLine 2\n\n---\n\nLine 3\nLine 4");
    }

    #[test]
    fn test_build_combined_content_single_note() {
        let note1 = Note::new("Only note".to_string());
        let notes: Vec<&Note> = vec![&note1];

        let result = build_combined_content(&notes);

        assert_eq!(result, "Only note");
    }

    #[test]
    fn test_build_combined_content_empty() {
        let notes: Vec<&Note> = vec![];

        let result = build_combined_content(&notes);

        assert_eq!(result, "");
    }

    #[test]
    fn test_merge_tags_unique() {
        let mut note1 = Note::new("Note 1".to_string());
        note1.tags = vec!["tag-a".to_string(), "tag-b".to_string()];

        let mut note2 = Note::new("Note 2".to_string());
        note2.tags = vec!["tag-c".to_string(), "tag-d".to_string()];

        let notes: Vec<&Note> = vec![&note1, &note2];
        let result = merge_tags(&notes);

        assert_eq!(result.len(), 4);
        assert!(result.contains(&"tag-a".to_string()));
        assert!(result.contains(&"tag-b".to_string()));
        assert!(result.contains(&"tag-c".to_string()));
        assert!(result.contains(&"tag-d".to_string()));
    }

    #[test]
    fn test_merge_tags_duplicates() {
        let mut note1 = Note::new("Note 1".to_string());
        note1.tags = vec!["common".to_string(), "unique-a".to_string()];

        let mut note2 = Note::new("Note 2".to_string());
        note2.tags = vec!["common".to_string(), "unique-b".to_string()];

        let notes: Vec<&Note> = vec![&note1, &note2];
        let result = merge_tags(&notes);

        // Should have 3 unique tags, not 4
        assert_eq!(result.len(), 3);
        assert!(result.contains(&"common".to_string()));
        assert!(result.contains(&"unique-a".to_string()));
        assert!(result.contains(&"unique-b".to_string()));
    }

    #[test]
    fn test_merge_tags_empty_notes() {
        let note1 = Note::new("Note 1".to_string());
        let note2 = Note::new("Note 2".to_string());
        let notes: Vec<&Note> = vec![&note1, &note2];

        let result = merge_tags(&notes);

        assert!(result.is_empty());
    }

    #[test]
    fn test_merge_tags_one_has_tags() {
        let mut note1 = Note::new("Note 1".to_string());
        note1.tags = vec!["tag-a".to_string()];

        let note2 = Note::new("Note 2".to_string());

        let notes: Vec<&Note> = vec![&note1, &note2];
        let result = merge_tags(&notes);

        assert_eq!(result.len(), 1);
        assert!(result.contains(&"tag-a".to_string()));
    }
}
