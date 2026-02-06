//! Main application state and coordination
//!
//! This module contains the core `App` struct and coordination logic for the TUI.
//! Business logic has been extracted into separate modules:
//!
//! - [`state`](../state) - State enums and types
//! - [`rendering`](../rendering) - UI rendering functions
//! - [`operations`](../operations) - Business logic operations
//! - [`input`](../input) - Keyboard input handlers
//! - [`helpers`](../helpers) - Markdown processing utilities
//!
//! # Refactoring Summary
//!
//! The original app.rs file (~5,050 lines) has been refactored to improve
//! maintainability and testability:
//!
//! - Final app.rs size: ~2,257 lines (55% reduction)
//! - Modules created: 20+ new module files
//! - Benefits: improved organization, better separation of concerns
//!
//! All functionality has been preserved with no logic changes.

use anyhow::{Context, Result};
use crossterm::{
    event::KeyEvent,
};
use rust_i18n::t;
use ratatui::Frame;
use zeroize::Zeroize;
use std::{
    cell::RefCell,
    collections::HashSet,
    fs::File,
    io::Write,
    path::PathBuf,
    time::Instant,
    sync::{Arc, Mutex},
};

use crate::{
    crypto::{CryptoService, KeyManager},
    db::Database,
    models::{Note, UserSettings, sync::{SyncCredentials, ConflictData}},
    password_storage::{self, PasswordStorage, RetrieveResult, StorageBackendType},
    repository::{EncryptionRepository, NoteRepository, SettingsRepository, sync::SyncRepository},
};


use super::state::{AppState, InputMode, ViewMode};
use super::rendering;
use super::input;

/// Search modifiers parsed from query string
#[derive(Debug, Default)]
struct SearchModifiers {
    has_attachment: bool,
    created_after: Option<String>,
    created_before: Option<String>,
    modified_after: Option<String>,
    modified_before: Option<String>,
    word_count_min: Option<usize>,
    word_count_max: Option<usize>,
    colors: Vec<String>,           // Color keys to match
    color_target: ColorTarget,      // Where to look for colors
}

#[derive(Debug, Clone, PartialEq, Default)]
enum ColorTarget {
    #[default]
    Both,   // Match colors on notes or tags (default)
    Note,   // Match only note colors
    Tag,    // Match only tag colors
}

/// Check if a note matches search criteria (excluding archive mode)
///
/// This is the shared search logic used by both filtered_notes() and count_opposite_mode_matches()
fn note_matches_search(
    note: &Note,
    modifiers: &SearchModifiers,
    query_parts: &[&str],
) -> bool {
    // has:attachment
    if modifiers.has_attachment && note.attachments.is_empty() {
        return false;
    }

    // created:>DATE (created after)
    if let Some(ref date) = modifiers.created_after {
        let note_date = note.created_at.format("%Y-%m-%d").to_string();
        if note_date.as_str() < date.as_str() {
            return false;
        }
    }

    // created:<DATE (created before)
    if let Some(ref date) = modifiers.created_before {
        let note_date = note.created_at.format("%Y-%m-%d").to_string();
        if note_date.as_str() > date.as_str() {
            return false;
        }
    }

    // modified:>DATE (modified after)
    if let Some(ref date) = modifiers.modified_after {
        let note_date = note.modified_at.format("%Y-%m-%d").to_string();
        if note_date.as_str() < date.as_str() {
            return false;
        }
    }

    // modified:<DATE (modified before)
    if let Some(ref date) = modifiers.modified_before {
        let note_date = note.modified_at.format("%Y-%m-%d").to_string();
        if note_date.as_str() > date.as_str() {
            return false;
        }
    }

    // words:>N (minimum word count)
    if let Some(min) = modifiers.word_count_min {
        let word_count = note.content.split_whitespace().count();
        if word_count < min {
            return false;
        }
    }

    // words:<N (maximum word count)
    if let Some(max) = modifiers.word_count_max {
        let word_count = note.content.split_whitespace().count();
        if word_count > max {
            return false;
        }
    }

    // Check each remaining query part (text/tag search)
    let content_lower = note.content.to_lowercase();
    for part in query_parts {
        if part.is_empty() {
            continue;
        }
        if let Some(tag) = part.strip_prefix('#') {
            // Tag search
            if !note.tags.iter().any(|t| t.to_lowercase().contains(tag)) {
                return false;
            }
        } else if let Some(neg_word) = part.strip_prefix('-') {
            // Negation
            if content_lower.contains(neg_word) {
                return false;
            }
        } else {
            // Regular text search
            if !content_lower.contains(part) {
                return false;
            }
        }
    }

    true
}

