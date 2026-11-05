//! Repository pattern for database access
//! Provides clean abstraction over database operations

pub mod note;
pub mod note_version;
pub mod settings;
pub mod attachment;
pub mod sync;
pub mod encryption;
pub mod rekey;
pub mod saved_search;

pub use note::NoteRepository;
pub use note_version::{NoteVersionRepository, NoteVersion, VersionReason};
pub use settings::SettingsRepository;
pub use encryption::EncryptionRepository;
pub use saved_search::{SavedSearch, SavedSearchRepository};
