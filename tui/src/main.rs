mod api;
mod backup;
mod crypto;
mod db;
mod export;
mod models;
mod password_storage;
mod repository;
mod sse;
mod ui;

// Initialize rust-i18n with fallback to en-GB
rust_i18n::i18n!("locales", fallback = "en-GB");

use anyhow::{Context, Result};
use rust_i18n::t;
use clap::{Parser, Subcommand};
use std::path::{Path, PathBuf};
use std::fs::OpenOptions;
use std::sync::{Arc, Mutex};
use std::io::{self, Read, Write};
use std::process::Command;
use std::env;
use tempfile::NamedTempFile;
use tracing::info;

use crypto::CryptoService;
use db::Database;
use models::Note;
use repository::{NoteRepository, sync::SyncRepository, EncryptionRepository};
use ui::{App, EventHandler, Tui};

/// Detect and set locale from environment
fn detect_locale() {
    if let Ok(lang) = env::var("LANG") {
        // LANG is typically in format like "en_GB.UTF-8" or "fr_FR.UTF-8"
        // Extract just the locale part (before the dot)
        let locale_part = lang.split('.').next().unwrap_or("en-GB");

        // For most languages, use just the language code (fr, es, de, etc.)
        // For en_GB, keep the full locale code as en-GB
        let locale = if locale_part.starts_with("en_GB") || locale_part.starts_with("en-GB") {
            "en-GB"
        } else {
            // Extract just the language code (first part before _ or -)
            locale_part.split('_').next()
                .or_else(|| locale_part.split('-').next())
                .unwrap_or("en-GB")
        };

        rust_i18n::set_locale(locale);
    }
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
    /// Create encrypted backup (compatible with web app)
    Backup {
        /// Output file path
        #[arg(short, long)]
        output: PathBuf,

        /// Password for encryption
        #[arg(short, long)]
        password: String,
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

        /// Password for decryption (backup password)
        #[arg(short, long)]
        password: String,

        /// Local database encryption password (defaults to backup password)
        #[arg(short = 'k', long)]
        key_password: Option<String>,
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

/// Prompt for password from stdin
fn prompt_password() -> Result<String> {
    print!("{}", t!("password.prompt"));
    io::stdout().flush()?;
    let password = rpassword::read_password()?;
    Ok(password)
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
        .context(t!("editor.temp_file_error"))?;
    temp_file
        .write_all(initial_content.as_bytes())
        .context(t!("editor.temp_write_error"))?;
    temp_file.flush()?;

    let temp_path = temp_file.path();

    // Get editor from environment (default to vi)
    let editor = env::var("EDITOR").unwrap_or_else(|_| t!("editor.default_editor").to_string());

    // Launch editor
    let status = Command::new(&editor)
        .arg(temp_path)
        .status()
        .context(t!("editor.external_failed", error = &editor))?;

    if !status.success() {
        anyhow::bail!("{}", t!("editor.external_exit_error"));
    }

    // Read the edited content
    let content = std::fs::read_to_string(temp_path)
        .context(t!("editor.temp_read_error"))?;

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
                archived: note.archived,
                archived_at: note.archived_at,
                locked: note.locked,
                locked_at: note.locked_at,
                deleted: note.deleted,
                deleted_at: note.deleted_at,
                version: note.version,
                word_wrap: Some(note.word_wrap),
                syntax_language: Some(note.syntax_language.to_string()),
                color: note.color.clone(),
                // Hash chain fields for git-like conflict detection
                content_hash: note.content_hash.clone(),
                parent_hash: note.parent_hash.clone(),
                hash_chain: note.hash_chain.clone(),
            })
        }).collect();

        let push_request = SyncPushRequest {
            notes: sync_notes?,
            attachments: vec![],
            versions: vec![],
            deletions: vec![],
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
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

            // Provide user-friendly error messages
            let error_msg = if status == 403 {
                t!("sync.account_deactivated").to_string()
            } else if status == 401 {
                t!("sync.invalid_key").to_string()
            } else if status == 409 {
                t!("sync.conflict").to_string()
            } else {
                error_body
            };

            anyhow::bail!("{}: {}", t!("sync.failed", error = ""), error_msg);
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
                conflict_data: None,
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
    // Detect and set locale from environment
    detect_locale();

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

            println!("✓ {}: {}", t!("note.deleted"), db_path.display());
            println!("{}", t!("password.change_success"));
        } else {
            println!("{}: {}", t!("error.file_not_found"), db_path.display());
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
                println!("{}", t!("note.save_failed", error = t!("password.empty_error")));
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
            println!("✓ {}: {}", t!("cli.new"), &note.id[..8]);

            // Auto-sync if configured
            let sync_repo = SyncRepository::new(db.connection());
            if let Ok(Some(metadata)) = sync_repo.get_metadata() {
                if metadata.sync_enabled {
                    println!("{} {}...", t!("sync.pushing"), metadata.sync_endpoint);
                    match perform_cli_sync(&db, &key, metadata) {
                        Ok(count) => {
                            println!("✓ {}: {}", t!("sync.success"), count);
                        }
                        Err(e) => {
                            eprintln!("⚠ {}: {}", t!("sync.failed", error = ""), e);
                            eprintln!("  {}", t!("sync.request_failed"));
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
                println!("{}", t!("note.no_notes"));
            } else {
                println!("{}\n", t!("note.total", count = filtered_notes.len()));
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
                println!("{}: {}", t!("search.no_results"), query);
            } else {
                println!("{}: '{}'\n", t!("search.results", count = results.len()), query);
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
                .context(t!("error.file_not_found"))?;

            println!("ID: {}", note.id);
            println!("{}: {}", t!("editor.created"), note.created_at);
            println!("{}: {}", t!("editor.modified"), note.modified_at);
            if !note.tags.is_empty() {
                println!("{}: {}", t!("editor.tags"), note.tags.join(", "));
            }
            println!("\n{}", note.content);

            return Ok(());
        }
        Some(Commands::RegisterUser { server, email, password }) => {
            use api::AuthClient;

            // Get password
            let password = match password {
                Some(pwd) => pwd,
                None => {
                    print!("{}", t!("password.prompt"));
                    io::stdout().flush()?;
                    rpassword::read_password()?
                }
            };

            // Validate password strength
            if password.len() < 12 {
                anyhow::bail!("{}", t!("password.empty_error"));
            }

            println!("{} {} {}...", t!("cli.register"), email, server);

            // Create auth client and register
            let client = AuthClient::new(server.clone());

            // Check server connectivity first
            if !client.health_check()? {
                anyhow::bail!("{} {}", t!("error.network", error = ""), server);
            }

            let response = client.register_user(&email, &password)
                .context(t!("sync.request_failed"))?;

            println!("\n✓ {}", t!("sync.success"));
            // lgtm[rust/clear-text-logging] - user_id is a non-sensitive UUID identifier
            println!("User ID: {}", response.user_id);
            println!("Email: {}", response.email);
            println!("{}: {}", t!("status.synced"), response.status);
            println!("\n{}", response.message);

            if response.status == "pending_approval" {
                println!("\n{}:", t!("help.general"));
                println!("1. {}", t!("sync.account_deactivated"));
                println!("2. {}", t!("cli.register"));
            }

            return Ok(());
        }
        Some(Commands::RegisterDevice { server, email, password, device_name, key_password }) => {
            use api::AuthClient;

            // Get user password
            let password = match password {
                Some(pwd) => pwd,
                None => {
                    print!("{}", t!("password.prompt"));
                    io::stdout().flush()?;
                    rpassword::read_password()?
                }
            };

            // Get encryption password for local database
            let key_password = match key_password {
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

            println!("{} '{}' {}...", t!("cli.register"), device_name, email);

            // Create auth client and register device
            let client = AuthClient::new(server.clone());

            // Check server connectivity first
            if !client.health_check()? {
                anyhow::bail!("{} {}", t!("error.network", error = ""), server);
            }

            let response = client.register_device(&email, &password, &device_name, "tui")
                .context(t!("sync.request_failed"))?;

            println!("\n✓ {}", t!("sync.success"));
            println!("Client ID: {}", response.client_id);
            // lgtm[rust/clear-text-logging] - user_id is a non-sensitive UUID identifier
            println!("User ID: {}", response.user_id);
            println!("{}: {}", t!("settings.device_name"), response.device_name);

            // Initialize database with encryption password
            let db = Database::open(&db_path, &key_password)
                .context("Failed to create database")?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &key_password)?;

            // Encrypt and store API key
            let crypto = CryptoService::new();
            let encrypted_api_key = crypto.encrypt_text(&response.api_key, &key)?;
            let api_key_json = serde_json::to_string(&encrypted_api_key)?;

            // Store sync configuration
            let sync_repo = SyncRepository::new(db.connection());
            let sync_metadata = models::sync::SyncMetadata {
                last_sync_at: None,
                last_push_at: None,
                last_pull_at: None,
                api_key: Some(api_key_json),
                client_id: Some(response.client_id),
                user_email: Some(email.clone()),
                sync_enabled: true,
                sync_endpoint: server.clone(),
                auto_sync_interval: Some(1),
                pending_registration_email: None,
                pending_device_name: None,
            };

            sync_repo.update_metadata(&sync_metadata)?;

            println!("\n✓ {}", t!("sync.success"));
            println!("\n{}:", t!("help.general"));
            println!("1. {}: jottery note", t!("cli.new"));
            println!("2. {}: jottery sync", t!("cli.sync"));
            println!("3. {}: jottery", t!("menu.notes"));

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

            let password = match password {
                Some(pwd) => pwd,
                None => {
                    print!("{}", t!("password.prompt"));
                    io::stdout().flush()?;
                    rpassword::read_password()?
                }
            };

            // Login to get session
            println!("{}", t!("inbox.token.logging_in"));
            let session = SessionClient::login(&server, &email, &password)
                .context("Failed to login to server")?;

            match action {
                InboxTokenAction::Generate { .. } => {
                    let token = session.generate_inbox_token()
                        .context("Failed to generate inbox token")?;
                    println!("\n{}", t!("inbox.token.generated_success"));
                    println!("\n  {}\n", token);
                    println!("{}", t!("inbox.token.save_warning"));
                    println!("\n{}:", t!("inbox.token.example_title"));
                    println!("  curl -X POST {}/api/v1/inbox \\", server);
                    println!("    -H \"Authorization: Bearer {}\" \\", token);
                    println!("    -H \"Content-Type: application/json\" \\");
                    println!("    -d '{{\"content\":\"My note\",\"tags\":[\"inbox\"]}}'");
                }
                InboxTokenAction::Revoke { .. } => {
                    session.revoke_inbox_token()
                        .context("Failed to revoke inbox token")?;
                    println!("✓ {}", t!("inbox.token.revoked"));
                }
                InboxTokenAction::Status { .. } => {
                    let has_token = session.get_inbox_token_status()
                        .context("Failed to check inbox token status")?;
                    if has_token {
                        println!("✓ {}", t!("inbox.token.active"));
                    } else {
                        println!("{}", t!("inbox.token.none"));
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

            let sync_repo = SyncRepository::new(db.connection());
            match sync_repo.get_metadata()? {
                Some(metadata) if metadata.sync_enabled => {
                    println!("{} {}...", t!("sync.pushing"), metadata.sync_endpoint);

                    // Perform sync using helper function
                    match perform_cli_sync(&db, &key, metadata) {
                        Ok(count) => {
                            println!("✓ {}: {}", t!("sync.success"), count);
                        }
                        Err(e) => {
                            eprintln!("✗ {}: {}", t!("sync.failed", error = ""), e);
                            std::process::exit(1);
                        }
                    }
                }
                _ => {
                    println!("{}", t!("sync.no_api_key"));
                }
            }

            return Ok(());
        }
        Some(Commands::Export { output, password }) => {
            info!("{}: {}", t!("cli.export"), output.display());
            let db = Database::open(&db_path, &password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let count = export::export_notes(&db, &key, &output)?;
            println!("✓ {}: {} {}", t!("cli.export"), count, output.display());
            return Ok(());
        }
        Some(Commands::Import { input, password }) => {
            info!("{}: {}", t!("cli.import"), input.display());
            let db = Database::open(&db_path, &password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            let count = export::import_notes(&db, &key, &input)?;
            println!("✓ {}: {} {}", t!("cli.import"), count, input.display());
            return Ok(());
        }
        Some(Commands::Backup { output, password }) => {
            info!("{}: {}", t!("backup.created"), output.display());
            let db = Database::open(&db_path, &password)
                .context(t!("password.unlock_failed", error = ""))?;

            // Derive key using stored salt
            let key = derive_key_from_db(&db, &password)?;

            println!("{}", t!("progress.backup"));

            // Create backup with progress indicator
            let backup = backup::create_backup(&db, &key, Some(Box::new(|current, total, item| {
                print!("\r{}: {}/{} ({})", t!("progress.processing"), current, total, item);
                let _ = io::stdout().flush();
            })))?;

            println!();

            // Write to file
            backup::write_backup(&backup, &output)?;

            println!("✓ {}: {} ({} {})",
                t!("backup.created"),
                output.display(),
                backup.data.len(),
                t!("backup.records")
            );
            return Ok(());
        }
        Some(Commands::RestoreBackup { input, password, key_password }) => {
            info!("{}: {}", t!("backup.restore"), input.display());

            // Validate backup file first
            println!("{}", t!("backup.validating"));
            let backup = backup::validate_backup(&input)?;

            println!("✓ {} v{} ({} {})",
                t!("backup.valid"),
                backup.version,
                backup.data.len(),
                t!("backup.records")
            );

            // Verify password
            println!("{}", t!("backup.verifying_password"));
            let key = backup::verify_backup_password(&backup, &password)?;
            println!("✓ {}", t!("password.correct"));

            // Use key_password for local database if provided, otherwise use backup password
            let local_password = key_password.as_ref().unwrap_or(&password);

            // Open or create database
            let db = Database::open(&db_path, local_password)
                .context(t!("password.unlock_failed", error = ""))?;

            println!("{}", t!("backup.restoring"));

            // Restore with progress indicator
            let restored = backup::restore_backup(&db, &backup, &key, Some(Box::new(|current, total, item| {
                print!("\r{}: {}/{} ({})", t!("progress.processing"), current, total, item);
                let _ = io::stdout().flush();
            })))?;

            println!();
            println!("✓ {}: {} {}", t!("backup.restored"), restored, t!("menu.notes"));
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

    info!("Entering main event loop");

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
        info!("About to render frame");
        tui.draw(|frame| {
            info!("Inside draw closure, calling app.render");
            app.render(frame);
            info!("app.render completed");
        })?;
        info!("Frame rendered successfully");

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
            ui::Event::Mouse(mouse) => {
                use crossterm::event::{MouseEventKind, MouseButton};
                use ui::state::{AppState, ViewMode, InputMode, FocusedPanel};

                match mouse.kind {
                    MouseEventKind::ScrollUp => {
                        // Scroll preview up 3 lines
                        app.preview_scroll_offset = app.preview_scroll_offset.saturating_sub(3);
                    }
                    MouseEventKind::ScrollDown => {
                        // Scroll preview down 3 lines
                        app.preview_scroll_offset = app.preview_scroll_offset.saturating_add(3);
                    }
                    MouseEventKind::Down(MouseButton::Left) => {
                        // Only handle clicks in NoteList state
                        if !matches!(app.state, AppState::NoteList) {
                            continue;
                        }
                        if !matches!(app.view_mode, ViewMode::NoteList) {
                            continue;
                        }
                        if app.search_active {
                            // Clicking outside search exits search mode
                            app.search_active = false;
                        }

                        let x = mouse.column;
                        let y = mouse.row;

                        // Check if click is in note list area
                        if let Some(area) = app.note_list_area {
                            if x >= area.x && x < area.x + area.width
                                && y >= area.y && y < area.y + area.height
                            {
                                // Calculate which note was clicked
                                let clicked_index = (y - area.y) as usize;
                                let filtered_count = app.filtered_notes().len();
                                if clicked_index < filtered_count {
                                    if app.is_multi_select_mode {
                                        // In multi-select mode, toggle selection
                                        app.toggle_note_selection(clicked_index);
                                    } else {
                                        // Normal mode: select the note
                                        app.selected_note = clicked_index;
                                        app.selected_note_id = app.filtered_notes().get(clicked_index).map(|n| n.id.clone());
                                        app.preview_scroll_offset = 0;
                                        app.focused_panel = FocusedPanel::NoteContent;
                                    }
                                }
                                continue;
                            }
                        }

                        // Check if click is on tags line (for entering tag mode)
                        if let Some(area) = app.tags_line_area {
                            if y == area.y && x >= area.x && x < area.x + area.width {
                                // Check if click is on a specific tag (for search)
                                let mut found_tag = None;
                                for (tag_name, tag_start, tag_end) in &app.tag_positions {
                                    if x >= *tag_start && x < *tag_end {
                                        found_tag = Some(tag_name.clone());
                                        break;
                                    }
                                }

                                if let Some(tag) = found_tag {
                                    // Click on a tag: search for it
                                    app.search_input = format!("#{}", tag);
                                    app.search_active = true;
                                    app.selected_note = 0;
                                } else {
                                    // Click on "Tags:" label or empty area: enter tag mode
                                    let filtered = app.filtered_notes();
                                    if !filtered.is_empty() && app.selected_note < filtered.len() {
                                        let note = &filtered[app.selected_note];
                                        let note_id = note.id.clone();
                                        let content = note.content.clone();
                                        let syntax_lang = note.syntax_language;
                                        let tags = note.tags.clone();

                                        app.note_input = content;
                                        app.note_syntax = syntax_lang;
                                        app.current_tags = tags;
                                        app.editing_note_id = Some(note_id);
                                        app.state = AppState::NoteView;
                                        app.input_mode = InputMode::Tag;
                                        app.tag_input.clear();
                                    }
                                }
                                continue;
                            }
                        }

                        // Check if click is on a note link in the preview area
                        if let Some(_preview_area) = app.preview_area {
                            let mut found_link = None;
                            for (note_id, link_y, link_x_start, link_x_end) in &app.note_link_positions {
                                if y == *link_y && x >= *link_x_start && x < *link_x_end {
                                    found_link = Some(note_id.clone());
                                    break;
                                }
                            }

                            if let Some(target_note_id) = found_link {
                                // Navigate to the linked note
                                // Find the note in the full notes list and update selection
                                let notes = app.filtered_notes();
                                if let Some(idx) = notes.iter().position(|n| n.id == target_note_id) {
                                    app.selected_note = idx;
                                    app.selected_note_id = Some(target_note_id);
                                    app.preview_scroll_offset = 0;
                                    app.focused_panel = FocusedPanel::NoteContent;
                                } else {
                                    // Note not in current filter, clear search and try again
                                    app.search_input.clear();
                                    app.search_active = false;
                                    let all_notes = app.filtered_notes();
                                    if let Some(idx) = all_notes.iter().position(|n| n.id == target_note_id) {
                                        app.selected_note = idx;
                                        app.selected_note_id = Some(target_note_id);
                                        app.preview_scroll_offset = 0;
                                        app.focused_panel = FocusedPanel::NoteContent;
                                    }
                                }
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Clean exit
    tui.exit()?;

    Ok(())
}