/// Application state and coordinator
pub struct App {
    /// Current view mode
    pub view_mode: ViewMode,
    /// Current state
    pub state: AppState,
    /// Input mode
    pub input_mode: InputMode,
    /// Password input buffer
    pub password_input: String,
    /// Password confirmation buffer (for new databases)
    pub password_confirm: String,
    /// Whether database is being created (vs unlocked)
    pub is_new_database: bool,
    /// Which password field is active (false = password, true = confirm)
    pub password_confirm_focused: bool,
    /// Whether to remember password after successful unlock
    pub remember_password_checkbox: bool,
    /// Note content input buffer
    pub note_input: String,
    /// Current note's syntax language
    pub note_syntax: crate::models::SyntaxLanguage,
    /// Tag input buffer (when adding tags)
    pub tag_input: String,
    /// Current tags for the note being edited
    pub current_tags: Vec<String>,
    /// Search input buffer
    pub search_input: String,
    /// Whether search mode is active
    pub search_active: bool,
    /// Whether archive mode is active (shows archived notes instead of active)
    pub archive_mode: bool,
    /// Sync status message
    pub sync_status: Option<String>,
    /// Current error message
    pub error: Option<String>,
    /// Flag to signal that terminal needs full redraw
    pub need_redraw: bool,
    /// Selected settings field (0-5: language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint)
    pub selected_setting: usize,
    /// Settings input buffer (for string/number fields)
    pub setting_input: String,
    /// Database path
    pub(crate) db_path: PathBuf,
    /// Database connection (when unlocked)
    pub(crate) db: Option<Database>,
    /// Master key (when unlocked)
    pub(crate) key: Option<[u8; 32]>,
    /// Key manager
    pub(crate) key_manager: KeyManager,
    /// Crypto service
    pub(crate) crypto: CryptoService,
    /// Loaded notes
    pub(crate) notes: Vec<Note>,
    /// Selected note index
    pub selected_note: usize,
    /// Selected note ID (persists through reloads/syncs)
    pub(crate) selected_note_id: Option<String>,
    /// Multi-selected note IDs
    pub selected_note_ids: HashSet<String>,
    /// Set of note IDs with unresolved conflicts (cached for rendering)
    pub conflict_note_ids: HashSet<String>,
    /// Whether multi-select mode is active
    pub is_multi_select_mode: bool,
    /// Last selected index for range selection
    pub last_selected_index: Option<usize>,
    /// Preview scroll offset (number of lines scrolled down)
    pub preview_scroll_offset: usize,
    /// Currently editing note ID (None = creating new note)
    pub(crate) editing_note_id: Option<String>,
    /// Settings
    pub settings: UserSettings,
    /// Sync credentials input buffer (for manual paste)
    pub credential_input: String,
    /// Debug log file (for troubleshooting)
    pub debug_log: Option<Arc<Mutex<File>>>,
    /// Syntax highlighter for code preview
    pub syntax_highlighter: crate::ui::syntax::SyntaxHighlighter,
    /// Last auto-sync time (for periodic sync)
    pub(crate) last_auto_sync: Option<Instant>,
    /// When sync status was set (for auto-clearing)
    pub(crate) sync_status_set_at: Option<Instant>,
    /// Current color scheme (cached from settings)
    pub color_scheme: crate::ui::ColorScheme,
    /// Cached color palette names (for color cycling without allocation)
    pub(crate) color_palette_cache: Vec<String>,
    /// Cached unique tags from all notes (for fast tag completion)
    pub(crate) all_tags_cache: Vec<String>,
    /// Cached filtered note IDs (for avoiding repeated filtering/sorting)
    /// Uses RefCell for interior mutability so filtered_notes(&self) can update cache
    filtered_notes_cache: RefCell<Option<Vec<String>>>,
    /// Selected attachment index in preview pane
    pub selected_attachment: usize,
    /// Which panel is focused in preview pane (content or attachments)
    pub focused_panel: super::state::FocusedPanel,
    /// File path input buffer (when adding attachments)
    pub attachment_path_input: String,
    /// Path completions for Tab completion
    pub path_completions: Vec<String>,
    /// Selected completion index
    pub path_completion_index: usize,
    /// Whether chafa is available for image preview (lazy-loaded)
    pub(crate) chafa_available: Option<bool>,
    /// Track if 'a' key was pressed (for a1, a2 sequence)
    pub(crate) last_key_was_a: bool,
    /// Show force resync confirmation modal
    pub show_force_sync_confirm: bool,
    /// Loaded versions for version history viewer
    pub loaded_versions: Vec<crate::repository::NoteVersion>,
    /// Selected version index in version history viewer
    pub selected_version: usize,
    /// Note ID for which versions are loaded (to detect when to reload)
    pub(crate) versions_note_id: Option<String>,
    /// Scroll offset for version preview content
    pub version_preview_scroll_offset: usize,
    /// Note ID for which conflict is being resolved
    pub(crate) conflict_note_id: Option<String>,
    /// Conflict data from server (encrypted)
    pub(crate) conflict_data: Option<ConflictData>,
    /// Decrypted local content for conflict view
    pub conflict_local_content: String,
    /// Decrypted server content for conflict view
    pub conflict_server_content: String,
    /// Decrypted local tags for conflict view
    pub conflict_local_tags: Vec<String>,
    /// Decrypted server tags for conflict view
    pub conflict_server_tags: Vec<String>,
    /// Scroll offset for conflict left pane (local version)
    pub conflict_local_scroll: usize,
    /// Scroll offset for conflict right pane (server version)
    pub conflict_server_scroll: usize,
    /// Which pane is focused in conflict view (false = local, true = server)
    pub conflict_focus_server: bool,
    /// Decrypted ancestor content for 3-way merge (if available)
    pub conflict_ancestor_content: Option<String>,
    /// Decrypted ancestor tags for 3-way merge (if available)
    pub conflict_ancestor_tags: Option<Vec<String>>,
    /// Bulk tags input buffer (for adding tags to selected notes)
    pub bulk_tags_input: String,
    /// Bulk export path input buffer
    pub bulk_export_path_input: String,
    /// Show bulk delete confirmation modal
    pub show_bulk_delete_confirm: bool,
    /// Show bulk combine confirmation modal
    pub show_bulk_combine_confirm: bool,
    /// Show note info modal
    pub show_note_info: bool,
    /// Rendered area for note list (for mouse click detection)
    pub note_list_area: Option<ratatui::layout::Rect>,
    /// Rendered area for tags line in preview (for mouse click detection)
    pub tags_line_area: Option<ratatui::layout::Rect>,
    /// Individual tag positions for click detection: (tag_name, x_start, x_end)
    pub tag_positions: Vec<(String, u16, u16)>,
    /// Note link positions for click detection: (note_id, y_line, x_start, x_end)
    pub note_link_positions: Vec<(String, u16, u16, u16)>,
    /// Rendered area for preview pane (for link click detection)
    pub preview_area: Option<ratatui::layout::Rect>,
    /// Tag completions for Tab cycling in tag input mode
    pub tag_completions: Vec<String>,
    /// Selected tag completion index
    pub tag_completion_index: usize,
    /// Search tag completions for Tab cycling in search mode
    pub search_tag_completions: Vec<String>,
    /// Selected search tag completion index
    pub search_tag_completion_index: usize,
    /// Password storage backend (keychain or file-based fallback)
    pub(crate) password_storage: Box<dyn PasswordStorage>,
    /// SSE handle for real-time sync notifications (when connected)
    pub(crate) sse_handle: Option<crate::sse::SseHandle>,
    /// Inbox items from server (unencrypted)
    pub inbox_items: Vec<crate::models::sync::InboxItem>,
    /// Selected inbox item index
    pub selected_inbox_item: usize,
    /// Show inbox delete confirmation
    pub show_inbox_delete_confirm: bool,
    /// Show inbox delete-all confirmation
    pub show_inbox_delete_all_confirm: bool,
}


impl App {
    /// Create a new app
    pub fn new(db_path: PathBuf, debug_log: Option<Arc<Mutex<File>>>) -> Result<Self> {
        let is_new_database = !db_path.exists();

        // Initialise password storage with migration from file to keychain if available
        let config_dir = db_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let password_storage = password_storage::create_storage(config_dir, true);

        Ok(Self {
            view_mode: ViewMode::NoteList,
            state: AppState::Locked,
            input_mode: InputMode::Normal,
            password_input: String::new(),
            password_confirm: String::new(),
            is_new_database,
            password_confirm_focused: false,
            remember_password_checkbox: false,
            note_input: String::new(),
            note_syntax: crate::models::SyntaxLanguage::default(),
            tag_input: String::new(),
            current_tags: Vec::new(),
            search_input: String::new(),
            search_active: false,
            archive_mode: false,
            sync_status: None,
            error: None,
            need_redraw: false,
            selected_setting: 0,
            setting_input: String::new(),
            db_path,
            db: None,
            key: None,
            key_manager: KeyManager::new(),
            crypto: CryptoService::new(),
            notes: Vec::new(),
            selected_note: 0,
            selected_note_id: None,
            selected_note_ids: HashSet::new(),
            conflict_note_ids: HashSet::new(),
            is_multi_select_mode: false,
            last_selected_index: None,
            preview_scroll_offset: 0,
            editing_note_id: None,
            settings: UserSettings::default(),
            credential_input: String::new(),
            debug_log,
            syntax_highlighter: crate::ui::syntax::SyntaxHighlighter::new(),
            last_auto_sync: None,
            sync_status_set_at: None,
            color_scheme: crate::ui::ColorScheme::default(),
            color_palette_cache: Vec::new(), // Populated when settings are loaded
            all_tags_cache: Vec::new(), // Populated when notes are loaded
            filtered_notes_cache: RefCell::new(None), // Populated on first access, invalidated on changes
            selected_attachment: 0,
            focused_panel: super::state::FocusedPanel::default(),
            attachment_path_input: String::new(),
            path_completions: Vec::new(),
            path_completion_index: 0,
            chafa_available: None,
            last_key_was_a: false,
            show_force_sync_confirm: false,
            loaded_versions: Vec::new(),
            selected_version: 0,
            versions_note_id: None,
            version_preview_scroll_offset: 0,
            conflict_note_id: None,
            conflict_data: None,
            conflict_local_content: String::new(),
            conflict_server_content: String::new(),
            conflict_local_tags: Vec::new(),
            conflict_server_tags: Vec::new(),
            conflict_local_scroll: 0,
            conflict_server_scroll: 0,
            conflict_focus_server: false,
            conflict_ancestor_content: None,
            conflict_ancestor_tags: None,
            bulk_tags_input: String::new(),
            bulk_export_path_input: String::new(),
            show_bulk_delete_confirm: false,
            show_bulk_combine_confirm: false,
            show_note_info: false,
            note_list_area: None,
            tags_line_area: None,
            tag_positions: Vec::new(),
            note_link_positions: Vec::new(),
            preview_area: None,
            tag_completions: Vec::new(),
            tag_completion_index: 0,
            search_tag_completions: Vec::new(),
            search_tag_completion_index: 0,
            password_storage,
            sse_handle: None,
            inbox_items: Vec::new(),
            selected_inbox_item: 0,
            show_inbox_delete_confirm: false,
            show_inbox_delete_all_confirm: false,
        })
    }

