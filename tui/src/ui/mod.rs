//! UI module for terminal interface
//!
//! This module contains all the TUI (Terminal User Interface) components
//! for the Jottery application, built with ratatui.
//!
//! # Architecture
//!
//! The UI is organized into several submodules:
//!
//! - [`app`] - Core application state and coordination
//! - [`app_state`] - Nested state structs for organizing App fields
//! - [`state`] - State machine types (AppState, InputMode, ViewMode)
//! - [`rendering`] - UI rendering functions for each screen
//! - [`operations`] - Business logic operations
//! - [`input`] - Keyboard input handlers
//! - [`helpers`] - Markdown processing utilities
//! - [`color_scheme`] - Color scheme definitions
//! - [`syntax`] - Syntax highlighting
//! - [`calc`] - Calculator expression evaluator
//! - [`event`] - Terminal event handling
//! - [`terminal`] - Terminal initialization and management

pub mod app;
pub mod app_state;
pub mod calc;
pub mod color_scheme;
pub mod event;
pub mod helpers;
pub mod input;
pub mod operations;
pub mod rendering;
pub mod state;
pub mod syntax;
pub mod terminal;
pub mod note_colors;

pub use app::App;
pub use color_scheme::ColorScheme;
pub use event::{Event, EventHandler};
pub use terminal::Tui;
