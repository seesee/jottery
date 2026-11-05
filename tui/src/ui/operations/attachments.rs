//! Attachment management operations

use anyhow::{Context, Result};
use rust_i18n::t;
use std::{
    env,
    io::Write,
    path::PathBuf,
    process::Command,
};

use crate::{
    models::Attachment,
    repository::{attachment::AttachmentRepository, NoteRepository},
};

use super::external;
use super::super::app::App;
use super::super::state::{InputMode, ViewMode};
use super::super::terminal::with_terminal_suspended;

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
    if let Some(rest) = path.strip_prefix("~/") {
        let home = env::var("HOME")
            .context("HOME environment variable not set")?;
        Ok(PathBuf::from(home).join(rest))
    } else {
        Ok(PathBuf::from(path))
    }
}

/// Get path completions for Tab completion
/// Returns a list of possible completions for the given partial path
pub fn get_path_completions(partial: &str) -> Vec<String> {
    if partial.is_empty() {
        // Show current directory contents
        if let Ok(entries) = std::fs::read_dir(".") {
            return entries
                .filter_map(|e| e.ok())
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    if e.path().is_dir() {
                        format!("{}/", name)
                    } else {
                        name
                    }
                })
                .take(20)
                .collect();
        }
        return Vec::new();
    }

    // Expand ~ for matching
    let expanded = if partial.starts_with("~/") {
        if let Ok(home) = env::var("HOME") {
            format!("{}{}", home, &partial[1..])
        } else {
            partial.to_string()
        }
    } else {
        partial.to_string()
    };

    let path = std::path::Path::new(&expanded);

    // Determine directory and prefix to match
    let (dir, prefix) = if path.is_dir() && partial.ends_with('/') {
        // Listing contents of a directory
        (path.to_path_buf(), String::new())
    } else if let Some(parent) = path.parent() {
        // Matching files in parent directory
        let prefix = path.file_name()
            .map(|f| f.to_string_lossy().to_string())
            .unwrap_or_default();
        (parent.to_path_buf(), prefix)
    } else {
        // Root or current directory
        (std::path::PathBuf::from("."), expanded.clone())
    };

    // Read directory and filter by prefix
    if let Ok(entries) = std::fs::read_dir(&dir) {
        let mut completions: Vec<String> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                prefix.is_empty() || name.starts_with(&prefix)
            })
            .map(|e| {
                let name = e.file_name().to_string_lossy().to_string();
                let is_dir = e.path().is_dir();

                // Build the full completion path
                let completion = if partial.starts_with("~/") {
                    // Preserve ~/ prefix
                    let home = env::var("HOME").unwrap_or_default();
                    let parent_str = dir.to_string_lossy();
                    let relative = parent_str.strip_prefix(&home).unwrap_or(&parent_str);
                    format!("~{}/{}{}", relative, name, if is_dir { "/" } else { "" })
                } else if dir.as_os_str() == "." {
                    format!("{}{}", name, if is_dir { "/" } else { "" })
                } else {
                    format!("{}/{}{}", dir.display(), name, if is_dir { "/" } else { "" })
                };
                completion
            })
            .take(20)
            .collect();

        completions.sort();
        completions
    } else {
        Vec::new()
    }
}

/// Largest file that can be attached, matching the web app.
const MAX_ATTACHMENT_BYTES: u64 = 10 * 1024 * 1024;

/// The file types that may be opened, with the suffix their temporary file gets.
///
/// A filename belongs to whoever created the attachment, and a server can change the
/// MIME type, so neither decides what handles the bytes. These are types whose usual
/// handlers only display them: the suffix comes from this list, so a file claiming
/// to be text and named invoice.terminal opens as text, if it opens at all.
const INERT_TYPES: &[(&str, &str)] = &[
    ("image/png", "png"),
    ("image/jpeg", "jpg"),
    ("image/gif", "gif"),
    ("image/webp", "webp"),
    ("image/bmp", "bmp"),
    ("application/pdf", "pdf"),
    ("text/plain", "txt"),
];

/// The suffix to open a file with, or None if it mustn't be opened.
fn inert_suffix(mime_type: &str, filename: &str) -> Option<&'static str> {
    let mime_type = mime_type.split(';').next().unwrap_or("").trim().to_ascii_lowercase();

    if let Some((_, suffix)) = INERT_TYPES.iter().find(|(inert, _)| *inert == mime_type) {
        return Some(suffix);
    }

    // Some clients send no useful type, so a known extension can stand in for
    // one. The suffix still comes from the list, so nothing else passes through.
    if mime_type.is_empty() || mime_type == "application/octet-stream" {
        let detected = detect_mime_type(std::path::Path::new(filename));
        return INERT_TYPES.iter().find(|(inert, _)| *inert == detected).map(|(_, suffix)| *suffix);
    }

    None
}

