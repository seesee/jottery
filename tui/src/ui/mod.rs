/// UI module for terminal interface
/// Built with ratatui

pub mod app;
pub mod color_scheme;
pub mod event;
pub mod helpers;
pub mod state;
pub mod syntax;
pub mod terminal;

pub use app::App;
pub use color_scheme::ColorScheme;
pub use event::{Event, EventHandler};
pub use state::{AppState, InputMode, ViewMode};
pub use terminal::Tui;
