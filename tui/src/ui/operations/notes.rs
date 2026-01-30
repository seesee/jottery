//! Note management operations (CRUD, versions, trash)

use anyhow::{Context, Result};

use crate::{
    models::Note,
    repository::{NoteRepository, NoteVersionRepository, VersionReason},
};

use super::super::app::App;

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
            sorted_notes.sort_by(|a, b| {
                match (a.pinned, b.pinned) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.modified_at.cmp(&a.modified_at),
                }
            });

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
            sorted_notes.sort_by(|a, b| {
                match (a.pinned, b.pinned) {
                    (true, false) => std::cmp::Ordering::Less,
                    (false, true) => std::cmp::Ordering::Greater,
                    _ => b.modified_at.cmp(&a.modified_at),
                }
            });
            app.selected_note_id = sorted_notes.first().map(|n| n.id.clone());
        }
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

/// View note content in read-only mode using best available pager
///
/// Pager preference order:
/// 1. $PAGER environment variable (if set)
/// 2. bat (syntax highlighting pager)
/// 3. less
/// 4. more (fallback)
pub fn view_note_readonly(app: &mut App, content: &str, syntax: crate::models::SyntaxLanguage) -> Result<()> {
    use std::io::Write;
    use crossterm::{
        execute,
        terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
                   Clear as TerminalClear, ClearType},
        cursor::MoveTo,
    };

    // Map syntax language to file extension for syntax highlighting
    let extension = match syntax {
        crate::models::SyntaxLanguage::Plain => "txt",
        crate::models::SyntaxLanguage::Markdown => "md",
        crate::models::SyntaxLanguage::Javascript => "js",
        crate::models::SyntaxLanguage::Python => "py",
        crate::models::SyntaxLanguage::Html => "html",
        crate::models::SyntaxLanguage::Css => "css",
        crate::models::SyntaxLanguage::Json => "json",
        crate::models::SyntaxLanguage::Sql => "sql",
        crate::models::SyntaxLanguage::Bash => "sh",
        crate::models::SyntaxLanguage::Perl => "pl",
        crate::models::SyntaxLanguage::Calc => "txt",
        crate::models::SyntaxLanguage::Outliner => "md", // Markdown-compatible format
    };

    // Create temp file with appropriate extension
    let mut temp_file = tempfile::Builder::new()
        .suffix(&format!(".{}", extension))
        .tempfile()
        .context("Failed to create temporary file")?;

    // Write content to temp file
    temp_file.write_all(content.as_bytes())
        .context("Failed to write to temporary file")?;
    temp_file.flush()?;

    let temp_path = temp_file.path().to_path_buf();

    // Suspend TUI
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(std::io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;

    // Determine pager to use
    let pager_result = if let Ok(pager) = std::env::var("PAGER") {
        // User-specified pager
        run_pager(&pager, &temp_path, None)
    } else if is_command_available("bat") {
        // bat with syntax highlighting (--paging=always forces pager mode)
        run_pager("bat", &temp_path, Some(&["--paging=always", "--style=plain"]))
    } else if is_command_available("less") {
        // less with raw control chars for any escape sequences
        run_pager("less", &temp_path, Some(&["-R"]))
    } else {
        // Fallback to more
        run_pager("more", &temp_path, None)
    };

    // Resume TUI
    execute!(std::io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    enable_raw_mode().context("Failed to enable raw mode")?;

    // Clear screen and force redraw
    execute!(
        std::io::stdout(),
        TerminalClear(ClearType::All),
        TerminalClear(ClearType::Purge),
        MoveTo(0, 0)
    )
    .context("Failed to clear screen")?;
    std::io::stdout().flush().context("Failed to flush stdout")?;
    app.need_redraw = true;

    pager_result
}

/// Check if a command is available in PATH
fn is_command_available(cmd: &str) -> bool {
    std::process::Command::new("which")
        .arg(cmd)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Run a pager with optional arguments
fn run_pager(pager: &str, path: &std::path::Path, args: Option<&[&str]>) -> Result<()> {
    let mut cmd = std::process::Command::new(pager);
    if let Some(args) = args {
        cmd.args(args);
    }
    cmd.arg(path);

    let status = cmd.status();

    match status {
        Ok(exit_status) if exit_status.success() => Ok(()),
        Ok(_) => {
            // Non-zero exit is usually OK for pagers (user might quit early)
            Ok(())
        }
        Err(e) => {
            anyhow::bail!("Failed to run pager '{}': {}", pager, e)
        }
    }
}

