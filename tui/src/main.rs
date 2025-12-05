mod crypto;
mod db;
mod models;

use anyhow::Result;
use tracing::info;

fn main() -> Result<()> {
    // Initialize logging
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("jottery_tui=debug".parse()?),
        )
        .init();

    info!("Jottery TUI v{}", env!("CARGO_PKG_VERSION"));
    info!("Starting application...");

    println!("Welcome to Jottery TUI!");
    println!("Database layer: ✓");
    println!("Data models: ✓");
    println!("Encryption layer: ✓");
    println!("This is a placeholder - TUI interface coming soon...");

    Ok(())
}