    /// Write to debug log if enabled
    pub(crate) fn debug_log(&self, message: &str) {
        if let Some(log) = &self.debug_log {
            if let Ok(mut file) = log.lock() {
                let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
                let _ = writeln!(file, "[{}] {}", timestamp, message);
                let _ = file.flush();
            }
        }
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle help and settings screens separately to avoid borrow issues
        if let AppState::Help { .. } = &self.state {
            return input::help::handle_help_key(self, key);
        }
        if let AppState::Settings { .. } = &self.state {
            return input::settings::handle_settings_key(self, key);
        }

        match &self.state {
            AppState::Locked => input::locked::handle_locked_key(self, key)?,
            AppState::NoteList => input::note_list::handle_note_list_key(self, key)?,
            AppState::NoteView => input::note_view::handle_note_view_key(self, key)?,
            AppState::ShowSyncCredentials { .. } => input::credentials::handle_show_credentials_key(self, key)?,
            AppState::InputSyncCredentials { .. } => input::credentials::handle_input_credentials_key(self, key)?,
            AppState::ImportInputDeviceName { .. } => input::credentials::handle_import_device_name_key(self, key)?,
            AppState::InputEmailForStatus { .. } => input::credentials::handle_input_email_for_status_key(self, key)?,
            AppState::ShowRegistrationStatus { .. } => input::credentials::handle_show_registration_status_key(self, key)?,
            AppState::RegisterInputEndpoint { .. } => input::credentials::handle_register_input_endpoint_key(self, key)?,
            AppState::RegisterInputEmail { .. } => input::credentials::handle_register_input_email_key(self, key)?,
            AppState::RegisterInputPassword { .. } => input::credentials::handle_register_input_password_key(self, key)?,
            AppState::RegisterPendingApproval { .. } => input::credentials::handle_register_pending_approval_key(self, key)?,
            AppState::RegisterInputDeviceName { .. } => input::credentials::handle_register_input_device_name_key(self, key)?,
            AppState::Quit => {}
            AppState::Settings { .. } => unreachable!(), // Handled above
            AppState::Help { .. } => unreachable!(), // Handled above
        }
        Ok(())
    }




    /// Unlock the database
    fn unlock(&mut self) -> Result<()> {
        // Open database
        let db = Database::open(&self.db_path, &self.password_input)
            .context("Failed to open database")?;

        let encryption_repo = EncryptionRepository::new(db.connection());

        // Get or create encryption metadata
        let (salt, iterations) = if let Some(metadata) = encryption_repo.get()? {
            // Load existing salt from database
            (metadata.salt, metadata.iterations)
        } else {
            // First-time setup: generate new salt and save it
            let new_salt = self.crypto.generate_salt();
            let iterations = 256_000;
            encryption_repo.save(&new_salt, iterations)?;
            (new_salt.to_vec(), iterations)
        };

        // Derive encryption key from password and salt
        self.debug_log(&format!("Unlock - Password length: {} chars", self.password_input.len()));
        self.debug_log(&format!("Unlock - Password is empty: {}", self.password_input.is_empty()));

        let key = self
            .crypto
            .derive_key(&self.password_input, &salt, iterations)?;

        // Debug logging for troubleshooting (key bytes intentionally excluded for security)
        self.debug_log(&format!("Unlock - Salt (hex): {}", hex::encode(&salt)));
        self.debug_log(&format!("Unlock - Salt length: {} bytes", salt.len()));
        self.debug_log(&format!("Unlock - Iterations: {}", iterations));

        self.key_manager.set_master_key(key);
        self.key = Some(key);
        self.db = Some(db);

        // Check if API key needs encryption (from paste credentials flow)
        if let Some(db) = &self.db {
            use crate::repository::sync::SyncRepository;
            let sync_repo = SyncRepository::new(db.connection());

            if let Ok(Some(mut metadata)) = sync_repo.get_metadata() {
                if let Some(api_key_str) = &metadata.api_key {
                    // Check if API key is plaintext (prefixed with "PLAINTEXT:")
                    if let Some(plaintext_key) = api_key_str.strip_prefix("PLAINTEXT:") {
                        self.debug_log("Unlock - Detected plaintext API key, encrypting with new key");

                        // Encrypt API key with the newly derived key
                        let encrypted = self.crypto.encrypt_text(plaintext_key, &key)?;
                        let encrypted_api_key = serde_json::to_string(&encrypted)?;

                        // Update metadata with encrypted API key
                        metadata.api_key = Some(encrypted_api_key);
                        sync_repo.update_metadata(&metadata)?;

                        self.debug_log("Unlock - API key encrypted and saved");
                    }
                }
            }
        }

        // Load notes
        self.load_notes()?;

        // Load settings
        if let Some(db) = &self.db {
            let settings_repo = SettingsRepository::new(db.connection());
            self.settings = settings_repo.get()?;
            self.debug_log(&format!("Unlock - Settings loaded: theme={}", self.settings.theme));
            self.debug_log(&format!("Unlock - Theme scheme name: {}", self.settings.theme.scheme_name()));
            self.debug_log(&format!("Unlock - Color palette present: {}", self.settings.color_palette.is_some()));
            self.debug_log(&format!("Unlock - Tag colors present: {}", self.settings.tag_colors.is_some()));
            // Update color scheme from loaded settings
            self.color_scheme = crate::ui::ColorScheme::by_name(self.settings.theme.scheme_name());
            // Update color palette cache
            self.refresh_color_palette_cache();
            self.debug_log("Unlock - Color scheme updated");
        }

        // Store password if remember checkbox was enabled
        if self.remember_password_checkbox {
            // The password is still in self.password_input at this point
            let password_to_store = self.password_input.clone();
            if let Err(e) = self.store_password_for_autounlock(&password_to_store) {
                self.error = Some(format!("Failed to store password: {}", e));
            } else {
                // Show different message based on storage backend type
                let msg = if self.password_storage.backend_type().is_secure() {
                    t!("password.remember_keychain_enabled")
                } else {
                    t!("password.remember_enabled")
                };
                self.sync_status = Some(msg.to_string());
                self.sync_status_set_at = Some(Instant::now());
            }
        }

        // Zeroize password fields for security and reset flags
        self.password_input.zeroize();
        self.password_confirm.zeroize();
        self.is_new_database = false;  // Database now exists
        self.password_confirm_focused = false;  // Reset focus
        self.remember_password_checkbox = false;  // Reset checkbox
        self.state = AppState::NoteList;

        Ok(())
    }

