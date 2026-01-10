//! Application state sub-structs
//!
//! This module provides nested state structs for organizing the App's many fields
//! into logical groups for better maintainability.
//!
//! # Purpose
//!
//! The main [`App`](super::App) struct has ~50 fields spanning different concerns
//! (password handling, note editing, search, sync, etc.). These sub-structs group
//! related fields together for:
//!
//! - **Better organization**: Related fields are co-located
//! - **Clearer ownership**: Each struct owns its domain
//! - **Easier testing**: Sub-structs can be tested in isolation
//!
//! # Incremental Adoption
//!
//! These types are available for new code and can be adopted incrementally.
//! The existing App field access patterns continue to work - this module
//! provides an alternative organization that can be migrated to over time.
//!
//! # Available Structs
//!
//! - [`PasswordState`] - Password/unlock state
//! - [`EditState`] - Note editing state
//! - [`SearchState`] - Search state
//! - [`SelectionState`] - Note list selection state
//! - [`SyncState`] - Sync state
//! - [`ConflictState`] - Conflict resolution state
//! - [`BulkState`] - Bulk operations state
//! - [`AttachmentState`] - Attachment browser state
//! - [`VersionState`] - Version history state
//! - [`SettingsState`] - Settings view state
//! - [`LayoutState`] - UI layout state (for mouse interaction)

// Allow dead code - these structs are provided for incremental adoption
#![allow(dead_code)]

use std::collections::HashSet;
use crate::models::SyntaxLanguage;

/// Password/unlock state
#[derive(Debug, Default)]
pub struct PasswordState {
    /// Password input buffer
    pub input: String,
    /// Password confirmation buffer (for new databases)
    pub confirm: String,
    /// Whether database is being created (vs unlocked)
    pub is_new_database: bool,
    /// Which password field is active (false = password, true = confirm)
    pub confirm_focused: bool,
    /// Whether to remember password after successful unlock
    pub remember_checkbox: bool,
}

impl PasswordState {
    pub fn new(is_new_database: bool) -> Self {
        Self {
            is_new_database,
            ..Default::default()
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.confirm.clear();
        self.confirm_focused = false;
    }
}

/// Note editing state
#[derive(Debug, Default)]
pub struct EditState {
    /// Note content input buffer
    pub content: String,
    /// Current note's syntax language
    pub syntax: SyntaxLanguage,
    /// Tag input buffer (when adding tags)
    pub tag_input: String,
    /// Current tags for the note being edited
    pub current_tags: Vec<String>,
    /// Currently editing note ID (None = creating new note)
    pub editing_note_id: Option<String>,
}

impl EditState {
    pub fn clear(&mut self) {
        self.content.clear();
        self.syntax = SyntaxLanguage::default();
        self.tag_input.clear();
        self.current_tags.clear();
        self.editing_note_id = None;
    }

    pub fn set_for_note(&mut self, id: Option<String>, content: String, tags: Vec<String>, syntax: SyntaxLanguage) {
        self.editing_note_id = id;
        self.content = content;
        self.current_tags = tags;
        self.syntax = syntax;
        self.tag_input.clear();
    }
}

/// Search state
#[derive(Debug, Default)]
pub struct SearchState {
    /// Search input buffer
    pub input: String,
    /// Whether search mode is active
    pub active: bool,
}

impl SearchState {
    pub fn clear(&mut self) {
        self.input.clear();
        self.active = false;
    }

    pub fn activate(&mut self) {
        self.active = true;
    }
}

/// Selection state (for note list)
#[derive(Debug, Default)]
pub struct SelectionState {
    /// Selected note index in filtered list
    pub index: usize,
    /// Selected note ID (persists through reloads/syncs)
    pub note_id: Option<String>,
    /// Multi-selected note IDs
    pub multi_ids: HashSet<String>,
    /// Whether multi-select mode is active
    pub is_multi_mode: bool,
    /// Last selected index for range selection
    pub last_index: Option<usize>,
    /// Preview scroll offset (number of lines scrolled down)
    pub preview_scroll: usize,
}

impl SelectionState {
    pub fn clear_multi(&mut self) {
        self.multi_ids.clear();
        self.is_multi_mode = false;
        self.last_index = None;
    }