/// View an attachment (decrypt to temp file, then view with appropriate tool)
///
/// Only inert file types are opened. For anything else this asks whether to save
/// the file instead, as opening it could run whatever the sender put in it.
pub fn view_attachment(app: &mut App, attachment: &Attachment) -> Result<()> {
    let Some(suffix) = inert_suffix(&attachment.mime_type, &attachment.filename) else {
        app.attachment_to_confirm = Some(attachment.clone());
        return Ok(());
    };

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    // Retrieve and decrypt attachment data
    let attachment_repo = AttachmentRepository::new(db.connection());
    let (_filename, _mime_type, _size, data) = attachment_repo
        .get(&attachment.id, key)?
        .context("Attachment not found in database")?;

    let temp_file = crate::secure_temp::create(&format!(".{}", suffix))?;

    // Write decrypted data to temp file
    std::fs::write(temp_file.path(), &data)
        .context("Failed to write attachment to temporary file")?;

    // Determine viewer based on the type it's being opened as
    let is_image = !matches!(suffix, "pdf" | "txt");

    if is_image && is_chafa_available(app) {
        // Use chafa for terminal image display
        view_with_chafa(app, temp_file.path())?;
    } else {
        // Use system default opener
        view_with_system_default(app, temp_file.path())?;
    }

    Ok(())
}

/// Make a filename safe to create on this machine. It came from another device,
/// so it could hold a path, or characters no file should have.
fn safe_filename(filename: &str) -> String {
    let name: String = filename
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or("")
        .chars()
        .filter(|c| !c.is_control())
        .collect();

    match name.trim() {
        "" | "." | ".." => "attachment".to_string(),
        name => name.to_string(),
    }
}

/// Where to suggest saving a file: the downloads folder, if this machine has one.
fn suggested_save_path(filename: &str) -> String {
    let name = safe_filename(filename);
    match dirs::download_dir() {
        Some(dir) => dir.join(name).to_string_lossy().into_owned(),
        None => name,
    }
}

/// Start asking where to save an attachment.
pub fn begin_save_attachment(app: &mut App, attachment: Attachment) {
    app.attachment_path_input = suggested_save_path(&attachment.filename);
    app.reset_path_completions();
    app.attachment_to_save = Some(attachment);
    app.input_mode = InputMode::AttachmentPath;
}

/// Start asking which file to attach to the selected note.
pub fn begin_add_attachment(app: &mut App) -> Result<()> {
    let filtered = app.filtered_notes();
    let note = filtered.get(app.selected_note)
        .ok_or_else(|| anyhow::anyhow!("{}", t!("note.no_notes")))?;

    if note.locked {
        anyhow::bail!("{}", t!("note.locked_cannot_edit"));
    }

    app.attachment_path_input.clear();
    app.reset_path_completions();
    app.attachment_to_save = None;
    app.input_mode = InputMode::AttachmentPath;
    Ok(())
}

/// The selected attachment, if the attachment list is what's in use.
pub fn selected_attachment(app: &App) -> Option<Attachment> {
    let filtered = app.filtered_notes();
    filtered.get(app.selected_note)?.attachments.get(app.selected_attachment).cloned()
}

/// Write an attachment to `path`, or into it if it's a directory.
///
/// An existing file is left alone: this is someone else's file name, so asking the
/// user to choose another is better than replacing what's there.
pub fn save_attachment(app: &mut App, attachment: &Attachment, path: &str) -> Result<PathBuf> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;

    let (_filename, _mime_type, _size, data) = AttachmentRepository::new(db.connection())
        .get(&attachment.id, key)?
        .context("Attachment not found in database")?;

    let mut target = expand_tilde(path)?;
    if target.is_dir() {
        target.push(safe_filename(&attachment.filename));
    }

    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let mut file = options.open(&target).map_err(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => {
            anyhow::anyhow!("{}", t!("attachment.exists", path = target.display().to_string()))
        }
        _ => anyhow::Error::new(e).context(target.display().to_string()),
    })?;
    file.write_all(&data).context("Failed to write attachment")?;

    Ok(target)
}

/// Wait for Enter before the TUI takes the screen back
fn wait_for_enter() -> Result<()> {
    println!("\n{}", t!("attachment.press_enter"));
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    Ok(())
}