    /// Attempt to auto-unlock using stored password
    /// Returns Ok(true) if successfully unlocked, Ok(false) if no stored password, Err on failure
    pub fn try_auto_unlock(&mut self) -> Result<bool> {
        let backend_name = if self.password_storage.backend_type().is_secure() {
            "keychain"
        } else {
            "file"
        };
        self.debug_log(&format!("Auto-unlock: using {} storage backend", backend_name));

        // New database can't be auto-unlocked
        if !self.db_path.exists() {
            self.debug_log("Auto-unlock: database does not exist yet");
            return Ok(false);
        }

        // Try to retrieve stored password using the password storage backend
        self.debug_log("Auto-unlock: attempting to retrieve stored password");
        let password = match self.password_storage.retrieve() {
            RetrieveResult::Found(pwd) => {
                self.debug_log(&format!("Auto-unlock: found password ({} chars)", pwd.len()));
                pwd
            }
            RetrieveResult::NotFound => {
                self.debug_log("Auto-unlock: no stored password found");
                return Ok(false);
            }
            RetrieveResult::Error(e) => {
                self.debug_log(&format!("Auto-unlock: error retrieving password: {}", e));
                return Ok(false);
            }
        };

        self.debug_log("Auto-unlock: attempting to unlock with stored password");

        // Try to unlock with this password
        self.password_input = password;
        match self.unlock() {
            Ok(()) => {
                self.debug_log("Auto-unlock successful");
                Ok(true)
            }
            Err(e) => {
                self.password_input.zeroize();
                // Delete invalid stored password
                let _ = self.password_storage.delete();
                Err(e).context("Auto-unlock failed")
            }
        }
    }

    /// Get the current password storage backend type
    #[allow(dead_code)]
    pub fn password_storage_type(&self) -> StorageBackendType {
        self.password_storage.backend_type()
    }

    /// Enable/disable remember password feature
    /// When enabling, encrypts and stores the current password
    #[allow(dead_code)]
    pub fn toggle_remember_password(&mut self) -> Result<()> {
        if self.settings.remember_password {
            // Disable: clear stored password using the storage backend
            self.settings.remember_password = false;
            self.settings.stored_password = None;

            // Delete stored password
            let _ = self.password_storage.delete();

            // Save settings
            if let Some(db) = &self.db {
                let settings_repo = SettingsRepository::new(db.connection());
                settings_repo.update(&self.settings)?;
            }

            self.sync_status = Some(t!("password.remember_disabled").to_string());
            self.sync_status_set_at = Some(Instant::now());
        } else {
            // Enable: this should be done through a confirmation flow
            self.sync_status = Some("Feature not yet fully implemented - use settings".to_string());
            self.sync_status_set_at = Some(Instant::now());
        }
        Ok(())
    }

    /// Store password for auto-unlock (call after successful unlock when user confirms)
    pub fn store_password_for_autounlock(&mut self, password: &str) -> Result<()> {
        // Store password using the storage backend (keychain or file)
        self.password_storage.store(password)?;

        self.debug_log(&format!(
            "Password stored using {} backend",
            if self.password_storage.backend_type().is_secure() { "keychain" } else { "file" }
        ));

        // Update settings (note: we no longer store encrypted password in settings)
        self.settings.remember_password = true;
        self.settings.stored_password = None; // No longer needed in settings

        if let Some(db) = &self.db {
            let settings_repo = SettingsRepository::new(db.connection());
            settings_repo.update(&self.settings)?;
        }

        Ok(())
    }

    /// Load notes from database
    fn load_notes(&mut self) -> Result<()> {
        if let (Some(db), Some(key)) = (&self.db, &self.key) {
            let old_selected_note = self.selected_note;
            let old_selected_note_id = self.selected_note_id.clone();

            let repo = NoteRepository::new(db.connection());
            self.notes = repo.list(false, key)?;

            self.debug_log(&format!("load_notes: old_selected_note={}, old_selected_note_id={:?}, notes_count={}",
                old_selected_note, old_selected_note_id, self.notes.len()));

            // Restore selection to the same note if it still exists (using persisted ID)
            // NOTE: We need to find position in the sorted view (pinned first, then by modified_at)
            // because selected_note is an index into filtered_notes(), not self.notes
            if let Some(note_id) = &self.selected_note_id.clone() {
                // Build sorted view like filtered_notes() does
                let mut sorted_notes: Vec<&Note> = self.notes.iter().collect();
                Self::sort_notes_by_pinned_and_date(&mut sorted_notes);

                if let Some(index) = sorted_notes.iter().position(|n| &n.id == note_id) {
                    self.selected_note = index;
                    self.debug_log(&format!("load_notes: found note {} at sorted index {}", note_id, index));
                } else {
                    // Note was deleted, select first note and update ID
                    self.selected_note = 0;
                    self.selected_note_id = sorted_notes.first().map(|n| n.id.clone());
                    self.debug_log(&format!("load_notes: note {} NOT FOUND, defaulting to 0", note_id));
                }
            } else {
                self.selected_note = 0;
                // Get first note from sorted view
                let mut sorted_notes: Vec<&Note> = self.notes.iter().collect();
                Self::sort_notes_by_pinned_and_date(&mut sorted_notes);
                self.selected_note_id = sorted_notes.first().map(|n| n.id.clone());
                self.debug_log("load_notes: no selected_note_id, defaulting to 0");
            }
        }
        Ok(())
    }

    /// Refresh the conflict note IDs cache from database
    pub fn refresh_conflict_cache(&mut self) {
        self.conflict_note_ids.clear();
        if let Some(db) = &self.db {
            let sync_repo = crate::repository::sync::SyncRepository::new(db.connection());
            if let Ok(ids) = sync_repo.get_conflict_note_ids() {
                self.conflict_note_ids = ids.into_iter().collect();
            }
        }
    }

    // Multi-select methods

    /// Toggle selection of a note at the given index
    pub fn toggle_note_selection(&mut self, index: usize) {
        // Get note IDs first to avoid borrow issues
        let note_ids: Vec<String> = self.filtered_notes().iter().map(|n| n.id.clone()).collect();
        if let Some(note_id) = note_ids.get(index) {
            if self.selected_note_ids.contains(note_id) {
                self.selected_note_ids.remove(note_id);
            } else {
                self.selected_note_ids.insert(note_id.clone());
            }
            self.is_multi_select_mode = !self.selected_note_ids.is_empty();
            self.last_selected_index = Some(index);
        }
    }

    /// Select a range of notes from last_selected_index to current index
    pub fn select_range(&mut self, to_index: usize) {
        let from_index = self.last_selected_index.unwrap_or(self.selected_note);
        let start = from_index.min(to_index);
        let end = from_index.max(to_index);

        // Get note IDs first to avoid borrow issues
        let note_ids: Vec<String> = self.filtered_notes().iter().map(|n| n.id.clone()).collect();
        for i in start..=end {
            if let Some(note_id) = note_ids.get(i) {
                self.selected_note_ids.insert(note_id.clone());
            }
        }
        self.is_multi_select_mode = !self.selected_note_ids.is_empty();
        self.last_selected_index = Some(to_index);
    }