    pub fn toggle_multi(&mut self, note_id: &str) {
        if self.multi_ids.contains(note_id) {
            self.multi_ids.remove(note_id);
        } else {
            self.multi_ids.insert(note_id.to_string());
        }
        self.is_multi_mode = !self.multi_ids.is_empty();
    }
}

/// Sync state
#[derive(Debug, Default)]
pub struct SyncState {
    /// Sync status message
    pub status: Option<String>,
    /// Credential input buffer (for manual paste)
    pub credential_input: String,
    /// Show force resync confirmation modal
    pub show_force_confirm: bool,
}

impl SyncState {
    pub fn set_status(&mut self, status: impl Into<String>) {
        self.status = Some(status.into());
    }

    pub fn clear_status(&mut self) {
        self.status = None;
    }
}

/// Conflict resolution state
#[derive(Debug, Default)]
pub struct ConflictState {
    /// Note ID for which conflict is being resolved
    pub note_id: Option<String>,
    /// Decrypted local content for conflict view
    pub local_content: String,
    /// Decrypted server content for conflict view
    pub server_content: String,
    /// Decrypted local tags for conflict view
    pub local_tags: Vec<String>,
    /// Decrypted server tags for conflict view
    pub server_tags: Vec<String>,
    /// Scroll offset for conflict left pane (local version)
    pub local_scroll: usize,
    /// Scroll offset for conflict right pane (server version)
    pub server_scroll: usize,
    /// Which pane is focused in conflict view (false = local, true = server)
    pub focus_server: bool,
}

impl ConflictState {
    pub fn clear(&mut self) {
        self.note_id = None;
        self.local_content.clear();
        self.server_content.clear();
        self.local_tags.clear();
        self.server_tags.clear();
        self.local_scroll = 0;
        self.server_scroll = 0;
        self.focus_server = false;
    }
}

/// Bulk operations state
#[derive(Debug, Default)]
pub struct BulkState {
    /// Bulk tags input buffer (for adding tags to selected notes)
    pub tags_input: String,
    /// Bulk export path input buffer
    pub export_path: String,
    /// Show bulk delete confirmation modal
    pub show_delete_confirm: bool,
    /// Show bulk combine confirmation modal
    pub show_combine_confirm: bool,
}

impl BulkState {
    pub fn clear(&mut self) {
        self.tags_input.clear();
        self.export_path.clear();
        self.show_delete_confirm = false;
        self.show_combine_confirm = false;
    }
}

/// Attachment browser state
#[derive(Debug, Default)]
pub struct AttachmentState {
    /// Selected attachment index in preview pane
    pub selected: usize,
    /// File path input buffer (when adding attachments)
    pub path_input: String,
    /// Path completions for Tab completion
    pub completions: Vec<String>,
    /// Selected completion index
    pub completion_index: usize,
}

impl AttachmentState {
    pub fn clear(&mut self) {
        self.path_input.clear();
        self.completions.clear();
        self.completion_index = 0;
    }
}

/// Version history state
#[derive(Debug, Default)]
pub struct VersionState {
    /// Loaded versions for version history viewer
    pub versions: Vec<crate::repository::NoteVersion>,
    /// Selected version index in version history viewer
    pub selected: usize,
    /// Note ID for which versions are loaded (to detect when to reload)
    pub note_id: Option<String>,
    /// Scroll offset for version preview content
    pub preview_scroll: usize,
}

impl VersionState {
    pub fn clear(&mut self) {
        self.versions.clear();
        self.selected = 0;
        self.note_id = None;
        self.preview_scroll = 0;
    }
}

/// Settings view state
#[derive(Debug, Default)]
pub struct SettingsState {
    /// Selected settings field
    pub selected: usize,
    /// Settings input buffer (for string/number fields)
    pub input: String,
}

/// UI layout state (for mouse interaction)
#[derive(Debug, Default)]
pub struct LayoutState {
    /// Rendered area for note list (for mouse click detection)
    pub note_list_area: Option<ratatui::layout::Rect>,
    /// Rendered area for tags line in preview (for mouse click detection)
    pub tags_line_area: Option<ratatui::layout::Rect>,
    /// Individual tag positions for click detection: (tag_name, x_start, x_end)
    pub tag_positions: Vec<(String, u16, u16)>,
}
