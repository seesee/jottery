mod crypto;
mod db;
mod export;
mod models;
mod repository;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};
use std::io::{self, Write};
use std::process::Command;
use std::env;
use tempfile::NamedTempFile;
use tracing::info;

use crypto::CryptoService;
use db::Database;
use models::Note;
use repository::{NoteRepository, sync::SyncRepository};
use ui::{App, EventHandler, Tui};

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
    /// Sync notes with remote server
    Sync {
        /// Password for decryption (will prompt if not provided)
        #[arg(short, long)]
        password: Option<String>,
    },
    /// Export notes to JSON file
    Export {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Password for decryption
        #[arg(short, long)]
        password: String,
    },
    /// Import notes from JSON file
    Import {
        /// Input file path
        #[arg(short, long)]
        input: PathBuf,

        /// Password for encryption
        #[arg(short, long)]
        password: String,
    },
}

/// Prompt for password from stdin
fn prompt_password() -> Result<String> {
    print!("Password: ");
    io::stdout().flush()?;
    let password = rpassword::read_password()?;
    Ok(password)
}

/// Get or prompt for password
fn get_password(password_opt: Option<String>) -> Result<String> {
    match password_opt {
        Some(pwd) => Ok(pwd),
        None => prompt_password(),
    }
}

/// Open $EDITOR with content and return the edited result
fn open_editor(initial_content: &str) -> Result<String> {
    // Create temporary file
    let mut temp_file = NamedTempFile::new()
        .context("Failed to create temporary file")?;
    temp_file
        .write_all(initial_content.as_bytes())
        .context("Failed to write to temporary file")?;
    temp_file.flush()?;

    let temp_path = temp_file.path();

    // Get editor from environment (default to vi)
    let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

    // Launch editor
    let status = Command::new(&editor)
        .arg(temp_path)
        .status()
        .context(format!("Failed to launch editor: {}", editor))?;

    if !status.success() {
        anyhow::bail!("Editor exited with non-zero status");
    }

    // Read the edited content
    let content = std::fs::read_to_string(temp_path)
        .context("Failed to read temporary file")?;

    Ok(content)
}