    /// Select all currently filtered notes
    pub fn select_all_filtered(&mut self) {
        // Get note IDs first to avoid borrow issues
        let note_ids: Vec<String> = self.filtered_notes().iter().map(|n| n.id.clone()).collect();
        for note_id in note_ids {
            self.selected_note_ids.insert(note_id);
        }
        self.is_multi_select_mode = !self.selected_note_ids.is_empty();
    }

    /// Clear all multi-selection
    pub fn clear_multi_selection(&mut self) {
        self.selected_note_ids.clear();
        self.is_multi_select_mode = false;
        self.last_selected_index = None;
    }

    /// Reset path completions state
    pub fn reset_path_completions(&mut self) {
        self.path_completions.clear();
        self.path_completion_index = 0;
    }

    /// Refresh the color palette cache from current settings
    pub fn refresh_color_palette_cache(&mut self) {
        self.color_palette_cache = self.settings.get_color_palette()
            .keys()
            .cloned()
            .collect();
    }

    /// Get the cached color palette names
    pub fn get_color_names(&self) -> &[String] {
        &self.color_palette_cache
    }

    /// Refresh the all tags cache from current notes
    pub fn refresh_all_tags_cache(&mut self) {
        use std::collections::HashSet;
        let mut unique_tags: HashSet<String> = HashSet::new();
        for note in &self.notes {
            for tag in &note.tags {
                unique_tags.insert(tag.clone());
            }
        }
        self.all_tags_cache = unique_tags.into_iter().collect();
        self.all_tags_cache.sort();
    }

    /// Get tags matching a prefix (case-insensitive) from the cache
    pub fn get_matching_tags(&self, prefix: &str) -> Vec<String> {
        if prefix.is_empty() {
            return Vec::new();
        }
        let prefix_lower = prefix.to_lowercase();
        self.all_tags_cache
            .iter()
            .filter(|tag| tag.to_lowercase().starts_with(&prefix_lower))
            .cloned()
            .collect()
    }

    /// Invalidate the filtered notes cache
    ///
    /// Call this when any of these change:
    /// - search_input
    /// - archive_mode
    /// - notes (load, add, delete, modify)
    pub fn invalidate_filter_cache(&self) {
        *self.filtered_notes_cache.borrow_mut() = None;
    }

