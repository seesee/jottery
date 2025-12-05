/// Repository pattern for database access
/// Provides clean abstraction over database operations

pub mod note;
pub mod settings;
pub mod attachment;
pub mod sync;

pub use note::NoteRepository;
pub use settings::SettingsRepository;
pub use attachment::AttachmentRepository;
pub use sync::SyncRepository;
