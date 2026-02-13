//! Bulk operations for multi-note actions

use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

use crate::repository::NoteRepository;

use super::super::app::App;

/// Export format for notes
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportData {
    pub version: String,
    pub export_date: String,
    pub notes: Vec<ExportNote>,
}

/// Export format for a single note
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportNote {
    pub id: String,
    pub created_at: String,
    pub modified_at: String,
    pub content: String,
    pub tags: Vec<String>,
    pub pinned: bool,
    pub word_wrap: bool,
    pub syntax_language: Option<String>,
}

/// Add tags to all selected notes
pub fn add_tags_to_selected(app: &mut App, tags: &[String]) -> Result<usize> {
    if tags.is_empty() || app.selected_note_ids.is_empty() {
        return Ok(0);
    }

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let repo = NoteRepository::new(db.connection());

    let selected_ids: Vec<String> = app.selected_note_ids.iter().cloned().collect();
    let mut updated_count = 0;

    for note_id in &selected_ids {
        if let Some(note) = app.notes.iter_mut().find(|n| &n.id == note_id) {
            let original_count = note.tags.len();

            // Add tags that aren't already present
            for tag in tags {
                if !note.tags.iter().any(|t| t.eq_ignore_ascii_case(tag)) {
                    note.tags.push(tag.clone());
                }
            }

            if note.tags.len() != original_count {
                note.modified_at = Utc::now();
                note.version += 1;
                repo.update(note, key)?;
                updated_count += 1;
            }
        }
    }

    // Clear multi-selection
    app.clear_multi_selection();

    // Trigger sync to push tag changes to server
    if updated_count > 0 && app.settings.sync_enabled {
        app.trigger_sync();
    }

    Ok(updated_count)
}

/// Delete all selected notes (soft delete)
pub fn delete_selected(app: &mut App) -> Result<usize> {
    if app.selected_note_ids.is_empty() {
        return Ok(0);
    }

    let db = app.db.as_ref().context("Database not available")?;
    let repo = NoteRepository::new(db.connection());

    let selected_ids: Vec<String> = app.selected_note_ids.iter().cloned().collect();
    let mut deleted_count = 0;

    for note_id in &selected_ids {
        repo.delete(note_id)?;
        deleted_count += 1;
    }

    // Remove deleted notes from the list
    app.notes.retain(|n| !selected_ids.contains(&n.id));

    // Adjust selection
    let note_count = app.notes.len();
    if app.selected_note >= note_count && note_count > 0 {
        app.selected_note = note_count - 1;
    } else if note_count == 0 {
        app.selected_note = 0;
    }

    // Clear multi-selection
    app.clear_multi_selection();

    // Trigger sync to push deletions to server
    if deleted_count > 0 && app.settings.sync_enabled {
        app.trigger_sync();
    }

    Ok(deleted_count)
}

/// Export selected notes to a JSON file
pub fn export_selected(app: &App, path: &Path) -> Result<usize> {
    if app.selected_note_ids.is_empty() {
        return Ok(0);
    }

    let mut export_notes = Vec::new();

    for note in &app.notes {
        if app.selected_note_ids.contains(&note.id) {
            export_notes.push(ExportNote {
                id: note.id.clone(),
                created_at: note.created_at.to_rfc3339(),
                modified_at: note.modified_at.to_rfc3339(),
                content: note.content.clone(),
                tags: note.tags.clone(),
                pinned: note.pinned,
                word_wrap: note.word_wrap,
                syntax_language: Some(note.syntax_language.to_string()),
            });
        }
    }

    let export_data = ExportData {
        version: "1.0".to_string(),
        export_date: Utc::now().to_rfc3339(),
        notes: export_notes,
    };

    let count = export_data.notes.len();
    let json = serde_json::to_string_pretty(&export_data)?;
    fs::write(path, json)?;

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
/// - Original notes are soft-deleted
pub fn combine_selected(app: &mut App) -> Result<usize> {
    if app.selected_note_ids.len() < 2 {
        anyhow::bail!("Select at least 2 notes to combine");
    }

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let repo = NoteRepository::new(db.connection());

    let selected_ids: Vec<String> = app.selected_note_ids.iter().cloned().collect();

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

    // Combine all attachments
    let combined_attachments: Vec<crate::models::Attachment> = selected_notes
        .iter()
        .flat_map(|n| n.attachments.clone())
        .collect();

    // Create new combined note
    let mut new_note = crate::models::Note::new(combined_content);
    new_note.tags = combined_tags;
    new_note.attachments = combined_attachments;
    new_note.syntax_language = crate::models::SyntaxLanguage::Markdown;

    // Save new note to database
    repo.create(&new_note, key)?;

    // Soft-delete original notes
    for note_id in &selected_ids {
        repo.delete(note_id)?;
    }

    let count = selected_ids.len();

    // Remove deleted notes from the list
    app.notes.retain(|n| !selected_ids.contains(&n.id));

    // Insert new note at appropriate position (after pinned notes)
    let pinned_count = app.notes.iter().filter(|n| n.pinned).count();
    app.notes.insert(pinned_count, new_note);

    // Clear multi-selection and select the new note
    app.clear_multi_selection();
    app.selected_note = pinned_count;

    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::Note;

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