    /// Sort notes by pinned status first (pinned at top), then by modified_at descending
    pub fn sort_notes_by_pinned_and_date(notes: &mut [&Note]) {
        notes.sort_by(|a, b| {
            match (a.pinned, b.pinned) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.modified_at.cmp(&a.modified_at),
            }
        });
    }

    /// Filter notes based on search query and sort (pinned first, then by modified date)
    ///
    /// Uses an internal cache to avoid repeated filtering/sorting. The cache is
    /// automatically invalidated when search_input, archive_mode, or notes change.
    pub fn filtered_notes(&self) -> Vec<&Note> {
        // Check if we have a valid cache
        {
            let cache = self.filtered_notes_cache.borrow();
            if let Some(cached_ids) = cache.as_ref() {
                // Rebuild references from cached IDs
                return cached_ids.iter()
                    .filter_map(|id| self.notes.iter().find(|n| &n.id == id))
                    .collect();
            }
        }

        // Cache miss - compute filtered notes
        let mut notes: Vec<&Note> = if self.search_input.is_empty() {
            self.notes.iter()
                .filter(|note| note.archived == self.archive_mode)
                .collect()
        } else {
            let query = self.search_input.to_lowercase();

            // Parse advanced modifiers from query
            let modifiers = Self::parse_search_modifiers(&query, &self.settings);
            let remaining_query = Self::remove_modifiers_from_query(&query);
            let query_parts: Vec<&str> = remaining_query.split_whitespace().collect();

            self.notes
                .iter()
                .filter(|note| {
                    // Filter by archive mode first
                    if note.archived != self.archive_mode {
                        return false;
                    }

                    // Apply shared search logic
                    if !note_matches_search(note, &modifiers, &query_parts) {
                        return false;
                    }

                    // Color filtering (only in filtered_notes, not count_opposite_mode_matches)
                    if !modifiers.colors.is_empty() {
                        let note_has_color = modifiers.colors.iter().any(|color_key| {
                            note.color.as_ref().is_some_and(|c| c.to_lowercase() == color_key.to_lowercase())
                        });

                        let tag_has_color = note.tags.iter().any(|tag| {
                            let tag_color = self.settings.get_tag_color(tag);
                            tag_color.is_some_and(|tc| {
                                modifiers.colors.iter().any(|color_key| tc.to_lowercase() == color_key.to_lowercase())
                            })
                        });

                        match modifiers.color_target {
                            ColorTarget::Note => {
                                if !note_has_color {
                                    return false;
                                }
                            }
                            ColorTarget::Tag => {
                                if !tag_has_color {
                                    return false;
                                }
                            }
                            ColorTarget::Both => {
                                if !note_has_color && !tag_has_color {
                                    return false;
                                }
                            }
                        }
                    }

                    true
                })
                .collect()
        };

        // Sort: pinned first, then by modified_at descending
        Self::sort_notes_by_pinned_and_date(&mut notes);

        // Cache the note IDs for future calls
        let ids: Vec<String> = notes.iter().map(|n| n.id.clone()).collect();
        *self.filtered_notes_cache.borrow_mut() = Some(ids);

        notes
    }

    /// Count how many notes match the current search in the opposite mode (archive/active)
    /// Returns count of matching notes in the opposite mode
    pub fn count_opposite_mode_matches(&self) -> usize {
        if self.search_input.is_empty() {
            return 0;
        }

        let query = self.search_input.to_lowercase();
        let modifiers = Self::parse_search_modifiers(&query, &self.settings);
        let remaining_query = Self::remove_modifiers_from_query(&query);
        let query_parts: Vec<&str> = remaining_query.split_whitespace().collect();

        self.notes
            .iter()
            .filter(|note| {
                // Filter by OPPOSITE archive mode
                if note.archived == self.archive_mode {
                    return false;
                }

                // Apply shared search logic (no color filtering for opposite mode count)
                note_matches_search(note, &modifiers, &query_parts)
            })
            .count()
    }

    /// Parse search modifiers from query string
    fn parse_search_modifiers(query: &str, settings: &crate::models::UserSettings) -> SearchModifiers {
        use regex::Regex;

        let mut modifiers = SearchModifiers::default();

        // has:attachment
        if query.contains("has:attachment") {
            modifiers.has_attachment = true;
        }

        // created:>DATE
        if let Some(caps) = Regex::new(r"created:>(\d{4}-\d{2}-\d{2})")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.created_after = Some(caps[1].to_string());
        }

        // created:<DATE
        if let Some(caps) = Regex::new(r"created:<(\d{4}-\d{2}-\d{2})")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.created_before = Some(caps[1].to_string());
        }

        // created:DATE..DATE (range)
        if let Some(caps) = Regex::new(r"created:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.created_after = Some(caps[1].to_string());
            modifiers.created_before = Some(caps[2].to_string());
        }

        // modified:>DATE
        if let Some(caps) = Regex::new(r"modified:>(\d{4}-\d{2}-\d{2})")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.modified_after = Some(caps[1].to_string());
        }

        // modified:<DATE
        if let Some(caps) = Regex::new(r"modified:<(\d{4}-\d{2}-\d{2})")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.modified_before = Some(caps[1].to_string());
        }

        // modified:DATE..DATE (range)
        if let Some(caps) = Regex::new(r"modified:(\d{4}-\d{2}-\d{2})\.\.(\d{4}-\d{2}-\d{2})")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.modified_after = Some(caps[1].to_string());
            modifiers.modified_before = Some(caps[2].to_string());
        }

        // words:>N
        if let Some(caps) = Regex::new(r"words:>(\d+)")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.word_count_min = caps[1].parse().ok();
        }

        // words:<N
        if let Some(caps) = Regex::new(r"words:<(\d+)")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.word_count_max = caps[1].parse().ok();
        }

        // words:N..N (range)
        if let Some(caps) = Regex::new(r"words:(\d+)\.\.(\d+)")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.word_count_min = caps[1].parse().ok();
            modifiers.word_count_max = caps[2].parse().ok();
        }

        // color:red note (notes with red color)
        if let Some(caps) = Regex::new(r"(?i)\bcolou?r:([a-z0-9#]+)\s+note\b")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.colors.push(caps[1].to_lowercase());
            modifiers.color_target = ColorTarget::Note;
        }

        // color:red tag (notes with red-colored tags)
        if let Some(caps) = Regex::new(r"(?i)\bcolou?r:([a-z0-9#]+)\s+tag\b")
            .ok()
            .and_then(|re| re.captures(query))
        {
            modifiers.colors.push(caps[1].to_lowercase());
            modifiers.color_target = ColorTarget::Tag;
        }

        // color:red (both notes and tags)
        if modifiers.colors.is_empty() {
            if let Some(caps) = Regex::new(r"(?i)\bcolou?r:([a-z0-9#]+)\b")
                .ok()
                .and_then(|re| re.captures(query))
            {
                modifiers.colors.push(caps[1].to_lowercase());
                modifiers.color_target = ColorTarget::Both;
            }
        }

        // category:important note (notes with category named "important")
        if let Some(caps) = Regex::new(r"(?i)\bcategory:([\w-]+)\s+note\b")
            .ok()
            .and_then(|re| re.captures(query))
        {
            let category_name = caps[1].trim();
            // Look up color key by display name
            if let Some(color_key) = settings.get_color_key_by_display_name(category_name) {
                modifiers.colors.push(color_key);
                modifiers.color_target = ColorTarget::Note;
            }
        }

        // category:important tag (notes with tags in category named "important")
        if let Some(caps) = Regex::new(r"(?i)\bcategory:([\w-]+)\s+tag\b")
            .ok()
            .and_then(|re| re.captures(query))
        {
            let category_name = caps[1].trim();
            if let Some(color_key) = settings.get_color_key_by_display_name(category_name) {
                modifiers.colors.push(color_key);
                modifiers.color_target = ColorTarget::Tag;
            }
        }

        // category:important (both notes and tags)
        if modifiers.colors.is_empty() {
            if let Some(caps) = Regex::new(r"(?i)\bcategory:([\w-]+)\b")
                .ok()
                .and_then(|re| re.captures(query))
            {
                let category_name = caps[1].trim();
                if let Some(color_key) = settings.get_color_key_by_display_name(category_name) {
                    modifiers.colors.push(color_key);
                    modifiers.color_target = ColorTarget::Both;
                }
            }
        }

        modifiers
    }

    /// Remove search modifiers from query string, leaving only text/tag search terms
    fn remove_modifiers_from_query(query: &str) -> String {
        use regex::Regex;

        let mut result = query.to_string();

        // Remove all modifier patterns
        let patterns = [
            r"has:attachment",
            r"created:>\d{4}-\d{2}-\d{2}",
            r"created:<\d{4}-\d{2}-\d{2}",
            r"created:\d{4}-\d{2}-\d{2}\.\.\d{4}-\d{2}-\d{2}",
            r"modified:>\d{4}-\d{2}-\d{2}",
            r"modified:<\d{4}-\d{2}-\d{2}",
            r"modified:\d{4}-\d{2}-\d{2}\.\.\d{4}-\d{2}-\d{2}",
            r"words:>\d+",
            r"words:<\d+",
            r"words:\d+\.\.\d+",
            r"(?i)\bcolou?r:[a-z0-9#]+\s+note\b",
            r"(?i)\bcolou?r:[a-z0-9#]+\s+tag\b",
            r"(?i)\bcolou?r:[a-z0-9#]+\b",
            r"(?i)\bcategory:[\w-]+\s+note\b",
            r"(?i)\bcategory:[\w-]+\s+tag\b",
            r"(?i)\bcategory:[\w-]+\b",
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                result = re.replace_all(&result, "").to_string();
            }
        }

        // Clean up extra whitespace
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Trigger manual sync (delegates to operations module)
    pub fn trigger_sync(&mut self) {
        crate::ui::operations::sync::trigger_sync(self);
    }

    /// Check if auto-sync should run and trigger it if needed
    /// Also handles auto-clearing sync status after timeout
    /// Call this periodically (e.g., on Tick events) to enable background sync
    pub fn check_auto_sync(&mut self) {
        // Auto-clear sync status after 5 seconds
        if let Some(set_at) = self.sync_status_set_at {
            let now = Instant::now();
            let elapsed = now.duration_since(set_at);
            if elapsed >= std::time::Duration::from_secs(5) {
                self.sync_status = None;
                self.sync_status_set_at = None;
            }
        }

        // Check if auto-sync is enabled
        if self.settings.auto_sync_interval_minutes <= 0 {
            return; // Auto-sync disabled
        }

        // Check if sync is configured
        if !self.settings.sync_enabled || self.settings.sync_endpoint.is_none() {
            return; // Sync not configured
        }

        // Check if we're unlocked (have database and key)
        if self.db.is_none() || self.key.is_none() {
            return; // Not unlocked, can't sync
        }

        // Check time since last auto-sync
        let now = Instant::now();
        let should_sync = match self.last_auto_sync {
            None => true, // Never synced, do it now
            Some(last) => {
                let elapsed = now.duration_since(last);
                let interval = std::time::Duration::from_secs(
                    (self.settings.auto_sync_interval_minutes as u64) * 60
                );
                elapsed >= interval
            }
        };

        if should_sync {
            self.debug_log("Auto-sync: triggering scheduled sync");
            // Trigger sync (this will update sync_status)
            self.trigger_sync();
            // Update last auto-sync time
            self.last_auto_sync = Some(now);
        }

        // Check for SSE messages (real-time sync notifications)
        self.check_sse_messages();
    }

    /// Start SSE connection if sync is enabled
    pub fn start_sse_if_enabled(&mut self) {
        // Only start if sync is enabled and we have an endpoint
        if !self.settings.sync_enabled {
            return;
        }

        let endpoint = match &self.settings.sync_endpoint {
            Some(ep) => ep.clone(),
            None => return,
        };

        // Get the API key (need to decrypt it)
        let db = match &self.db {
            Some(db) => db,
            None => return,
        };

        let key = match &self.key {
            Some(k) => k,
            None => return,
        };

        let sync_repo = SyncRepository::new(db.connection());
        let metadata = match sync_repo.get_metadata() {
            Ok(Some(m)) => m,
            _ => return,
        };

        let encrypted_api_key = match &metadata.api_key {
            Some(k) => k,
            None => return,
        };

        // Decrypt the API key
        let encrypted: crate::crypto::EncryptedData = match serde_json::from_str(encrypted_api_key) {
            Ok(e) => e,
            Err(_) => return,
        };

        let api_key = match self.crypto.decrypt_text(&encrypted, key) {
            Ok(k) => k,
            Err(_) => return,
        };

        // Stop existing SSE connection if any
        self.stop_sse();

        // Start new SSE connection
        self.debug_log(&format!("SSE: Starting connection to {}", endpoint));
        let config = crate::sse::SseConfig {
            endpoint,
            api_key,
        };

        self.sse_handle = Some(crate::sse::start_sse_thread(config));
    }

    /// Stop SSE connection
    pub fn stop_sse(&mut self) {
        if let Some(mut handle) = self.sse_handle.take() {
            self.debug_log("SSE: Stopping connection");
            handle.stop();
        }
    }

    /// Check for SSE messages and handle them
    fn check_sse_messages(&mut self) {
        // Collect all pending messages first to avoid borrow issues
        let messages: Vec<crate::sse::SseMessage> = {
            match &self.sse_handle {
                Some(h) => {
                    let mut msgs = Vec::new();
                    while let Some(msg) = h.try_recv() {
                        msgs.push(msg);
                    }
                    msgs
                }
                None => return,
            }
        };

        // Now process the collected messages
        let mut should_sync = false;
        for msg in messages {
            match msg {
                crate::sse::SseMessage::SyncNotification => {
                    self.debug_log("SSE: Received sync notification, triggering sync");
                    should_sync = true;
                }
                crate::sse::SseMessage::Connected => {
                    self.debug_log("SSE: Connected to server");
                }
                crate::sse::SseMessage::Disconnected => {
                    self.debug_log("SSE: Disconnected from server (will reconnect)");
                }
                crate::sse::SseMessage::Failed(err) => {
                    self.debug_log(&format!("SSE: Connection failed: {}", err));
                    // Don't restart here - the SSE thread handles retries
                }
            }
        }

        // Trigger sync if we received a notification
        if should_sync {
            self.trigger_sync();
        }
    }

    /// Save settings to database
    fn save_settings(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            let settings_repo = SettingsRepository::new(db.connection());
            settings_repo.update(&self.settings)?;
        }
        Ok(())
    }

    /// Paste sync credentials from clipboard
    #[allow(dead_code)]
    fn paste_sync_credentials(&mut self) -> Result<()> {
        // Get clipboard content
        let mut clipboard = arboard::Clipboard::new()
            .context("Failed to access clipboard")?;
        let clipboard_text = clipboard.get_text()
            .context("Failed to read from clipboard")?;

        // Decode credentials
        let creds = SyncCredentials::from_base64(clipboard_text.trim())
            .context("Invalid sync credentials format")?;

        self.debug_log(&format!("Paste credentials - endpoint: {}", creds.endpoint));
        self.debug_log(&format!("Paste credentials - client_id: {}", creds.client_id));
        self.debug_log(&format!("Paste credentials - has salt: {}", creds.salt.is_some()));

        // Get database
        let db = self.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

        // If web app salt is provided, update it first
        // We'll encrypt the API key AFTER the user unlocks with the new salt
        if let Some(salt_b64) = &creds.salt {
            use base64::Engine;
            use crate::repository::encryption::EncryptionRepository;
            let encryption_repo = EncryptionRepository::new(db.connection());

            // Decode the base64 salt from web app
            let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64)
                .context("Invalid base64 salt from sync credentials")?;

            self.debug_log(&format!("Paste credentials - Salt (base64): {}", salt_b64));
            self.debug_log(&format!("Paste credentials - Salt (hex): {}", hex::encode(&salt)));
            self.debug_log(&format!("Paste credentials - Salt length: {} bytes", salt.len()));

            // Validate salt length - must be at least 32 bytes (256 bits) for PBKDF2
            if salt.len() < 32 {
                anyhow::bail!("Invalid salt length: {} bytes (expected at least 32 bytes). Web app salt may be incompatible with TUI.", salt.len());
            }

            // Update encryption metadata with web app's salt AND iteration count
            self.debug_log(&format!("Paste credentials - Saving salt with {} iterations", crate::crypto::DEFAULT_ITERATIONS));
            encryption_repo.save(&salt, crate::crypto::DEFAULT_ITERATIONS)?;
            self.debug_log("Paste credentials - Salt saved successfully");
        }

        // Save sync metadata
        let sync_repo = SyncRepository::new(db.connection());
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

        // If salt was synced, we need to store plaintext and encrypt on next unlock
        // because the key will be re-derived with the new salt.
        // Otherwise, encrypt immediately with the current session key if available.
        let api_key_to_store = if creds.salt.is_some() {
            // Salt changed - must wait for next unlock to encrypt with new key
            self.debug_log("Paste credentials - Storing API key (will encrypt on next unlock with new salt)");
            format!("PLAINTEXT:{}", creds.api_key)
        } else if let Some(ref key) = self.key {
            // No salt change and we have a session key - encrypt immediately
            self.debug_log("Paste credentials - Encrypting API key immediately");
            let encrypted = self.crypto.encrypt_text(&creds.api_key, key)
                .context("Failed to encrypt API key")?;
            serde_json::to_string(&encrypted)
                .context("Failed to serialize encrypted API key")?
        } else {
            // No session key available (shouldn't happen in normal flow)
            self.debug_log("Paste credentials - No session key, storing plaintext temporarily");
            format!("PLAINTEXT:{}", creds.api_key)
        };

        metadata.api_key = Some(api_key_to_store);
        metadata.client_id = Some(creds.client_id);
        metadata.sync_endpoint = creds.endpoint.clone();
        metadata.sync_enabled = true;

        sync_repo.update_metadata(&metadata)?;

        // Update settings
        self.settings.sync_endpoint = Some(creds.endpoint);
        self.settings.sync_enabled = true;
        self.save_settings()?;

        // If web app salt was provided, we need to lock and force re-unlock with the new salt
        // This ensures the user knows the salt was changed and re-enters their password
        if creds.salt.is_some() {
            self.debug_log("Paste credentials - Locking database to force re-unlock with new salt");

            // Stop SSE connection before locking
            self.stop_sse();

            // Automatically lock the database
            self.key = None;
            self.notes.clear();
            self.selected_note = 0;
            self.password_input.zeroize();
            self.password_confirm.zeroize();
            self.input_mode = InputMode::Normal;
            self.state = AppState::Locked;

            // Show message about what happened
            self.error = Some(t!("sync.salt_sync").to_string());
        }

        Ok(())
    }

    /// Copy sync credentials to clipboard
    #[allow(dead_code)]
    fn copy_sync_credentials(&mut self) -> Result<()> {
        // Get sync metadata
        if let Some(db) = &self.db {
            let sync_repo = SyncRepository::new(db.connection());
            let metadata = sync_repo.get_metadata()?
                .ok_or_else(|| anyhow::anyhow!("No sync configuration found"))?;

            // Check if credentials exist
            let encrypted_api_key = metadata.api_key
                .ok_or_else(|| anyhow::anyhow!("No API key configured. Enable sync first."))?;
            let client_id = metadata.client_id
                .ok_or_else(|| anyhow::anyhow!("No client ID found. Enable sync first."))?;

            // Decrypt API key
            let api_key = if let Some(key) = &self.key {
                let encrypted: crate::crypto::EncryptedData = serde_json::from_str(&encrypted_api_key)?;
                self.crypto.decrypt_text(&encrypted, key)?
            } else {
                anyhow::bail!("Database not unlocked");
            };

            // Create credentials payload
            let creds = SyncCredentials::new(
                metadata.sync_endpoint,
                api_key,
                client_id,
            );

            // Encode to base64
            let encoded = creds.to_base64()?;

            // Copy to clipboard
            let mut clipboard = arboard::Clipboard::new()
                .context("Failed to access clipboard")?;
            clipboard.set_text(&encoded)
                .context("Failed to write to clipboard")?;
        } else {
            anyhow::bail!("Database not available");
        }

        Ok(())
    }


    /// Check if terminal needs redraw and reset flag
    pub fn should_redraw(&mut self) -> bool {
        if self.need_redraw {
            self.need_redraw = false;
            true
        } else {
            false
        }
    }

    /// Render the UI
    pub fn render(&mut self, frame: &mut Frame) {
        self.debug_log(&format!("render() called, state={:?}", self.state));
        match &self.state {
            AppState::Locked => {
                self.debug_log("Rendering locked screen");
                rendering::locked::render_locked(self, frame)
            },
            AppState::NoteList => {
                self.debug_log("Rendering note list");
                rendering::note_list::render_note_list(self, frame);
                self.debug_log("Note list rendered successfully");
            },
            AppState::NoteView => rendering::note_view::render_note_view(self, frame),
            AppState::Settings { .. } => rendering::settings::render_settings(self, frame),
            AppState::Help { .. } => rendering::help::render_help(self, frame),
            AppState::ShowSyncCredentials { credentials, .. } => {
                rendering::credentials::render_show_credentials(self, frame, credentials)
            }
            AppState::InputSyncCredentials { .. } => rendering::credentials::render_input_credentials(self, frame),
            AppState::ImportInputDeviceName { .. } => rendering::credentials::render_import_device_name(self, frame),
            AppState::InputEmailForStatus { .. } => rendering::credentials::render_input_email_for_status(self, frame),
            AppState::ShowRegistrationStatus { status_message, .. } => {
                rendering::credentials::render_registration_status(self, frame, status_message)
            }
            AppState::RegisterInputEndpoint { .. } => {
                rendering::credentials::render_register_input_endpoint(self, frame)
            }
            AppState::RegisterInputEmail { .. } => {
                rendering::credentials::render_register_input_email(self, frame)
            }
            AppState::RegisterInputPassword { .. } => {
                rendering::credentials::render_register_input_password(self, frame)
            }
            AppState::RegisterPendingApproval { email, .. } => {
                let email = email.clone();
                rendering::credentials::render_register_pending_approval(self, frame, &email)
            }
            AppState::RegisterInputDeviceName { .. } => {
                rendering::credentials::render_register_input_device_name(self, frame)
            }
            AppState::Quit => {}
        }
    }

    /// Check if app should quit
    pub fn should_quit(&self) -> bool {
        matches!(self.state, AppState::Quit)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{UserSettings, ColorPalette};
    use std::collections::HashMap;

    fn create_test_settings_with_colors() -> UserSettings {
        let mut settings = UserSettings::default();

        // Add tag colors
        let mut tag_colors = HashMap::new();
        tag_colors.insert("work".to_string(), "blue".to_string());
        tag_colors.insert("urgent".to_string(), "red".to_string());
        settings.tag_colors = Some(tag_colors);

        // Customize color palette with display names
        let mut palette: ColorPalette = settings.get_color_palette().clone();
        if let Some(red_def) = palette.get_mut("red") {
            red_def.display_name = Some("important".to_string());
        }
        settings.color_palette = Some(palette);

        settings
    }

    #[test]
    fn test_parse_color_search_simple() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("color:red", &settings);

        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Both);
    }

    #[test]
    fn test_parse_color_search_note_qualifier() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("color:blue note", &settings);

        assert_eq!(modifiers.colors, vec!["blue"]);
        assert_eq!(modifiers.color_target, ColorTarget::Note);
    }

    #[test]
    fn test_parse_color_search_tag_qualifier() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("color:green tag", &settings);

        assert_eq!(modifiers.colors, vec!["green"]);
        assert_eq!(modifiers.color_target, ColorTarget::Tag);
    }

    #[test]
    fn test_parse_category_search_simple() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("category:important", &settings);

        // "important" should map to "red" via display name
        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Both);
    }

    #[test]
    fn test_parse_category_search_note_qualifier() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("category:important note", &settings);

        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Note);
    }

    #[test]
    fn test_parse_category_search_tag_qualifier() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("category:important tag", &settings);

        assert_eq!(modifiers.colors, vec!["red"]);
        assert_eq!(modifiers.color_target, ColorTarget::Tag);
    }

    #[test]
    fn test_parse_category_search_invalid() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("category:nonexistent", &settings);

        // Should not find any colors for nonexistent category
        assert!(modifiers.colors.is_empty());
    }

    #[test]
    fn test_parse_single_color_modifier() {
        let settings = create_test_settings_with_colors();
        let modifiers = App::parse_search_modifiers("color:red some text", &settings);

        // Should capture the color
        assert_eq!(modifiers.colors.len(), 1);
        assert_eq!(modifiers.colors[0], "red");
        assert_eq!(modifiers.color_target, ColorTarget::Both);
    }

    #[test]
    fn test_remove_modifiers_from_query() {
        let cleaned = App::remove_modifiers_from_query("color:red important meeting");
        assert!(!cleaned.contains("color:red"));
        assert!(cleaned.contains("important"));
        assert!(cleaned.contains("meeting"));
    }

    #[test]
    fn test_remove_category_modifiers_from_query() {
        let cleaned = App::remove_modifiers_from_query("category:important urgent task");
        assert!(!cleaned.contains("category:important"));
        assert!(cleaned.contains("urgent"));
        assert!(cleaned.contains("task"));
    }

    #[test]
    fn test_sort_notes_by_pinned_and_date() {
        use chrono::{TimeZone, Utc};

        // Create notes with varying pinned status and dates
        let mut note1 = Note::new("Note 1".to_string());
        note1.pinned = false;
        note1.modified_at = Utc.with_ymd_and_hms(2024, 1, 1, 12, 0, 0).unwrap();

        let mut note2 = Note::new("Note 2 - pinned".to_string());
        note2.pinned = true;
        note2.modified_at = Utc.with_ymd_and_hms(2024, 1, 2, 12, 0, 0).unwrap();

        let mut note3 = Note::new("Note 3".to_string());
        note3.pinned = false;
        note3.modified_at = Utc.with_ymd_and_hms(2024, 1, 3, 12, 0, 0).unwrap();

        let mut note4 = Note::new("Note 4 - pinned older".to_string());
        note4.pinned = true;
        note4.modified_at = Utc.with_ymd_and_hms(2024, 1, 1, 10, 0, 0).unwrap();

        let mut notes: Vec<&Note> = vec![&note1, &note2, &note3, &note4];
        App::sort_notes_by_pinned_and_date(&mut notes);

        // Pinned notes should come first, sorted by date descending
        assert!(notes[0].pinned);
        assert!(notes[1].pinned);
        // Note2 is more recent than Note4, so Note2 should be first
        assert!(notes[0].content.contains("Note 2"));
        assert!(notes[1].content.contains("Note 4"));

        // Unpinned notes should come after, sorted by date descending
        assert!(!notes[2].pinned);
        assert!(!notes[3].pinned);
        // Note3 is more recent than Note1
        assert!(notes[2].content.contains("Note 3"));
        assert!(notes[3].content.contains("Note 1"));
    }
}