/// View image with chafa in terminal
pub fn view_with_chafa(app: &mut App, path: &std::path::Path) -> Result<()> {
    with_terminal_suspended(app, || {
        // Let chafa auto-detect the best output format (symbols, sixel, kitty,
        // etc.) and scale to fit the terminal
        let status = Command::new("chafa")
            .arg("--animate=false")
            .arg(path)
            .status()
            .context("Failed to launch chafa")?;

        if !status.success() {
            anyhow::bail!("Chafa exited with non-zero status");
        }
        wait_for_enter()
    })
}

/// View file with system default application
pub fn view_with_system_default(app: &mut App, path: &std::path::Path) -> Result<()> {
    // Checked first, so a machine with no opener doesn't flash the screen
    let mut opener = external::system_opener(path)
        .ok_or_else(|| anyhow::anyhow!("{}", t!("attachment.no_opener")))?;

    with_terminal_suspended(app, || {
        let status = opener.status().context("Failed to launch viewer")?;

        if status.success() {
            println!("{}", t!("attachment.opened"));
            wait_for_enter()?;
        }
        Ok(())
    })
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
        if note.locked {
            anyhow::bail!("{}", t!("note.locked_cannot_edit"));
        }
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

        // Delete attachment data from database, unless a duplicate of this note shares it
        if !note_repo.attachment_in_use(&attachment_id)? {
            AttachmentRepository::new(db.connection()).delete(&attachment_id)?;
        }

        // Adjust selection
        if note_in_list.attachments.is_empty() {
            // No more attachments, close viewer
            app.view_mode = ViewMode::NoteList;
        } else if app.selected_attachment >= note_in_list.attachments.len() {
            app.selected_attachment = note_in_list.attachments.len() - 1;
        }

        app.notify(t!("attachment.deleted", name = filename).to_string());
    }

    Ok(())
}

/// Add attachment to the current note
pub fn add_attachment_to_current_note(app: &mut App, file_path: &str) -> Result<()> {
    let filtered = app.filtered_notes();
    let note_id = filtered.get(app.selected_note)
        .map(|note| note.id.clone())
        .ok_or_else(|| anyhow::anyhow!("{}", t!("note.no_notes")))?;

    add_attachment(app, &note_id, file_path)
}