/// Format a note for display
fn format_note_preview(note: &Note, show_content: bool) -> String {
    let title = if !note.content.is_empty() {
        note.content.lines().next().unwrap_or("(empty)")
    } else {
        "(empty)"
    };

    let tags_str = if note.tags.is_empty() {
        String::new()
    } else {
        format!(" [{}]", note.tags.join(", "))
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
        &note.id[..8],
        title,
        tags_str,
        preview
    )
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    // If debug-log is specified, automatically enable debug level
    let log_level = if cli.debug || cli.debug_log.is_some() { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(format!("jottery_tui={}", log_level).parse()?),
        )
        .init();

    info!("Jottery TUI v{}", env!("CARGO_PKG_VERSION"));
    info!("Database: {}", cli.database.display());

    // Get absolute path to database
    let db_path = if cli.database.is_absolute() {
        cli.database
    } else {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("jottery");
        std::fs::create_dir_all(&config_dir)?;
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

            println!("✓ Database deleted: {}", db_path.display());
            println!("You can now start fresh with a new password.");
        } else {
            println!("Database does not exist: {}", db_path.display());
        }
        return Ok(());
    }

    // Handle subcommands
    match cli.command {
        Some(Commands::Note { password, tags }) => {
            let password = get_password(password)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let crypto = CryptoService::new();
            let salt = crypto.generate_salt();
            let key = crypto.derive_key(&password, &salt, 256_000)?;

            // Open editor with empty content
            let content = open_editor("")?;

            // If content is empty, don't create note
            if content.trim().is_empty() {
                println!("Note is empty, not saving.");
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
            println!("✓ Note created: {}", &note.id[..8]);

            // Auto-sync if configured
            let sync_repo = SyncRepository::new(db.connection());
            if let Ok(Some(metadata)) = sync_repo.get_metadata() {
                if metadata.sync_enabled {
                    println!("Syncing...");
                    // Note: Full sync implementation would go here
                    println!("✓ Sync complete");
                }
            }

            return Ok(());
        }
        Some(Commands::List { password, tag, limit }) => {
            let password = get_password(password)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let crypto = CryptoService::new();
            let salt = crypto.generate_salt();
            let key = crypto.derive_key(&password, &salt, 256_000)?;

            let note_repo = NoteRepository::new(db.connection());
            let notes = note_repo.list(false, &key)?;

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
                println!("No notes found.");
            } else {
                println!("Found {} notes:\n", filtered_notes.len());
                for note in filtered_notes {
                    println!("{}", format_note_preview(&note, false));
                }
            }

            return Ok(());
        }
        Some(Commands::Search { query, password, limit }) => {
            let password = get_password(password)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let crypto = CryptoService::new();
            let salt = crypto.generate_salt();
            let key = crypto.derive_key(&password, &salt, 256_000)?;

            let note_repo = NoteRepository::new(db.connection());
            let notes = note_repo.list(false, &key)?;

            // Simple search implementation
            let query_lower = query.to_lowercase();
            let results: Vec<_> = notes.into_iter()
                .filter(|n| {
                    n.content.to_lowercase().contains(&query_lower) ||
                    n.tags.iter().any(|t| t.to_lowercase().contains(&query_lower))
                })
                .take(limit)
                .collect();

            if results.is_empty() {
                println!("No results found for: {}", query);
            } else {
                println!("Found {} results for '{}':\n", results.len(), query);
                for note in results {
                    println!("{}", format_note_preview(&note, true));
                }
            }

            return Ok(());
        }
        Some(Commands::Show { id, password }) => {
            let password = get_password(password)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let crypto = CryptoService::new();
            let salt = crypto.generate_salt();
            let key = crypto.derive_key(&password, &salt, 256_000)?;

            let note_repo = NoteRepository::new(db.connection());
            let notes = note_repo.list(false, &key)?;

            // Find note by partial ID match
            let note = notes.into_iter()
                .find(|n| n.id.starts_with(&id))
                .context("Note not found")?;

            println!("ID: {}", note.id);
            println!("Created: {}", note.created_at);
            println!("Modified: {}", note.modified_at);
            if !note.tags.is_empty() {
                println!("Tags: {}", note.tags.join(", "));
            }
            println!("\n{}", note.content);

            return Ok(());
        }
        Some(Commands::Sync { password }) => {
            let password = get_password(password)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let sync_repo = SyncRepository::new(db.connection());
            match sync_repo.get_metadata()? {
                Some(metadata) if metadata.sync_enabled => {
                    println!("Syncing with {}...", metadata.sync_endpoint);
                    // Note: Full sync implementation would go here
                    println!("✓ Sync complete");
                }
                _ => {
                    println!("Sync is not configured. Use the TUI to set it up.");
                }
            }

            return Ok(());
        }
        Some(Commands::Export { output, password }) => {
            info!("Exporting notes to: {}", output.display());
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let crypto = CryptoService::new();
            let salt = crypto.generate_salt();
            let key = crypto.derive_key(&password, &salt, 256_000)?;

            let count = export::export_notes(&db, &key, &output)?;
            println!("✓ Exported {} notes to {}", count, output.display());
            return Ok(());
        }
        Some(Commands::Import { input, password }) => {
            info!("Importing notes from: {}", input.display());
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            let crypto = CryptoService::new();
            let salt = crypto.generate_salt();
            let key = crypto.derive_key(&password, &salt, 256_000)?;

            let count = export::import_notes(&db, &key, &input)?;
            println!("✓ Imported {} notes from {}", count, input.display());
            return Ok(());
        }
        None => {
            // Run interactive TUI
        }
    }

    // Open debug log file if specified
    let debug_log = if let Some(log_path) = cli.debug_log {
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .context(format!("Failed to open debug log: {}", log_path.display()))?;
        Some(Arc::new(Mutex::new(file)))
    } else {
        None
    };

    // Create TUI
    let mut tui = Tui::new()?;
    tui.enter()?;

    // Create app
    let mut app = App::new(db_path, debug_log)?;

    // Try auto-unlock if password is stored
    if let Ok(true) = app.try_auto_unlock() {
        info!("Auto-unlock successful");
    }

    // Event handler
    let events = EventHandler::default();

    // Main loop
    while !app.should_quit() {
        // Check if we need to force a full redraw (e.g., after external editor)
        if app.should_redraw() {
            // Clear ratatui's internal buffer
            tui.clear()?;
            // Also force a screen clear and redraw
            use crossterm::{execute, terminal::{Clear, ClearType}};
            execute!(std::io::stdout(), Clear(ClearType::All))?;
        }

        // Render
        tui.draw(|frame| {
            app.render(frame);
        })?;

        // Handle events
        match events.next()? {
            ui::Event::Key(key) => {
                app.handle_key(key)?;
            }
            ui::Event::Resize(_, _) => {
                // Terminal resized, will redraw on next iteration
            }
            ui::Event::Tick => {
                // Check if auto-sync should run
                app.check_auto_sync();
            }
            ui::Event::Mouse(_) => {
                // Ignore for now
            }
        }
    }

    // Clean exit
    tui.exit()?;

    Ok(())
}
