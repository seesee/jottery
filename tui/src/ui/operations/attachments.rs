//! Attachment management operations

use anyhow::{Context, Result};
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear as TerminalClear, ClearType},
    cursor::MoveTo,
};
use rust_i18n::t;
use std::{
    env,
    io::{self, Write},
    path::PathBuf,
    process::Command,
};
use tempfile::NamedTempFile;

use crate::{
    models::Attachment,
    repository::{attachment::AttachmentRepository, NoteRepository},
};

use super::super::app::App;
use super::super::state::ViewMode;

/// Check if chafa is available for image preview (cached result)
pub fn is_chafa_available(app: &mut App) -> bool {
    if let Some(available) = app.chafa_available {
        return available;
    }

    // Try to run chafa --version
    let available = Command::new("chafa")
        .arg("--version")
        .output()
        .map(|output| output.status.success())
        .unwrap_or(false);

    app.chafa_available = Some(available);
    available
}

/// Detect MIME type from file extension
pub fn detect_mime_type(path: &std::path::Path) -> String {
    if let Some(ext) = path.extension() {
        match ext.to_str().unwrap_or("").to_lowercase().as_str() {
            // Images
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            "gif" => "image/gif",
            "webp" => "image/webp",
            "svg" => "image/svg+xml",
            "bmp" => "image/bmp",
            // Documents
            "pdf" => "application/pdf",
            "txt" => "text/plain",
            "md" => "text/markdown",
            "json" => "application/json",
            "xml" => "application/xml",
            // Archives
            "zip" => "application/zip",
            "tar" => "application/x-tar",
            "gz" => "application/gzip",
            // Default
            _ => "application/octet-stream",
        }
    } else {
        "application/octet-stream"
    }
    .to_string()
}

