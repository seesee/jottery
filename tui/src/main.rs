mod api;
mod backup;
mod crypto;
mod db;
mod export;
mod models;
mod password_storage;
mod repository;
mod search;
mod secure_temp;
mod sse;
mod terminal_text;
#[cfg(test)]
mod test_support;
mod ui;
mod vault;

// Initialize rust-i18n with fallback to en-GB
rust_i18n::i18n!("locales", fallback = "en-GB");

use anyhow::{Context, Result};
use rust_i18n::t;
use clap::{Parser, Subcommand, CommandFactory, FromArgMatches};
use std::path::{Path, PathBuf};
use std::fs::OpenOptions;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::io::{self, Read, Write};
use std::env;
use tracing::info;

use crypto::CryptoService;
use db::Database;
use models::{Note, get_note_title, get_regular_tags};
use repository::{NoteRepository, sync::SyncRepository};
use terminal_text::{errln, outln};
use ui::{App, EventHandler, Tui};

/// Detect and set locale from environment
fn detect_locale() {
    if let Some(locale) = models::system_locale() {
        rust_i18n::set_locale(&locale);
    }
}

/// Build CLI command with translated help text
fn build_translated_cli() -> clap::Command {

    // Start with the base command from derive
    let mut cmd = Cli::command();

    // Update main app description
    cmd = cmd.about(t!("cli.about").to_string());

    // Update global options
    cmd = cmd.mut_arg("database", |a| a.help(t!("cli.database_help").to_string()));
    cmd = cmd.mut_arg("debug", |a| a.help(t!("cli.debug_help").to_string()));
    cmd = cmd.mut_arg("debug_log", |a| a.help(t!("cli.debug_log_help").to_string()));
    cmd = cmd.mut_arg("reset", |a| a.help(t!("cli.reset_help").to_string()));
    cmd = cmd.mut_arg("allow_insecure_http", |a| a.help(t!("cli.allow_insecure_http_help").to_string()));
    cmd = cmd.mut_arg("password_stdin", |a| a.help(t!("cli.password_stdin_help").to_string()));

    // Update subcommands
    cmd = cmd.mut_subcommand("note", |sub| {
        sub.about(t!("cli.note_about").to_string())
            .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
            .mut_arg("tags", |a| a.help(t!("cli.tags_help").to_string()))
    });

    cmd = cmd.mut_subcommand("list", |sub| {
        sub.about(t!("cli.list_about").to_string())
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
            .mut_arg("tag", |a| a.help(t!("cli.tag_filter_help").to_string()))
            .mut_arg("limit", |a| a.help(t!("cli.limit_help").to_string()))
            .mut_arg("archived", |a| a.help(t!("cli.archived_help").to_string()))
    });

    cmd = cmd.mut_subcommand("search", |sub| {
        sub.about(t!("cli.search_about").to_string())
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
            .mut_arg("limit", |a| a.help(t!("cli.limit_results_help").to_string()))
    });

    cmd = cmd.mut_subcommand("show", |sub| {
        sub.about(t!("cli.show_about").to_string())
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("attach", |sub| {
        sub.about(t!("cli.attach_about").to_string())
            .mut_arg("id", |a| a.help(t!("cli.note_id_help").to_string()))
            .mut_arg("file", |a| a.help(t!("cli.attach_file_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("save-attachment", |sub| {
        sub.about(t!("cli.save_attachment_about").to_string())
            .mut_arg("id", |a| a.help(t!("cli.note_id_help").to_string()))
            .mut_arg("number", |a| a.help(t!("cli.attachment_number_help").to_string()))
            .mut_arg("output", |a| a.help(t!("cli.output_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("archive", |sub| {
        sub.about(t!("cli.archive_about").to_string())
            .mut_arg("id", |a| a.help(t!("cli.note_id_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("unarchive", |sub| {
        sub.about(t!("cli.unarchive_about").to_string())
            .mut_arg("id", |a| a.help(t!("cli.note_id_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("sync", |sub| {
        sub.about(t!("cli.sync_about").to_string())
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("register-user", |sub| {
        sub.about(t!("cli.register_user_about").to_string())
            .mut_arg("server", |a| a.help(t!("cli.server_help").to_string()))
            .mut_arg("email", |a| a.help(t!("cli.email_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
    });

    cmd = cmd.mut_subcommand("register-device", |sub| {
        sub.about(t!("cli.register_device_about").to_string())
            .mut_arg("server", |a| a.help(t!("cli.server_help").to_string()))
            .mut_arg("email", |a| a.help(t!("cli.email_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
            .mut_arg("device_name", |a| a.help(t!("cli.device_name_help").to_string()))
            .mut_arg("key_password", |a| a.help(t!("cli.key_password_help").to_string()))
    });

    cmd = cmd.mut_subcommand("export", |sub| {
        sub.about(t!("cli.export_about").to_string())
            .mut_arg("output", |a| a.help(t!("cli.output_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_decrypt_help").to_string()))
    });

    cmd = cmd.mut_subcommand("import", |sub| {
        sub.about(t!("cli.import_about").to_string())
            .mut_arg("input", |a| a.help(t!("cli.input_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
    });

    cmd = cmd.mut_subcommand("backup", |sub| {
        sub.about(t!("cli.backup_about").to_string())
            .mut_arg("output", |a| a.help(t!("cli.output_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
    });

    cmd = cmd.mut_subcommand("inbox-token", |sub| {
        sub.about(t!("cli.inbox_token_about").to_string())
            .mut_subcommand("generate", |s| {
                s.about(t!("cli.inbox_generate_about").to_string())
                    .mut_arg("server", |a| a.help(t!("cli.server_optional_help").to_string()))
                    .mut_arg("email", |a| a.help(t!("cli.email_help").to_string()))
                    .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
            })
            .mut_subcommand("revoke", |s| {
                s.about(t!("cli.inbox_revoke_about").to_string())
                    .mut_arg("server", |a| a.help(t!("cli.server_optional_help").to_string()))
                    .mut_arg("email", |a| a.help(t!("cli.email_help").to_string()))
                    .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
            })
            .mut_subcommand("status", |s| {
                s.about(t!("cli.inbox_status_about").to_string())
                    .mut_arg("server", |a| a.help(t!("cli.server_optional_help").to_string()))
                    .mut_arg("email", |a| a.help(t!("cli.email_help").to_string()))
                    .mut_arg("password", |a| a.help(t!("cli.password_help").to_string()))
            })
    });

    cmd = cmd.mut_subcommand("restore-backup", |sub| {
        sub.about(t!("cli.restore_backup_about").to_string())
            .mut_arg("input", |a| a.help(t!("cli.input_help").to_string()))
            .mut_arg("password", |a| a.help(t!("cli.backup_password_help").to_string()))
            .mut_arg("key_password", |a| a.help(t!("cli.local_key_help").to_string()))
    });

    cmd = cmd.mut_subcommand("store-password", |sub| {
        sub.about(t!("cli.store_password_about").to_string())
            .mut_arg("password", |a| a.help(t!("cli.password_store_help").to_string()))
    });

    // Note: The built-in help (-h/--help) and version (-V/--version) flags
    // are special clap flags and cannot be translated via mut_arg.
    // The "help" subcommand is auto-generated and also cannot be easily customized.

    cmd
}

#[derive(Parser)]
#[command(name = "jottery")]
#[command(version = env!("CARGO_PKG_VERSION"))]
#[command(about = "A privacy-focused, encrypted note-taking TUI", long_about = None)]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "jottery.db")]
    database: PathBuf,

    /// Enable debug logging
    #[arg(long)]
    debug: bool,

    /// Debug log file path (for troubleshooting)
    #[arg(long)]
    debug_log: Option<PathBuf>,

    /// Reset: delete the database and start fresh
    #[arg(long)]
    reset: bool,

    /// Allow sending a password over plain HTTP to a public host
    #[arg(long, global = true)]
    allow_insecure_http: bool,

    /// Read passwords from stdin, one per line, instead of prompting
    #[arg(long, global = true)]
    password_stdin: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    /// Create a quick note with $EDITOR
    Note {
        /// Password for encryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Optional tags for the note (comma-separated)
        #[arg(short, long)]
        tags: Option<String>,
    },
    /// List all notes
    List {
        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Filter by tag
        #[arg(short, long)]
        tag: Option<String>,

        /// Maximum number of notes to show
        #[arg(short = 'n', long, default_value = "20")]
        limit: usize,

        /// List archived notes instead
        #[arg(long)]
        archived: bool,
    },
    /// Search notes
    Search {
        /// Search query
        query: String,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Maximum number of results to show
        #[arg(short = 'n', long, default_value = "10")]
        limit: usize,
    },
    /// Show a specific note by ID
    Show {
        /// Note ID (partial match supported)
        id: String,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Attach a file to a note
    Attach {
        /// Note ID (partial match supported)
        id: String,

        /// File to attach
        file: PathBuf,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Save a note's attachment to a file
    SaveAttachment {
        /// Note ID (partial match supported)
        id: String,

        /// Which attachment, as numbered by show
        #[arg(short = 'n', long, default_value = "1")]
        number: usize,

        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Archive a note
    Archive {
        /// Note ID (partial match supported)
        id: String,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Bring a note back from the archive
    Unarchive {
        /// Note ID (partial match supported)
        id: String,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Sync notes with remote server
    Sync {
        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Register a new user account on the server
    RegisterUser {
        /// Server endpoint URL
        #[arg(short, long)]
        server: String,

        /// User email
        #[arg(short, long)]
        email: String,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Register this device with the server (requires approved user account)
    RegisterDevice {
        /// Server endpoint URL
        #[arg(short, long)]
        server: String,

        /// User email
        #[arg(short, long)]
        email: String,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Device name
        #[arg(short = 'n', long, default_value = "TUI")]
        device_name: String,

        /// Encryption password for local database
        #[arg(short = 'k', long)]
        key_password: Option<String>,
    },
    /// Export notes to JSON file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Import notes from JSON file
    Import {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Password for encryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Create encrypted backup (compatible with web app)
    Backup {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Password for encryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Manage inbox API token
    InboxToken {
        #[command(subcommand)]
        action: InboxTokenAction,
    },
    /// Restore from encrypted backup
    RestoreBackup {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Password for decryption (backup password, will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,

        /// Local database encryption password (defaults to backup password)
        #[arg(short = 'k', long)]
        key_password: Option<String>,
    },
    /// Store password for auto-unlock (used for demo/testing automation)
    StorePassword {
        /// Password to store (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
}

#[derive(Subcommand)]
enum InboxTokenAction {
    /// Generate a new inbox token (requires server login)
    Generate {
        /// Server endpoint URL (uses configured endpoint if omitted)
        #[arg(short, long)]
        server: Option<String>,

        /// User email
        #[arg(short, long)]
        email: String,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Revoke the current inbox token
    Revoke {
        /// Server endpoint URL
        #[arg(short, long)]
        server: Option<String>,

        /// User email
        #[arg(short, long)]
        email: String,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Check if an inbox token exists
    Status {
        /// Server endpoint URL
        #[arg(short, long)]
        server: Option<String>,

        /// User email
        #[arg(short, long)]
        email: String,

        /// Password (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
}

/// Open the debug log for this user alone. What it holds is meant to be safe, but it
/// says when notes were touched and how a vault is set up, which is nobody else's.
fn open_debug_log(path: &Path) -> Result<Arc<Mutex<std::fs::File>>> {
    let mut options = OpenOptions::new();
    options.create(true).append(true);

    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }

    let file = options.open(path)
        .with_context(|| format!("Failed to open debug log: {}", path.display()))?;

    // An existing log keeps its permissions when opened, so they're set here as well
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))
            .context("Failed to restrict the debug log")?;
    }

    Ok(Arc::new(Mutex::new(file)))
}

/// The debug log as a place for tracing to write, shared with the app's own lines.
#[derive(Clone)]
struct SharedLog(Arc<Mutex<std::fs::File>>);

impl Write for SharedLog {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        match self.0.lock() {
            Ok(mut file) => file.write(bytes),
            Err(_) => Ok(bytes.len()),
        }
    }

    fn flush(&mut self) -> io::Result<()> {
        match self.0.lock() {
            Ok(mut file) => file.flush(),
            Err(_) => Ok(()),
        }
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for SharedLog {
    type Writer = SharedLog;

    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

/// Set by --password-stdin
static PASSWORD_ON_STDIN: AtomicBool = AtomicBool::new(false);

/// Prompt for password from stdin
fn prompt_password() -> Result<String> {
    print!("{}", t!("password.prompt"));
    io::stdout().flush()?;
    let password = rpassword::read_password()?;
    Ok(password)
}

/// The next password piped in with --password-stdin, one per line.
fn password_from_stdin() -> Result<Option<String>> {
    if !PASSWORD_ON_STDIN.load(Ordering::Relaxed) {
        return Ok(None);
    }

    // Byte by byte, because anything read past the line belongs to the command, such as
    // a note piped in after the password
    let mut line = Vec::new();
    let mut byte = [0u8; 1];
    while io::stdin().read(&mut byte)? == 1 && byte[0] != b'\n' {
        line.push(byte[0]);
    }

    let line = String::from_utf8(line).context("The password on stdin isn't valid UTF-8")?;
    Ok(Some(line.trim_end_matches('\r').to_string()))
}

/// A password given without a prompt. On the command line it's visible in `ps` and shell
/// history, so that still works for existing scripts but says so.
fn supplied_password(flag: Option<String>, env_var: &str) -> Result<Option<String>> {
    if let Some(password) = flag {
        errln!("⚠ {}", t!("cli.password_flag_deprecated"));
        return Ok(Some(password));
    }

    if let Some(password) = password_from_stdin()? {
        return Ok(Some(password));
    }

    Ok(env::var(env_var).ok().filter(|password| !password.is_empty()))
}

/// The notes password, if it was given without a prompt.
fn supplied_notes_password(flag: Option<String>) -> Result<Option<String>> {
    supplied_password(flag, "JOTTERY_PASSWORD")
}

/// The account password, from wherever it was given, or by asking.
fn account_password(flag: Option<String>) -> Result<String> {
    match supplied_password(flag, "JOTTERY_ACCOUNT_PASSWORD")? {
        Some(password) => Ok(password),
        None => prompt_password(),
    }
}

/// A password that has no stored copy to fall back on.
fn required_password(flag: Option<String>) -> Result<String> {
    match supplied_notes_password(flag)? {
        Some(password) => Ok(password),
        None => prompt_password(),
    }
}

/// Try to load stored password using the password storage backend
///
/// For CLI usage, we use file-based storage directly since keychain may not be
/// accessible from non-interactive contexts (cron jobs, scripts, etc.)
fn try_load_stored_password(db_path: &Path) -> Result<Option<String>> {
    let config_dir = db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;

    // Use file storage for CLI (keychain may not be accessible in headless mode)
    let storage = password_storage::create_file_storage(config_dir);

    match storage.retrieve() {
        password_storage::RetrieveResult::Found(password) => Ok(Some(password)),
        password_storage::RetrieveResult::NotFound => Ok(None),
        password_storage::RetrieveResult::Error(e) => Err(e),
    }
}

/// Get or prompt for password, checking stored password first
fn get_password(password_opt: Option<String>, db_path: &Path) -> Result<String> {
    match supplied_notes_password(password_opt)? {
        Some(pwd) => Ok(pwd),
        None => {
            // Try to load stored password first
            if let Ok(Some(stored)) = try_load_stored_password(db_path) {
                Ok(stored)
            } else {
                prompt_password()
            }
        }
    }
}

/// Get the vault's key from its password, creating the vault if this is the first run
fn derive_key_from_db(db: &Database, password: &str) -> Result<[u8; 32]> {
    vault::unlock(db.connection(), &CryptoService::new(), password)
}

/// Open $EDITOR with content and return the edited result
fn open_editor(initial_content: &str) -> Result<String> {
    // Create temporary file
    let mut temp_file = secure_temp::create("")
        .context(t!("editor.temp_file_error"))?;
    temp_file
        .write_all(initial_content.as_bytes())
        .context(t!("editor.temp_write_error"))?;
    temp_file.flush()?;

    if secure_temp::take_disk_warning() {
        errln!("⚠ {}", t!("editor.temp_on_disk"));
    }

    let temp_path = temp_file.path();

    secure_temp::set_child_has_terminal(true);
    let edited = ui::operations::external::run_editor(&ui::operations::external::editor_command(), temp_path);
    secure_temp::set_child_has_terminal(false);
    edited?;

    // Read the edited content
    let content = std::fs::read_to_string(temp_path)
        .context(t!("editor.temp_read_error"))?;

    Ok(content)
}

/// Sync from the CLI, the same way the TUI does: a push and a full pull.
fn perform_cli_sync(db_path: &Path, db: Database, key: [u8; 32]) -> Result<ui::operations::sync::SyncOutcome> {
    let mut app = App::headless(db_path.to_path_buf(), db, key)?;
    ui::operations::notes::load_notes(&mut app)?;
    ui::operations::sync::perform_sync(&mut app, false)
}

/// Open the vault for a command that changes a note, and find that note.
fn open_note(db_path: &Path, password: Option<String>, id: &str) -> Result<(App, models::Note)> {
    let password = get_password(password, db_path)?;
    let db = Database::open(db_path, &password)
        .context("Failed to open database. Check your password.")?;
    let key = derive_key_from_db(&db, &password)?;

    let mut app = App::headless(db_path.to_path_buf(), db, key)?;
    ui::operations::notes::load_notes(&mut app)?;

    let note = app.all_notes().iter()
        .find(|note| note.id.starts_with(id))
        .cloned()
        .context(t!("error.file_not_found"))?;

    Ok((app, note))
}

/// Push a change made by a command, if this device syncs. The change is safe
/// locally whatever happens, so a failure here is a warning.
fn sync_if_connected(app: &mut App) {
    let syncs = app.sync_metadata().is_some_and(|metadata| metadata.sync_enabled);
    if syncs {
        match ui::operations::sync::perform_sync(app, false) {
            Ok(outcome) => report_sync(&outcome),
            Err(e) => errln!("⚠ {}: {}", t!("sync.failed", error = ""), e),
        }
    }
}

/// Move a note in or out of the archive from the CLI.
fn set_note_archived(db_path: &Path, password: Option<String>, id: &str, archived: bool) -> Result<()> {
    let (mut app, note) = open_note(db_path, password, id)?;

    if note.locked {
        anyhow::bail!("{}", t!("note.locked_cannot_archive"));
    }

    ui::operations::notes::set_archived(&mut app, &note.id, archived)?;
    let done = if archived { t!("archive.archived") } else { t!("archive.unarchived") };
    outln!("✓ {}: {}", done, models::char_prefix(&note.id, 8));

    sync_if_connected(&mut app);
    Ok(())
}

/// Say what a finished sync did, including anything it couldn't do.
fn report_sync(outcome: &ui::operations::sync::SyncOutcome) {
    // codeql[rust/cleartext-logging] these are note counts; CodeQL taints the sync result via the `?` on the api_key
    outln!("✓ {}: {}", t!("sync.success"), outcome.synced);
    if outcome.rejected > 0 {
        errln!("⚠ {}", t!("sync.rejected_notes", count = outcome.rejected));
    }
    if outcome.skipped > 0 {
        errln!("⚠ {}", t!("sync.skipped_notes", count = outcome.skipped));
    }
    if outcome.quota_exceeded {
        errln!("⚠ {}", t!("sync.quota_exceeded"));
    }
    if outcome.deletions_held > 0 {
        errln!("⚠ {}", t!("sync.deletions_held_cli", count = outcome.deletions_held));
    }
}

/// Format a note for display
fn format_note_preview(note: &Note, show_content: bool) -> String {
    // Use custom title tag or first non-empty line
    let title = get_note_title(&note.content, &note.tags);
    let title_str = if title.is_empty() { "(empty)" } else { &title };

    // Filter out virtual tags for display
    let regular_tags = get_regular_tags(&note.tags);
    let tags_str = if regular_tags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", regular_tags.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", "))
    };

    let preview = if show_content {
        let content_preview = note.content.lines()
            .take(3)
            .collect::<Vec<_>>()
            .join("\n");
        format!("\n{}\n", content_preview)
    } else {
        String::new()
    };

    format!("{} - {}{}{}",
        models::char_prefix(&note.id, 8),
        title_str,
        tags_str,
        preview
    )
}

fn main() {
    // Reported here, not by returning the error, because a failure can quote
    // whatever a server sent and Rust's default report would print it raw.
    let result = run();
    secure_temp::remove_everything();

    if let Err(error) = result {
        errln!("Error: {:?}", error);
        std::process::exit(1);
    }
}

fn run() -> Result<()> {
    // Detect and set locale from environment BEFORE parsing CLI
    // This ensures --help output is translated
    detect_locale();

    // Build translated CLI and parse arguments
    let cmd = build_translated_cli();
    let matches = cmd.get_matches();
    let cli = Cli::from_arg_matches(&matches)?;

    // Initialize logging. With a log file it all goes there, and otherwise to stderr,
    // because stdout is a command's output and the terminal is the TUI's. This crate is
    // called jottery, whatever the package is called.
    let debug_log = cli.debug_log.as_deref().map(open_debug_log).transpose()?;

    let log_level = if cli.debug || debug_log.is_some() { "debug" } else { "error" };
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        .add_directive(format!("jottery={}", log_level).parse()?);

    match &debug_log {
        Some(file) => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_ansi(false)
            .with_writer(SharedLog(file.clone()))
            .init(),
        None => tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(io::stderr)
            .init(),
    }

    info!("Jottery TUI v{}", env!("CARGO_PKG_VERSION"));

    // Get absolute path to database
    // Priority: JOTTERY_DB_PATH env var > --database flag > default
    let db_path = if let Ok(env_db) = env::var("JOTTERY_DB_PATH") {
        info!("Using database from JOTTERY_DB_PATH: {}", env_db);
        PathBuf::from(env_db)
    } else if cli.database.is_absolute() {
        info!("Database: {}", cli.database.display());
        cli.database
    } else {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("jottery");
        std::fs::create_dir_all(&config_dir)?;

        // This directory is Jottery's own, unlike one named by a path above
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&config_dir, std::fs::Permissions::from_mode(0o700))?;
        }

        config_dir.join(&cli.database)
    };

    info!("Using database: {}", db_path.display());

    // Handle reset flag
    if cli.reset {
        if db_path.exists() {
            info!("Deleting database: {}", db_path.display());
            std::fs::remove_file(&db_path)?;

            // Also remove SQLite WAL and SHM files if they exist
            let wal_path = db_path.with_extension("db-wal");
            let shm_path = db_path.with_extension("db-shm");
            let _ = std::fs::remove_file(&wal_path);
            let _ = std::fs::remove_file(&shm_path);

            outln!("✓ {}: {}", t!("note.deleted"), db_path.display());
            outln!("{}", t!("password.change_success"));
        } else {
            outln!("{}: {}", t!("error.file_not_found"), db_path.display());
        }
        return Ok(());
    }

    // Whatever a killed run left behind goes first. A command only needs the handler
    // for its editor. The TUI also has a terminal to put back.
    secure_temp::sweep_leftovers();
    secure_temp::install_signal_cleanup(cli.command.is_none());

    // The TUI says this on its unlock screen. A script only needs telling once.
    if cli.command.is_some() && !db_path.exists() && !Database::encrypts_files() {
        errln!("⚠ {}", t!("password.no_file_encryption"));
    }

    let allow_insecure_http = cli.allow_insecure_http;
    PASSWORD_ON_STDIN.store(cli.password_stdin, Ordering::Relaxed);

    // Handle subcommands
    match cli.command {
        Some(Commands::Note { password, tags }) => {
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            // Check if content is being piped in
            use std::io::IsTerminal;
            let content = if !io::stdin().is_terminal() {
                // Reading from pipe
                let mut buffer = String::new();
                io::stdin().read_to_string(&mut buffer)
                    .context("Failed to read from stdin")?;
                buffer
            } else {
                // No pipe, open editor
                open_editor("")?
            };

            // If content is empty, don't create note
            if content.trim().is_empty() {
                outln!("{}", t!("note.save_failed", error = t!("password.empty_error")));
                return Ok(());
            }

            // Parse tags if provided
            let note_tags: Vec<String> = tags
                .map(|t| t.split(',').map(|s| s.trim().to_string()).collect())
                .unwrap_or_default();

            // Create note struct
            let mut note = Note::new(content);
            note.tags = note_tags;

            // Save to database
            let note_repo = NoteRepository::new(db.connection());
            note_repo.create(&note, &key)?;
            outln!("✓ {}: {}", t!("cli.new"), models::char_prefix(&note.id, 8));

            // Auto-sync if configured
            let metadata = SyncRepository::new(db.connection()).get_metadata();
            if let Ok(Some(metadata)) = metadata {
                if metadata.sync_enabled {
                    outln!("{} {}...", t!("sync.pushing"), metadata.sync_endpoint);
                    match perform_cli_sync(&db_path, db, key) {
                        Ok(outcome) => report_sync(&outcome),
                        Err(e) => {
                            errln!("⚠ {}: {}", t!("sync.failed", error = ""), e);
                            errln!("  {}", t!("sync.request_failed"));
                        }
                    }
                }
            }

            return Ok(());
        }
        Some(Commands::List { password, tag, limit, archived }) => {
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let note_repo = NoteRepository::new(db.connection());
            let notes = if archived {
                note_repo.get_archived(&key)?
            } else {
                note_repo.list(false, &key)?
            };

            // Filter by tag if specified
            let filtered_notes: Vec<_> = if let Some(tag_filter) = tag {
                notes.into_iter()
                    .filter(|n| n.tags.contains(&tag_filter))
                    .take(limit)
                    .collect()
            } else {
                notes.into_iter().take(limit).collect()
            };

            if filtered_notes.is_empty() {
                outln!("{}", t!("note.no_notes"));
            } else {
                outln!("{}\n", t!("note.total", count = filtered_notes.len()));
                for note in filtered_notes {
                    outln!("{}", format_note_preview(&note, false));
                }
            }

            return Ok(());
        }
        Some(Commands::Search { query, password, limit }) => {
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let note_repo = NoteRepository::new(db.connection());
            let notes = note_repo.list_not_deleted(&key)?.notes;
            let settings = repository::SettingsRepository::new(db.connection()).get()?;

            // The same query language as the TUI, with the same answer about the archive
            let parsed = search::SearchQuery::parse(&query, &settings);
            let archived = parsed.archived().unwrap_or(false);
            let results: Vec<_> = notes.into_iter()
                .filter(|note| note.archived == archived && parsed.matches(note, &settings))
                .take(limit)
                .collect();

            if results.is_empty() {
                outln!("{}: {}", t!("search.no_results"), query);
            } else {
                outln!("{}: '{}'\n", t!("search.results", count = results.len()), query);
                for note in results {
                    outln!("{}", format_note_preview(&note, true));
                }
            }

            return Ok(());
        }
        Some(Commands::Show { id, password }) => {
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let note_repo = NoteRepository::new(db.connection());
            let notes = note_repo.list_not_deleted(&key)?.notes;

            // Find note by partial ID match
            let note = notes.into_iter()
                .find(|n| n.id.starts_with(&id))
                .context(t!("error.file_not_found"))?;

            outln!("ID: {}", note.id);
            outln!("{}: {}", t!("editor.created"), note.created_at);
            outln!("{}: {}", t!("editor.modified"), note.modified_at);
            if !note.tags.is_empty() {
                outln!("{}: {}", t!("editor.tags"), note.tags.join(", "));
            }
            for (i, attachment) in note.attachments.iter().enumerate() {
                outln!("{} {}: {} ({})", t!("ui.attachments"), i + 1, attachment.filename, attachment.mime_type);
            }
            outln!("\n{}", note.content);

            return Ok(());
        }
        Some(Commands::Attach { id, file, password }) => {
            let (mut app, note) = open_note(&db_path, password, &id)?;

            if note.locked {
                anyhow::bail!("{}", t!("note.locked_cannot_edit"));
            }

            ui::operations::attachments::add_attachment(&mut app, &note.id, &file.to_string_lossy())?;
            outln!("✓ {}", t!("attachment.added", name = file.display().to_string()));

            sync_if_connected(&mut app);
            return Ok(());
        }
        Some(Commands::SaveAttachment { id, number, output, password }) => {
            let (mut app, note) = open_note(&db_path, password, &id)?;

            let attachment = number.checked_sub(1)
                .and_then(|index| note.attachments.get(index))
                .with_context(|| t!("attachment.no_attachments").to_string())?;

            let saved = ui::operations::attachments::save_attachment(&mut app, attachment, &output.to_string_lossy())?;
            outln!("✓ {}", t!("attachment.saved", path = saved.display().to_string()));
            return Ok(());
        }
        Some(Commands::Archive { id, password }) => {
            set_note_archived(&db_path, password, &id, true)?;
            return Ok(());
        }
        Some(Commands::Unarchive { id, password }) => {
            set_note_archived(&db_path, password, &id, false)?;
            return Ok(());
        }
        Some(Commands::RegisterUser { server, email, password }) => {
            use api::AuthClient;

            api::require_safe_endpoint(&server, allow_insecure_http)?;

            // Get password
            let password = account_password(password)?;

            // Validate password strength
            if password.len() < 12 {
                anyhow::bail!("{}", t!("register.password_too_short"));
            }

            // The server will see this password, so it mustn't be the one for the notes
            if let Ok(Some(notes_password)) = try_load_stored_password(&db_path) {
                vault::check_passwords_differ(&notes_password, &password)?;
            }

            outln!("{} {} {}...", t!("cli.register"), email, server);

            // Create auth client and register
            let client = AuthClient::new(server.clone());

            // Check server connectivity first
            if !client.health_check()? {
                anyhow::bail!("{} {}", t!("error.network", error = ""), server);
            }

            let response = client.register_user(&email, &password)
                .context(t!("sync.request_failed"))?;

            outln!("\n✓ {}", t!("sync.success"));
            // codeql[rust/cleartext-logging] user_id is a non-sensitive UUID identifier
            outln!("User ID: {}", response.user_id);
            outln!("Email: {}", response.email);
            outln!("{}: {}", t!("status.synced"), response.status);
            outln!("\n{}", response.message);

            if response.status == "pending_approval" {
                outln!("\n{}:", t!("help.general"));
                outln!("1. {}", t!("sync.account_deactivated"));
                outln!("2. {}", t!("cli.register"));
            }

            return Ok(());
        }
        Some(Commands::RegisterDevice { server, email, password, device_name, key_password }) => {
            use api::AuthClient;

            api::require_safe_endpoint(&server, allow_insecure_http)?;

            // Get user password
            let password = account_password(password)?;

            // Get encryption password for local database
            let key_password = match supplied_notes_password(key_password)? {
                Some(pwd) => pwd,
                None => {
                    print!("{}", t!("password.enter"));
                    io::stdout().flush()?;
                    let pwd1 = rpassword::read_password()?;
                    print!("{}", t!("password.confirm"));
                    io::stdout().flush()?;
                    let pwd2 = rpassword::read_password()?;

                    if pwd1 != pwd2 {
                        anyhow::bail!("{}", t!("password.mismatch_error"));
                    }
                    pwd1
                }
            };

            // The server sees the account password, so a notes password being chosen
            // now mustn't match it. An existing vault has already chosen, and refusing
            // would stop this person adding a device, so that only warns.
            let passwords_differ = vault::check_passwords_differ(&key_password, &password);
            if !db_path.exists() {
                passwords_differ.as_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
            }

            // Open the vault before involving the server, so a wrong password leaves
            // no stray device behind
            let db = Database::open(&db_path, &key_password)
                .context("Failed to create database")?;
            let vault_exists = repository::EncryptionRepository::new(db.connection()).get()?.is_some();
            if !vault_exists {
                passwords_differ.as_ref().map_err(|e| anyhow::anyhow!("{e}"))?;
            }
            let local_key = derive_key_from_db(&db, &key_password)?;
            if passwords_differ.is_err() {
                errln!("⚠ {}", t!("password.same_as_account_warning"));
            }

            outln!("{} '{}' {}...", t!("cli.register"), device_name, email);

            // Create auth client and register device
            let client = AuthClient::new(server.clone());

            // Check server connectivity first
            if !client.health_check()? {
                anyhow::bail!("{} {}", t!("error.network", error = ""), server);
            }

            let response = client.register_device(&email, &password, &device_name, "tui")
                .context(t!("sync.request_failed"))?;

            outln!("\n✓ {}", t!("sync.success"));
            outln!("Client ID: {}", response.client_id);
            // codeql[rust/cleartext-logging] user_id is a non-sensitive UUID identifier
            outln!("User ID: {}", response.user_id);
            outln!("{}: {}", t!("settings.device_name"), response.device_name);

            // Use the account's key if it has one, or give it this vault's. Notes
            // written under any other key couldn't be read on the account's devices.
            let crypto = CryptoService::new();
            let account = ui::operations::envelope::Account {
                client: &client,
                api_key: &response.api_key,
                user_id: &response.user_id,
            };
            let key = ui::operations::envelope::adopt_account_key(&db, &crypto, &account, &key_password, &local_key)?;

            // Encrypt and store API key
            let encrypted_api_key = crypto.encrypt_text(&response.api_key, &key)?;
            let api_key_json = serde_json::to_string(&encrypted_api_key)?;

            // Store sync configuration
            let sync_repo = SyncRepository::new(db.connection());
            let sync_metadata = models::sync::SyncMetadata {
                api_key: Some(api_key_json),
                client_id: Some(response.client_id),
                user_email: Some(email.clone()),
                user_id: Some(response.user_id.clone()),
                sync_enabled: true,
                sync_endpoint: server.clone(),
                auto_sync_interval: Some(1),
                // Adopting the key above recorded what this device knows about it
                server_key_seen: sync_repo.get_metadata()?.and_then(|stored| stored.server_key_seen),
                ..Default::default()
            };

            sync_repo.update_metadata(&sync_metadata)?;

            // The TUI reads these to decide whether to sync
            let settings_repo = repository::SettingsRepository::new(db.connection());
            let mut settings = settings_repo.get()?;
            settings.sync_enabled = true;
            settings.sync_endpoint = Some(server.clone());
            settings_repo.update(&settings)?;

            outln!("\n✓ {}", t!("sync.success"));
            outln!("\n{}:", t!("help.general"));
            outln!("1. {}: jottery note", t!("cli.new"));
            outln!("2. {}: jottery sync", t!("cli.sync"));
            outln!("3. {}: jottery", t!("menu.notes"));

            return Ok(());
        }
        Some(Commands::InboxToken { action }) => {
            use api::SessionClient;

            let (server, email, password) = match &action {
                InboxTokenAction::Generate { server, email, password } |
                InboxTokenAction::Revoke { server, email, password } |
                InboxTokenAction::Status { server, email, password } => {
                    (server.clone(), email.clone(), password.clone())
                }
            };

            let server = match server {
                Some(s) => s,
                None => {
                    // Try to get from database sync metadata
                    let pwd = get_password(None, &db_path)?;
                    let db = Database::open(&db_path, &pwd)
                        .context("Failed to open database")?;
                    let sync_repo = SyncRepository::new(db.connection());
                    match sync_repo.get_metadata()? {
                        Some(m) if !m.sync_endpoint.is_empty() => m.sync_endpoint,
                        _ => anyhow::bail!("No server specified and no sync endpoint configured. Use --server."),
                    }
                }
            };
            api::require_safe_endpoint(&server, allow_insecure_http)?;

            let password = account_password(password)?;

            // Both passwords being in hand is a chance to spot a dangerous reuse. It's
            // an existing account, so this warns: refusing would help nobody.
            if let Ok(Some(notes_password)) = try_load_stored_password(&db_path) {
                if vault::check_passwords_differ(&notes_password, &password).is_err() {
                    errln!("⚠ {}", t!("password.same_as_account_warning"));
                }
            }

            // Login to get session
            outln!("{}", t!("inbox.token.logging_in"));
            let session = SessionClient::login(&server, &email, &password)
                .context("Failed to login to server")?;

            match action {
                InboxTokenAction::Generate { .. } => {
                    let token = session.generate_inbox_token()
                        .context("Failed to generate inbox token")?;
                    outln!("\n{}", t!("inbox.token.generated_success"));
                    outln!("\n  {}\n", token);
                    outln!("{}", t!("inbox.token.save_warning"));
                    outln!("\n{}:", t!("inbox.token.example_title"));
                    outln!("  curl -X POST {}/api/v1/inbox \\", server);
                    outln!("    -H \"Authorization: Bearer {}\" \\", token);
                    outln!("    -H \"Content-Type: application/json\" \\");
                    outln!("    -d '{{\"content\":\"My note\",\"tags\":[\"inbox\"]}}'");
                }
                InboxTokenAction::Revoke { .. } => {
                    session.revoke_inbox_token()
                        .context("Failed to revoke inbox token")?;
                    outln!("✓ {}", t!("inbox.token.revoked"));
                }
                InboxTokenAction::Status { .. } => {
                    let has_token = session.get_inbox_token_status()
                        .context("Failed to check inbox token status")?;
                    if has_token {
                        outln!("✓ {}", t!("inbox.token.active"));
                    } else {
                        outln!("{}", t!("inbox.token.none"));
                    }
                }
            }

            return Ok(());
        }
        Some(Commands::Sync { password }) => {
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let metadata = SyncRepository::new(db.connection()).get_metadata()?;
            match metadata {
                Some(metadata) if metadata.sync_enabled => {
                    outln!("{} {}...", t!("sync.pushing"), metadata.sync_endpoint);

                    // Perform sync using helper function
                    match perform_cli_sync(&db_path, db, key) {
                        Ok(outcome) => report_sync(&outcome),
                        Err(e) => {
                            errln!("✗ {}: {}", t!("sync.failed", error = ""), e);
                            std::process::exit(1);
                        }
                    }
                }
                _ => {
                    outln!("{}", t!("sync.no_api_key"));
                }
            }

            return Ok(());
        }
        Some(Commands::Export { output, password }) => {
            info!("{}: {}", t!("cli.export"), output.display());
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let count = export::export_notes(&db, &key, &output)?;
            outln!("✓ {}: {} {}", t!("cli.export"), count, output.display());
            return Ok(());
        }
        Some(Commands::Import { input, password }) => {
            info!("{}: {}", t!("cli.import"), input.display());
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let summary = export::import_notes(&db, &key, &input)?;
            outln!(
                "✓ {}: {}",
                input.display(),
                t!("cli.import_summary", created = summary.created, updated = summary.updated, skipped = summary.skipped),
            );
            if summary.failed > 0 {
                errln!("⚠ {}", t!("cli.import_failed", count = summary.failed));
            }
            return Ok(());
        }
        Some(Commands::Backup { output, password }) => {
            info!("{}: {}", t!("backup.created"), output.display());
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            outln!("{}", t!("progress.backup"));

            // Create backup with progress indicator
            let backup = backup::create_backup(&db, &key, Some(Box::new(|current, total, item| {
                print!("\r{}: {}/{} ({})", t!("progress.processing"), current, total, item);
                let _ = io::stdout().flush();
            })))?;

            outln!();

            // Write to file
            backup::write_backup(&backup, &output)?;

            outln!("✓ {}: {} ({} {})",
                t!("backup.created"),
                output.display(),
                backup.record_count(),
                t!("backup.records")
            );

            // A backup that's missing something mustn't look like one that isn't
            if backup.skipped > 0 {
                errln!("⚠ {}", t!("backup.incomplete", count = backup.skipped));
                std::process::exit(1);
            }
            return Ok(());
        }
        Some(Commands::RestoreBackup { input, password, key_password }) => {
            info!("{}: {}", t!("backup.restore"), input.display());

            // Validate backup file first
            outln!("{}", t!("backup.validating"));
            let backup = backup::validate_backup(&input)?;

            outln!("✓ {} v{} ({} {})",
                t!("backup.valid"),
                backup.version,
                backup.record_count(),
                t!("backup.records")
            );

            let password = required_password(password)?;
            let key_password = supplied_password(key_password, "JOTTERY_KEY_PASSWORD")?;

            // Use key_password for local database if provided, otherwise use backup password
            let local_password = key_password.as_ref().unwrap_or(&password);

            // Open or create database
            let db = Database::open(&db_path, local_password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Check both passwords before touching anything
            outln!("{}", t!("backup.verifying_password"));
            let keys = backup::restore_keys(&db, &backup, &password, local_password)?;
            outln!("✓ {}", t!("password.correct"));

            outln!("{}", t!("backup.restoring"));

            // Restore with progress indicator
            let restored = backup::restore_backup(&db, &backup, &keys, Some(Box::new(|current, total, item| {
                print!("\r{}: {}/{} ({})", t!("progress.processing"), current, total, item);
                let _ = io::stdout().flush();
            })))?;

            outln!();
            outln!("✓ {}: {} {}, {} {}, {} {}",
                t!("backup.restored"),
                restored.notes, t!("menu.notes"),
                restored.attachments, t!("ui.attachments"),
                restored.versions, t!("ui.version_history"),
            );

            if restored.failed > 0 {
                errln!("⚠ {}", t!("backup.records_failed", count = restored.failed));
                std::process::exit(1);
            }
            return Ok(());
        }
        Some(Commands::StorePassword { password }) => {
            let password = required_password(password)?;

            // Store password for auto-unlock (used for demo/testing automation)
            // First verify the password works with the database
            if db_path.exists() {
                let db = Database::open(&db_path, &password)
                    .context("Password verification failed - incorrect password")?;
                derive_key_from_db(&db, &password)
                    .context("Password verification failed - incorrect password")?;
            }

            // Get the config directory (same as database directory)
            let config_dir = db_path.parent()
                .ok_or_else(|| anyhow::anyhow!("Invalid database path"))?;

            // Store password using file-based storage
            let storage = password_storage::create_file_storage(config_dir);
            storage.store(&password)?;

            outln!("✓ Password stored for auto-unlock");
            outln!("  Database: {}", db_path.display());
            outln!("  Storage: {}", config_dir.join(".jottery_remember").display());
            return Ok(());
        }
        None => {
            // Run interactive TUI
        }
    }

    // Create TUI
    ui::install_panic_hook();
    let mut tui = Tui::new()?;
    tui.enter()?;

    // Create app
    let mut app = App::new(db_path, debug_log)?;
    app.enable_background_sync();

    // Try auto-unlock if password is stored
    if let Ok(true) = app.try_auto_unlock() {
        info!("Auto-unlock successful");
    }

    // Event handler
    let events = EventHandler::default();

    info!("Entering main event loop");

    // Track state variant to force full redraw on major transitions
    let mut last_state = std::mem::discriminant(&app.state);

    // Drawing is the expensive part of this loop, so it waits for a reason
    let mut needs_draw = true;

    // Main loop
    while !app.should_quit() {
        // Force full redraw when the app state variant changes (e.g., Locked→NoteList,
        // NoteList→Settings). This works around terminals that leave stale characters
        // or spacing artifacts across screen transitions.
        let current_state = std::mem::discriminant(&app.state);
        if current_state != last_state {
            app.need_redraw = true;
            last_state = current_state;
        }

        // Check if we need to force a full redraw (e.g., after external editor or state change)
        if app.should_redraw() {
            // Clear ratatui's internal buffer
            tui.clear()?;
            // Also force a screen clear and redraw
            use crossterm::{execute, terminal::{Clear, ClearType}};
            execute!(std::io::stdout(), Clear(ClearType::All))?;
            needs_draw = true;
        }

        // Render
        if needs_draw {
            tui.draw(|frame| app.render(frame))?;
        }

        // Handle events
        match events.next()? {
            ui::Event::Key(key) => {
                needs_draw = true;
                app.register_activity();
                // A failed action is the user's to see, not a reason to quit
                if let Err(e) = app.handle_key(key) {
                    app.error = Some(e.to_string());
                }
            }
            ui::Event::Resize(_, _) => {
                needs_draw = true;
            }
            ui::Event::Tick => {
                // Auto-lock, auto-sync and anything else that runs on a timer
                needs_draw = app.on_tick();
            }
            ui::Event::Mouse(mouse) => {
                needs_draw = true;
                app.register_activity();
                app.handle_mouse(mouse);
            }
        }
    }

    // Clean exit
    tui.exit()?;
    app.wait_for_sync(std::time::Duration::from_secs(5));

    Ok(())
}
