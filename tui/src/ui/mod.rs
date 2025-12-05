/// UI module for terminal interface
/// Built with ratatui

pub mod app;
pub mod event;
pub mod terminal;

pub use app::App;
pub use event::{Event, EventHandler};
pub use terminal::Tui;
