mod crypto;
mod db;
mod models;
mod repository;
mod ui;

use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use tracing::info;

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
