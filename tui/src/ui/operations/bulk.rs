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

    Ok(updated_count)
}

/// Remove tags from all selected notes
#[allow(dead_code)]
pub fn remove_tags_from_selected(app: &mut App, tags: &[String]) -> Result<usize> {
    if tags.is_empty() || app.selected_note_ids.is_empty() {
        return Ok(0);
    }

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    let repo = NoteRepository::new(db.connection());

    let selected_ids: Vec<String> = app.selected_note_ids.iter().cloned().collect();
    let tags_lower: Vec<String> = tags.iter().map(|t| t.to_lowercase()).collect();
    let mut updated_count = 0;

    for note_id in &selected_ids {
        if let Some(note) = app.notes.iter_mut().find(|n| &n.id == note_id) {
            let original_count = note.tags.len();

            // Remove matching tags (case-insensitive)
            note.tags.retain(|t| !tags_lower.contains(&t.to_lowercase()));

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

/// Get all unique tags from selected notes (for remove tags dialog)
#[allow(dead_code)]
pub fn get_tags_from_selected(app: &App) -> Vec<String> {
    let mut tags = std::collections::HashSet::new();

    for note in &app.notes {
        if app.selected_note_ids.contains(&note.id) {
            for tag in &note.tags {
                tags.insert(tag.clone());
            }
        }
    }

    let mut tags: Vec<String> = tags.into_iter().collect();
    tags.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()));
    tags
}