/// Expand ~ to home directory in file path
pub fn expand_tilde(path: &str) -> Result<PathBuf> {
    if path.starts_with("~/") {
        let home = env::var("HOME")
            .context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(&path[2..]))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// View an attachment (decrypt to temp file, then view with appropriate tool)
pub fn view_attachment(app: &mut App, attachment: &Attachment) -> Result<()> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    // Retrieve and decrypt attachment data
    let attachment_repo = AttachmentRepository::new(db.connection());
    let (filename, mime_type, _size, data) = attachment_repo
        .get(&attachment.id, key)?
        .context("Attachment not found in database")?;

    // Create temporary file with original extension
    let extension = std::path::Path::new(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");

    let temp_file = tempfile::Builder::new()
        .suffix(&format!(".{}", extension))
        .tempfile()
        .context("Failed to create temporary file")?;

    // Write decrypted data to temp file
    std::fs::write(temp_file.path(), &data)
        .context("Failed to write attachment to temporary file")?;

    // Determine viewer based on MIME type
    let is_image = mime_type.starts_with("image/");

    if is_image && is_chafa_available(app) {
        // Use chafa for terminal image display
        view_with_chafa(app, temp_file.path())?;
    } else {
        // Use system default opener
        view_with_system_default(app, temp_file.path())?;
    }

    Ok(())
}

/// View image with chafa in terminal
pub fn view_with_chafa(app: &mut App, path: &std::path::Path) -> Result<()> {
    // Suspend TUI
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;

    // Launch chafa
    let status = Command::new("chafa")
        .arg("--size=80x40") // Reasonable terminal size
        .arg("--animate=false") // No animation for static images
        .arg(path)
        .status()
        .context("Failed to launch chafa")?;

    if status.success() {
        // Wait for user to press key to continue
        println!("\nPress Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    }

    // Resume TUI (same pattern as edit_with_external_editor)
    execute!(io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    enable_raw_mode().context("Failed to enable raw mode")?;
    execute!(
        io::stdout(),
        TerminalClear(ClearType::All),
        TerminalClear(ClearType::Purge),
        MoveTo(0, 0)
    )
    .context("Failed to clear screen")?;
    io::stdout().flush().context("Failed to flush stdout")?;
    app.need_redraw = true;

    if !status.success() {
        anyhow::bail!("Chafa exited with non-zero status");
    }

    Ok(())
}

/// View file with system default application
pub fn view_with_system_default(app: &mut App, path: &std::path::Path) -> Result<()> {
    // Suspend TUI
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;

    // Determine command based on OS
    #[cfg(target_os = "macos")]
    let open_cmd = "open";
    #[cfg(target_os = "linux")]
    let open_cmd = "xdg-open";
    #[cfg(target_os = "windows")]
    let open_cmd = "start";
    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    let open_cmd = "xdg-open"; // Fallback

    // Launch viewer (detached process)
    let status = Command::new(open_cmd)
        .arg(path)
        .status()
        .context(format!("Failed to launch viewer: {}", open_cmd))?;

    if status.success() {
        // Wait for user acknowledgment (file opened in background)
        println!("File opened in default application.");
        println!("Press Enter to continue...");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
    }

    // Resume TUI
    execute!(io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    enable_raw_mode().context("Failed to enable raw mode")?;
    execute!(
        io::stdout(),
        TerminalClear(ClearType::All),
        TerminalClear(ClearType::Purge),
        MoveTo(0, 0)
    )
    .context("Failed to clear screen")?;
    io::stdout().flush().context("Failed to flush stdout")?;
    app.need_redraw = true;

    Ok(())
}

/// Delete the currently selected attachment
pub fn delete_current_attachment(app: &mut App) -> Result<()> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    // Get the note ID and attachment info first (without holding a borrow)
    let (note_id, attachment_id, filename) = {
        let filtered = app.filtered_notes();
        if filtered.is_empty() || app.selected_note >= filtered.len() {
            anyhow::bail!("{}", t!("note.no_notes"));
        }

        let note = filtered[app.selected_note];
        if app.selected_attachment >= note.attachments.len() {
            anyhow::bail!("No attachment selected");
        }

        let attachment_id = note.attachments[app.selected_attachment].id.clone();
        let filename = note.attachments[app.selected_attachment].filename.clone();
        (note.id.clone(), attachment_id, filename)
    };

    // Now we can mutate app.notes
    if let Some(note_in_list) = app.notes.iter_mut().find(|n| n.id == note_id) {
        // Remove attachment from note
        note_in_list.attachments.retain(|att| att.id != attachment_id);

        // Update note in database
        note_in_list.modified_at = chrono::Utc::now();
        note_in_list.version += 1;

        let note_repo = NoteRepository::new(db.connection());
        note_repo.update(note_in_list, key)?;

        // Delete attachment data from database
        let attachment_repo = AttachmentRepository::new(db.connection());
        attachment_repo.delete(&attachment_id)?;

        // Adjust selection
        if note_in_list.attachments.is_empty() {
            // No more attachments, close viewer
            app.view_mode = ViewMode::NoteList;
        } else if app.selected_attachment >= note_in_list.attachments.len() {
            app.selected_attachment = note_in_list.attachments.len() - 1;
        }

        app.error = Some(format!("Deleted: {}", filename));
    }

    Ok(())
}

/// Add attachment to the current note
pub fn add_attachment_to_current_note(app: &mut App, file_path: &str) -> Result<()> {
    // Clone debug_log so we can use it in the closure without borrowing app
    let debug_log = app.debug_log.clone();
    // Helper to write debug messages
    let log_debug = |msg: &str| {
        if let Some(log) = &debug_log {
            if let Ok(mut file) = log.lock() {
                let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
            }
        }
    };

    log_debug(&format!("add_attachment: Starting with path: {}", file_path));

    // Expand tilde in path
    let expanded_path = expand_tilde(file_path)?;
    log_debug(&format!("add_attachment: Expanded path: {:?}", expanded_path));

    // Validate file exists
    if !expanded_path.exists() {
        log_debug("add_attachment: File not found");
        anyhow::bail!("File not found: {}", file_path);
    }

    if !expanded_path.is_file() {
        anyhow::bail!("Path is not a file: {}", file_path);
    }

    // Check file size (warn if > 10MB, reject if > 50MB)
    let metadata = std::fs::metadata(&expanded_path)
        .context("Failed to read file metadata")?;
    let size_bytes = metadata.len();

    if size_bytes > 50 * 1024 * 1024 {
        anyhow::bail!("File too large (>50MB): {} bytes", size_bytes);
    }

    // Read file data
    let data = std::fs::read(&expanded_path)
        .context("Failed to read file")?;

    // Detect MIME type
    let mime_type = detect_mime_type(&expanded_path);

    // Get filename
    let filename = expanded_path
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unnamed")
        .to_string();

    // Get current note
    let filtered = app.filtered_notes();
    log_debug(&format!("add_attachment: filtered_notes count: {}", filtered.len()));
    let note_id = if !filtered.is_empty() && app.selected_note < filtered.len() {
        let id = filtered[app.selected_note].id.clone();
        log_debug(&format!("add_attachment: Selected note ID: {}", id));
        id
    } else {
        log_debug("add_attachment: No note selected");
        anyhow::bail!("{}", t!("note.no_notes"));
    };

    // Store encrypted attachment
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    // Generate UUID for attachment
    let attachment_id = uuid::Uuid::new_v4().to_string();
    log_debug(&format!("add_attachment: Generated attachment ID: {}", attachment_id));

    let attachment_repo = AttachmentRepository::new(db.connection());
    attachment_repo.store(&attachment_id, &filename, &mime_type, size_bytes as i64, &data, key)?;
    log_debug("add_attachment: Stored in attachments table");

    // Create Attachment struct
    let attachment = Attachment {
        id: attachment_id.clone(),
        filename: filename.clone(),
        mime_type: mime_type.clone(),
        size: size_bytes as i64,
        data: attachment_id, // Reference to blob store
        thumbnail_data: None,
    };

    // Load the note, add attachment, and save
    let note_repo = NoteRepository::new(db.connection());
    let mut note = note_repo
        .get(&note_id, key)?
        .context("Note not found")?;

    log_debug(&format!("add_attachment: Note has {} attachments before adding", note.attachments.len()));
    note.attachments.push(attachment);
    note.modified_at = chrono::Utc::now();
    note.version += 1;

    note_repo.update(&note, key)?;
    log_debug(&format!("add_attachment: Updated note, now has {} attachments", note.attachments.len()));

    // Refresh the note list to show updated note
    super::notes::load_notes(app)?;
    log_debug("add_attachment: Reloaded notes list");

    Ok(())
}

/// Remove attachment from the current note
pub fn remove_attachment_from_current_note(app: &mut App) -> Result<()> {
    // Get current note
    let filtered = app.filtered_notes();
    let note_id = if !filtered.is_empty() && app.selected_note < filtered.len() {
        filtered[app.selected_note].id.clone()
    } else {
        anyhow::bail!("{}", t!("note.no_notes"));
    };

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    // Load the note
    let note_repo = NoteRepository::new(db.connection());
    let mut note = note_repo
        .get(&note_id, key)?
        .context("Note not found")?;

    // Check if there are attachments
    if note.attachments.is_empty() {
        anyhow::bail!("{}", t!("attachment.no_attachments"));
    }

    // Validate selected_attachment index
    if app.selected_attachment >= note.attachments.len() {
        anyhow::bail!("Invalid attachment selection");
    }

    // Get attachment to remove
    let attachment = note.attachments.remove(app.selected_attachment);

    // Delete from database
    let attachment_repo = AttachmentRepository::new(db.connection());
    attachment_repo.delete(&attachment.id)?;

    // Update note
    note.modified_at = chrono::Utc::now();
    note.version += 1;

    note_repo.update(&note, key)?;

    // Adjust selected_attachment if needed
    if app.selected_attachment >= note.attachments.len() && !note.attachments.is_empty() {
        app.selected_attachment = note.attachments.len() - 1;
    } else if note.attachments.is_empty() {
        app.selected_attachment = 0;
    }

    // Refresh the note list to show updated note
    super::notes::load_notes(app)?;

    Ok(())
}

/// Edit note content with external $EDITOR
pub fn edit_with_external_editor(app: &mut App) -> Result<String> {
    // Create temporary file with current note content
    let mut temp_file = NamedTempFile::new()
        .context("Failed to create temporary file")?;
    temp_file
        .write_all(app.note_input.as_bytes())
        .context("Failed to write to temporary file")?;
    temp_file.flush()?;

    let temp_path = temp_file.path();

    // Suspend TUI
    disable_raw_mode().context("Failed to disable raw mode")?;
    execute!(io::stdout(), LeaveAlternateScreen)
        .context("Failed to leave alternate screen")?;

    // Get editor from environment (default to vi)
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // Launch editor
    let status = Command::new(&editor)
        .arg(temp_path)
        .status()
        .context(format!("Failed to launch editor: {}", editor))?;

    // Resume TUI
    execute!(io::stdout(), EnterAlternateScreen)
        .context("Failed to enter alternate screen")?;
    enable_raw_mode().context("Failed to enable raw mode")?;

    // Clear screen with crossterm (this clears the visible screen)
    execute!(
        io::stdout(),
        TerminalClear(ClearType::All),
        TerminalClear(ClearType::Purge),
        MoveTo(0, 0)
    ).context("Failed to clear screen")?;
    io::stdout().flush().context("Failed to flush stdout")?;

    // Set flag to force ratatui buffer clear on next render
    app.need_redraw = true;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    // Read modified content
    let content = std::fs::read_to_string(temp_path)
        .context("Failed to read modified content")?;

    Ok(content)
}
