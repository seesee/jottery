//! Business logic operations for the TUI application
//!
//! This module contains all the business logic operations extracted from the main
//! App struct. Operations are organized by functionality:
//!
//! - **auth**: Authentication and password management
//! - **notes**: Note CRUD operations, versions, and trash management
//! - **attachments**: Attachment management and viewing
//! - **settings**: Settings management and sync credentials
//! - **sync**: Bidirectional sync with remote server

pub mod auth;
pub mod bulk;
pub mod inbox;
pub mod notes;
pub mod attachments;
pub mod settings;
pub mod sync;
