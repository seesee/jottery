/// Data models for Jottery TUI
/// Matches web app schema (src/lib/types/models.ts)

pub mod note;
pub mod settings;
pub mod encryption;
pub mod sync;

pub use note::*;
pub use settings::*;