/// Attach the file at `file_path` to a note
pub fn add_attachment(app: &mut App, note_id: &str, file_path: &str) -> Result<()> {
    if app.notes.iter().any(|note| note.id == note_id && note.locked) {
        anyhow::bail!("{}", t!("note.locked_cannot_edit"));
    }

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

    log_debug("add_attachment: Starting");

    // Expand tilde in path
    let expanded_path = expand_tilde(file_path)?;

    // Validate file exists
    if !expanded_path.exists() {
        log_debug("add_attachment: File not found");
        anyhow::bail!("File not found: {}", file_path);
    }

    if !expanded_path.is_file() {
        anyhow::bail!("Path is not a file: {}", file_path);
    }

    // Check file size
    let metadata = std::fs::metadata(&expanded_path)
        .context("Failed to read file metadata")?;
    let size_bytes = metadata.len();

    if size_bytes > MAX_ATTACHMENT_BYTES {
        anyhow::bail!("{}", t!("attachment.too_large", max_mb = MAX_ATTACHMENT_BYTES / (1024 * 1024)));
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
        .get(note_id, key)?
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

    app.notify(t!("attachment.added", name = filename).to_string());

    // Trigger sync to push the attachment to the server
    if app.settings.sync_enabled {
        app.trigger_sync();
    }

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

    if note.locked {
        anyhow::bail!("{}", t!("note.locked_cannot_edit"));
    }

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

    // Update note
    note.modified_at = chrono::Utc::now();
    note.version += 1;

    note_repo.update(&note, key)?;

    // Delete from database, unless a duplicate of this note shares it
    if !note_repo.attachment_in_use(&attachment.id)? {
        AttachmentRepository::new(db.connection()).delete(&attachment.id)?;
    }

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
    let mut temp_file = crate::secure_temp::create("")?;
    temp_file
        .write_all(app.note_input.as_bytes())
        .context("Failed to write to temporary file")?;
    temp_file.flush()?;

    if crate::secure_temp::take_disk_warning() {
        app.notify(t!("editor.temp_on_disk").to_string());
    }

    let temp_path = temp_file.path();
    let editor = external::editor_command();

    with_terminal_suspended(app, || external::run_editor(&editor, temp_path))?;

    // Read modified content
    let content = std::fs::read_to_string(temp_path)
        .context("Failed to read modified content")?;

    Ok(content)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crypto::CryptoService;
    use crate::db::Database;
    use crate::models::Note;
    use crate::ui::input::attachment_path::handle_attachment_path_key;
    use crate::ui::state::AppState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    #[test]
    fn inert_types_open_with_a_suffix_from_the_list() {
        assert_eq!(inert_suffix("image/png", "photo.png"), Some("png"));
        assert_eq!(inert_suffix("IMAGE/JPEG", "photo.jpeg"), Some("jpg"));
        assert_eq!(inert_suffix("text/plain; charset=utf-8", "notes.txt"), Some("txt"));
        assert_eq!(inert_suffix("application/pdf", "report.pdf"), Some("pdf"));
    }

    /// The reported attack: the sender picks the name, and the extension used to
    /// choose the handler. A .terminal file can run a command.
    #[test]
    fn the_filename_cannot_pick_the_handler() {
        assert_eq!(inert_suffix("text/plain", "invoice.terminal"), Some("txt"));
        assert_eq!(inert_suffix("image/png", "payload.command"), Some("png"));
    }

    #[test]
    fn active_types_are_never_opened() {
        let refused = [
            ("text/html", "page.html"),
            ("image/svg+xml", "drawing.svg"),
            ("application/x-apple-diskimage", "setup.dmg"),
            ("application/octet-stream", "invoice.terminal"),
            ("application/octet-stream", "run.command"),
            ("application/octet-stream", "link.webloc"),
            ("application/octet-stream", "link.fileloc"),
            ("application/octet-stream", "app.desktop"),
            ("application/octet-stream", "setup.pkg"),
            ("application/octet-stream", "page.html"),
            ("", "no-extension"),
            ("", "page.html"),
        ];
        for (mime_type, filename) in refused {
            assert_eq!(inert_suffix(mime_type, filename), None, "{mime_type} {filename}");
        }
    }

    #[test]
    fn a_known_extension_stands_in_for_a_missing_type() {
        assert_eq!(inert_suffix("application/octet-stream", "photo.PNG"), Some("png"));
        assert_eq!(inert_suffix("", "report.pdf"), Some("pdf"));
    }

    #[test]
    fn a_saved_filename_cannot_leave_the_chosen_folder() {
        assert_eq!(safe_filename("../../.ssh/authorized_keys"), "authorized_keys");
        assert_eq!(safe_filename("..\\..\\windows\\evil.dll"), "evil.dll");
        assert_eq!(safe_filename(".."), "attachment");
        assert_eq!(safe_filename(""), "attachment");
        assert_eq!(safe_filename("a\u{1b}[31mb.txt"), "a[31mb.txt");
    }

    fn enter() -> KeyEvent {
        KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE)
    }

    /// An unlocked app with one selected note, whose attachment is `data`.
    fn app_with_attachment(mime_type: &str, filename: &str, data: &[u8]) -> (App, Attachment) {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);

        let key = CryptoService::new().generate_master_key();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let attachment = Attachment {
            id: uuid::Uuid::new_v4().to_string(),
            filename: filename.to_string(),
            mime_type: mime_type.to_string(),
            size: data.len() as i64,
            data: String::new(),
            thumbnail_data: None,
        };
        AttachmentRepository::new(db.connection())
            .store(&attachment.id, filename, mime_type, data.len() as i64, data, &key)
            .unwrap();

        let mut note = Note::new("Has a file".to_string());
        note.attachments.push(attachment.clone());
        NoteRepository::new(db.connection()).create(&note, &key).unwrap();

        app.notes.push(note);
        app.db = Some(db);
        app.key = Some(key);
        app.state = AppState::NoteList;
        (app, attachment)
    }

    #[test]
    fn viewing_an_active_type_asks_instead_of_opening() {
        let (mut app, attachment) = app_with_attachment("text/html", "page.html", b"<script>");

        view_attachment(&mut app, &attachment).unwrap();

        assert_eq!(app.attachment_to_confirm.as_ref().unwrap().filename, "page.html");
    }

    #[test]
    fn an_attachment_saves_to_a_chosen_path() {
        let (mut app, attachment) = app_with_attachment("application/zip", "archive.zip", b"bytes");
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("saved.zip");

        save_attachment(&mut app, &attachment, target.to_str().unwrap()).unwrap();

        assert_eq!(std::fs::read(target).unwrap(), b"bytes");
    }

    #[test]
    fn saving_into_a_folder_uses_a_safe_name() {
        let (mut app, attachment) = app_with_attachment("text/plain", "../../escape.txt", b"bytes");
        let dir = tempfile::tempdir().unwrap();

        let saved = save_attachment(&mut app, &attachment, dir.path().to_str().unwrap()).unwrap();

        assert_eq!(saved, dir.path().join("escape.txt"));
        assert_eq!(std::fs::read(saved).unwrap(), b"bytes");
    }

    #[test]
    fn saving_never_replaces_a_file() {
        let (mut app, attachment) = app_with_attachment("text/plain", "a.txt", b"new");
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a.txt");
        std::fs::write(&target, "mine").unwrap();

        assert!(save_attachment(&mut app, &attachment, target.to_str().unwrap()).is_err());

        assert_eq!(std::fs::read_to_string(target).unwrap(), "mine");
    }

    #[test]
    fn the_path_prompt_saves_when_it_was_opened_for_saving() {
        let (mut app, attachment) = app_with_attachment("application/zip", "a.zip", b"bytes");
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("a.zip");

        begin_save_attachment(&mut app, attachment);
        app.attachment_path_input = target.to_string_lossy().into_owned();
        handle_attachment_path_key(&mut app, enter()).unwrap();

        assert!(target.exists());
        assert!(matches!(app.input_mode, InputMode::Normal));
        assert!(app.attachment_to_save.is_none());
    }

    #[test]
    fn the_path_prompt_attaches_when_it_was_opened_for_adding() {
        let (mut app, _) = app_with_attachment("text/plain", "first.txt", b"one");
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("second.txt");
        std::fs::write(&file, "two").unwrap();

        begin_add_attachment(&mut app).unwrap();
        app.attachment_path_input = file.to_string_lossy().into_owned();
        handle_attachment_path_key(&mut app, enter()).unwrap();

        assert!(app.error.is_none(), "{:?}", app.error);
        let conn = app.db.as_ref().unwrap().connection();
        assert_eq!(AttachmentRepository::new(conn).count().unwrap(), 2);
    }

    #[test]
    fn a_locked_note_takes_no_attachments() {
        let (mut app, _) = app_with_attachment("text/plain", "first.txt", b"one");
        app.notes[0].locked = true;

        assert!(begin_add_attachment(&mut app).is_err());
        assert!(!matches!(app.input_mode, InputMode::AttachmentPath));
    }

    /// The check is here rather than at each key, because there's more than one way in.
    #[test]
    fn a_locked_note_keeps_its_attachments() {
        let (mut app, attachment) = app_with_attachment("text/plain", "first.txt", b"one");
        let note_id = app.notes[0].id.clone();
        app.notes[0].locked = true;
        let db = app.db.as_ref().unwrap();
        NoteRepository::new(db.connection()).update(&app.notes[0], app.key.as_ref().unwrap()).unwrap();

        assert!(delete_current_attachment(&mut app).is_err());
        assert!(remove_attachment_from_current_note(&mut app).is_err());

        let path = std::env::temp_dir().join(format!("jottery-locked-{}.txt", uuid::Uuid::new_v4()));
        std::fs::write(&path, "more").unwrap();
        assert!(add_attachment_to_current_note(&mut app, path.to_str().unwrap()).is_err());
        std::fs::remove_file(&path).unwrap();

        let db = app.db.as_ref().unwrap();
        let stored = NoteRepository::new(db.connection()).get(&note_id, app.key.as_ref().unwrap()).unwrap().unwrap();
        assert_eq!(stored.attachments.len(), 1);
        assert_eq!(stored.attachments[0].id, attachment.id);
    }

    /// A duplicate shares its original's files, so removing one from either keeps it.
    #[test]
    fn a_file_a_duplicate_still_uses_stays() {
        let (mut app, attachment) = app_with_attachment("text/plain", "first.txt", b"one");
        let original_id = app.notes[0].id.clone();
        crate::ui::operations::notes::duplicate_selected(&mut app).unwrap();

        app.selected_note_id = Some(original_id);
        app.keep_selection_on_note();
        delete_current_attachment(&mut app).unwrap();

        let conn = app.db.as_ref().unwrap().connection();
        assert_eq!(AttachmentRepository::new(conn).list_ids().unwrap(), [attachment.id]);
    }

    #[test]
    fn a_file_over_the_limit_is_refused() {
        let (mut app, _) = app_with_attachment("text/plain", "first.txt", b"one");
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("big.bin");
        std::fs::File::create(&file).unwrap().set_len(MAX_ATTACHMENT_BYTES + 1).unwrap();

        assert!(add_attachment_to_current_note(&mut app, file.to_str().unwrap()).is_err());
    }
}
