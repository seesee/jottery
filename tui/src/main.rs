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
use repository::{NoteRepository, sync::SyncRepository, EncryptionRepository};
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

/// Try to load stored password from remember file
fn try_load_stored_password(db_path: &PathBuf) -> Result<Option<String>> {
    let config_dir = db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
    let remember_file = config_dir.join(".jottery_remember");

    if !remember_file.exists() {
        return Ok(None);
    }

    let encrypted_password = std::fs::read_to_string(&remember_file)
        .context("Failed to read stored password file")?;

    if encrypted_password.trim().is_empty() {
        return Ok(None);
    }

    // Decrypt using device-specific constant key (same as TUI)
    use crypto::EncryptedData;
    let crypto = CryptoService::new();

    // Get device key (must match the one in app.rs)
    let device_key = get_device_key();

    let encrypted_data: EncryptedData = serde_json::from_str(&encrypted_password)
        .context("Failed to parse stored password")?;
    let password = crypto.decrypt_text(&encrypted_data, &device_key)
        .context("Failed to decrypt stored password")?;

    Ok(Some(password))
}

/// Get device-specific encryption key for stored password
/// Must match the implementation in ui/app.rs
fn get_device_key() -> [u8; 32] {
    // Must exactly match the constant in ui/app.rs
    let constant = b"jottery-tui-device-key-v1.0.0---";
    let mut key = [0u8; 32];
    key.copy_from_slice(&constant[..32]);
    key
}

/// Get or prompt for password, checking stored password first
fn get_password(password_opt: Option<String>, db_path: &PathBuf) -> Result<String> {
    match password_opt {
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

/// Derive encryption key from password using stored salt
fn derive_key_from_db(db: &Database, password: &str) -> Result<[u8; 32]> {
    let encryption_repo = EncryptionRepository::new(db.connection());
    let crypto = CryptoService::new();

    // Get stored salt and iterations from database
    let (salt, iterations) = if let Some(metadata) = encryption_repo.get()? {
        (metadata.salt, metadata.iterations)
    } else {
        // First-time setup: generate new salt and save it
        let new_salt = crypto.generate_salt();
        let iterations = 256_000;
        encryption_repo.save(&new_salt, iterations)?;
        (new_salt.to_vec(), iterations)
    };

    // Derive key using stored salt
    crypto.derive_key(password, &salt, iterations)
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

/// Perform sync from CLI (simplified version of TUI sync)
fn perform_cli_sync(db: &Database, key: &[u8; 32], mut metadata: models::sync::SyncMetadata) -> Result<usize> {
    use models::sync::*;
    use repository::sync::SyncRepository;
    use chrono::Utc;
    use sha2::{Sha256, Digest};

    let crypto = CryptoService::new();
    let sync_repo = SyncRepository::new(db.connection());
    let note_repo = NoteRepository::new(db.connection());

    // Get and decrypt API key
    let encrypted_api_key = metadata.api_key.as_ref()
        .ok_or_else(|| anyhow::anyhow!("No API key configured"))?;
    let api_key_encrypted: crypto::EncryptedData = serde_json::from_str(encrypted_api_key)?;
    let api_key = crypto.decrypt_text(&api_key_encrypted, key)?;

    let endpoint = metadata.sync_endpoint.clone();

    // PUSH: Get notes modified since last sync
    let notes_to_push = if let Some(last_sync) = metadata.last_sync_at {
        note_repo.get_modified_after(last_sync, key)?
    } else {
        note_repo.list(false, key)?
    };

    let sync_count = notes_to_push.len();

    if !notes_to_push.is_empty() {
        // Convert notes to sync format
        let sync_notes: Result<Vec<SyncNote>> = notes_to_push.iter().map(|note| {
            let encrypted_content = crypto.encrypt_text(&note.content, key)?;
            let content_json = serde_json::to_string(&encrypted_content)?;

            let encrypted_tags: Result<Vec<String>> = note.tags.iter()
                .map(|tag| {
                    let tag_json = serde_json::to_string(tag)?;
                    let encrypted_tag = crypto.encrypt_text(&tag_json, key)?;
                    Ok(serde_json::to_string(&encrypted_tag)?)
                })
                .collect();

            Ok(SyncNote {
                id: note.id.clone(),
                created_at: note.created_at,
                modified_at: note.modified_at,
                content: content_json,
                tags: encrypted_tags?,
                attachments: vec![], // Simplified: skip attachments for CLI
                pinned: note.pinned,
                deleted: note.deleted,
                deleted_at: note.deleted_at,
                version: note.version,
                word_wrap: Some(note.word_wrap),
                syntax_language: Some(note.syntax_language.to_string()),
            })
        }).collect();

        let push_request = SyncPushRequest {
            notes: sync_notes?,
            attachments: vec![],
            versions: vec![],
        };

        // Send push request
        let client = reqwest::blocking::Client::new();
        let push_url = format!("{}/api/v1/sync/push", endpoint);
        let response = client
            .post(&push_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&push_request)
            .send()
            .context("Failed to send push request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Push failed: {} - {}", status, error_text);
        }

        let push_response: SyncPushResponse = response.json()
            .context("Failed to parse push response")?;

        // Update sync metadata for accepted notes
        let now = Utc::now();
        for accepted in push_response.accepted {
            // Compute sync hash using SHA-256
            let mut hasher = Sha256::new();
            hasher.update(accepted.id.as_bytes());
            let sync_hash = format!("{:x}", hasher.finalize());

            let note_metadata = NoteSyncMetadata {
                note_id: accepted.id.clone(),
                synced_at: accepted.synced_at,
                sync_hash,
                server_version: accepted.server_version,
                last_sync_status: SyncStatus::Synced,
                error_message: None,
            };
            sync_repo.update_note_metadata(&note_metadata)?;
        }

        metadata.last_push_at = Some(now);
        metadata.last_sync_at = Some(now);
        sync_repo.update_metadata(&metadata)?;
    }

    // PULL: Simplified - just update sync timestamp
    // Full pull implementation would fetch from server
    let now = Utc::now();
    metadata.last_pull_at = Some(now);
    sync_repo.update_metadata(&metadata)?;

    Ok(sync_count)
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
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

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
                    println!("Syncing to {}...", metadata.sync_endpoint);
                    match perform_cli_sync(&db, &key, metadata) {
                        Ok(count) => {
                            println!("✓ Synced {} note(s) to server", count);
                        }
                        Err(e) => {
                            eprintln!("⚠ Sync failed: {}", e);
                            eprintln!("  Note saved locally. Run 'jottery sync' to retry.");
                        }
                    }
                }
            }

            return Ok(());
        }
        Some(Commands::List { password, tag, limit }) => {
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

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
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

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
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

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
            let password = get_password(password, &db_path)?;
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let sync_repo = SyncRepository::new(db.connection());
            match sync_repo.get_metadata()? {
                Some(metadata) if metadata.sync_enabled => {
                    println!("Syncing with {}...", metadata.sync_endpoint);

                    // Perform sync using helper function
                    match perform_cli_sync(&db, &key, metadata) {
                        Ok(count) => {
                            println!("✓ Sync complete - {} notes synced", count);
                        }
                        Err(e) => {
                            eprintln!("✗ Sync failed: {}", e);
                            std::process::exit(1);
                        }
                    }
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

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let count = export::export_notes(&db, &key, &output)?;
            println!("✓ Exported {} notes to {}", count, output.display());
            return Ok(());
        }
        Some(Commands::Import { input, password }) => {
            info!("Importing notes from: {}", input.display());
            let db = Database::open(&db_path, &password)
                .context("Failed to open database. Check your password.")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

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
