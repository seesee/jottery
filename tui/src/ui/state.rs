//! Application state enums and types
//!
//! This module defines the core state machine for the TUI application,
//! including app states, input modes, and view modes.

/// Application state
pub enum AppState {
    /// Locked - password input screen
    Locked,
    /// Unlocked - main note list
    NoteList,
    /// Viewing/editing a note
    NoteView,
    /// Settings panel
    Settings {
        /// Previous state to return to
        previous: Box<AppState>,
    },
    /// Help screen
    Help {
        /// Previous state to return to
        previous: Box<AppState>,
    },
    /// Show sync credentials as text (for manual copy)
    ShowSyncCredentials {
        credentials: String,
        previous: Box<AppState>,
    },
    /// Input sync credentials as text (for manual paste)
    InputSyncCredentials {
        previous: Box<AppState>,
    },
    /// Input email for registration status check
    InputEmailForStatus {
        previous: Box<AppState>,
    },
    /// Show registration status result
    ShowRegistrationStatus {
        status_message: String,
        previous: Box<AppState>,
    },
    /// Registration flow - input sync server endpoint
    RegisterInputEndpoint {
        previous: Box<AppState>,
    },
    /// Registration flow - input email
    RegisterInputEmail {
        endpoint: String,
        previous: Box<AppState>,
    },
    /// Registration flow - input password
    RegisterInputPassword {
        endpoint: String,
        email: String,
        previous: Box<AppState>,
    },
    /// Registration flow - pending approval
    RegisterPendingApproval {
        endpoint: String,
        email: String,
        previous: Box<AppState>,
    },
    /// Registration flow - input device name (after approval)
    RegisterInputDeviceName {
        endpoint: String,
        email: String,
        password: String,
        previous: Box<AppState>,
    },
    /// Quit
    Quit,
}

/// Current input mode
pub enum InputMode {
    /// Normal mode (navigation)
    Normal,
    /// Insert mode (typing)
    Insert,
    /// Tag mode (adding tags)
    Tag,
    /// Settings edit mode
    SettingsEdit,
    /// Password verification mode (for enabling remember password)
    PasswordVerify,
    /// Attachment path input mode (entering file path to attach)
    /// Note: This mode is defined but not currently used in the application
    #[allow(dead_code)]
    AttachmentPath,
    /// Bulk add tags input mode
    BulkAddTags,
    /// Bulk export path input mode
    BulkExportPath,
}

/// Current view mode
pub enum ViewMode {
    /// Normal note list view
    NoteList,
    /// Recycle bin view (deleted notes)
    RecycleBin,
    /// Attachment viewer modal
    AttachmentViewer,
    /// Version history viewer modal
    VersionHistory,
    /// Conflict resolution modal
    ConflictResolution,
}

/// Which panel is focused in the preview pane
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusedPanel {
    /// Note content panel (default)
    #[default]
    NoteContent,
    /// Attachments panel
    Attachments,
}
