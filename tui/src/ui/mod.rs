/// UI module for terminal interface
/// Built with ratatui

pub mod app;
pub mod color_scheme;
pub mod event;
pub mod syntax;
pub mod terminal;

pub use app::App;
pub use color_scheme::ColorScheme;
pub use event::{Event, EventHandler};
pub use terminal::Tui;
