mod crypto;
mod db;
mod export;
mod models;
mod repository;
mod ui;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use tracing::info;

use crypto::CryptoService;
use db::Database;
use ui::{App, EventHandler, Tui};

#[derive(Parser)]
#[command(name = "jottery")]
#[command(about = "A privacy-focused, encrypted note-taking TUI", long_about = None)]
struct Cli {
    /// Database file path
    #[arg(short, long, default_value = "jottery.db")]
    database: PathBuf,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Reset: delete the database and start fresh
    #[arg(long)]
    reset: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
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

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let log_level = if cli.debug { "debug" } else { "info" };
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

    // Create TUI
    let mut tui = Tui::new()?;
    tui.enter()?;

    // Create app
    let mut app = App::new(db_path)?;

    // Event handler
    let events = EventHandler::default();

    // Main loop
    while !app.should_quit() {
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
            ui::Event::Tick | ui::Event::Mouse(_) => {
                // Ignore for now
            }
        }
    }

    // Clean exit
    tui.exit()?;

    Ok(())
}
