//! Application state enums and types
//!
//! This module defines the core state machine for the TUI application,
//! including app states, input modes, and view modes.

use zeroize::Zeroizing;

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
        /// Current scroll offset
        scroll_offset: usize,
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
        password: Zeroizing<String>,
        previous: Box<AppState>,
    },
    /// Password change flow - input current password
    ChangePasswordCurrent {
        previous: Box<AppState>,
    },
    /// Password change flow - input new password
    ChangePasswordNew {
        current_password: Zeroizing<String>,
        previous: Box<AppState>,
    },
    /// Password change flow - confirm new password
    ChangePasswordConfirm {
        current_password: Zeroizing<String>,
        new_password: Zeroizing<String>,
        previous: Box<AppState>,
    },
    /// Quit
    Quit,
}

/// Names the state and nothing else, because several states hold passwords, which
/// `Zeroizing` prints in full.
impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Locked => "Locked",
            Self::NoteList => "NoteList",
            Self::NoteView => "NoteView",
            Self::Settings { .. } => "Settings",
            Self::Help { .. } => "Help",
            Self::InputEmailForStatus { .. } => "InputEmailForStatus",
            Self::ShowRegistrationStatus { .. } => "ShowRegistrationStatus",
            Self::RegisterInputEndpoint { .. } => "RegisterInputEndpoint",
            Self::RegisterInputEmail { .. } => "RegisterInputEmail",
            Self::RegisterInputPassword { .. } => "RegisterInputPassword",
            Self::RegisterPendingApproval { .. } => "RegisterPendingApproval",
            Self::RegisterInputDeviceName { .. } => "RegisterInputDeviceName",
            Self::ChangePasswordCurrent { .. } => "ChangePasswordCurrent",
            Self::ChangePasswordNew { .. } => "ChangePasswordNew",
            Self::ChangePasswordConfirm { .. } => "ChangePasswordConfirm",
            Self::Quit => "Quit",
        })
    }
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
    /// Attachment path input mode (entering a path to attach from or save to)
    AttachmentPath,
    /// Bulk add tags input mode
    BulkAddTags,
    /// Bulk remove tags input mode
    BulkRemoveTags,
    /// Bulk set colour input mode
    BulkSetColour,
    /// Bulk export path input mode
    BulkExportPath,
    /// Naming the query in the search bar, to save it
    SaveSearchName,
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
    /// Inbox view (unencrypted notes from Inbox API)
    Inbox,
    /// The list of saved searches
    SavedSearches,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_output_never_shows_a_password() {
        let password = uuid::Uuid::new_v4().to_string();
        let state = AppState::ChangePasswordConfirm {
            current_password: Zeroizing::new(password.clone()),
            new_password: Zeroizing::new(password.clone()),
            previous: Box::new(AppState::RegisterInputDeviceName {
                endpoint: String::new(),
                email: String::new(),
                password: Zeroizing::new(password.clone()),
                previous: Box::new(AppState::NoteList),
            }),
        };

        assert!(!format!("{state:?}").contains(&password));
    }
}
