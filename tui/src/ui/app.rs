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
use std::{
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
    models::{Attachment, Note, UserSettings, sync::{SyncCredentials, ConflictData}},
    repository::{attachment::AttachmentRepository, EncryptionRepository, NoteRepository, SettingsRepository, sync::SyncRepository},
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
    /// Bulk tags input buffer (for adding tags to selected notes)
    pub bulk_tags_input: String,
    /// Bulk export path input buffer
    pub bulk_export_path_input: String,
    /// Show bulk delete confirmation modal
    pub show_bulk_delete_confirm: bool,
    /// Show bulk combine confirmation modal
    pub show_bulk_combine_confirm: bool,
    /// Rendered area for note list (for mouse click detection)
    pub note_list_area: Option<ratatui::layout::Rect>,
    /// Rendered area for tags line in preview (for mouse click detection)
    pub tags_line_area: Option<ratatui::layout::Rect>,
    /// Individual tag positions for click detection: (tag_name, x_start, x_end)
    pub tag_positions: Vec<(String, u16, u16)>,
}


impl App {
    /// Create a new app
    pub fn new(db_path: PathBuf, debug_log: Option<Arc<Mutex<File>>>) -> Result<Self> {
        let is_new_database = !db_path.exists();

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
            bulk_tags_input: String::new(),
            bulk_export_path_input: String::new(),
            show_bulk_delete_confirm: false,
            show_bulk_combine_confirm: false,
            note_list_area: None,
            tags_line_area: None,
            tag_positions: Vec::new(),
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
            AppState::InputEmailForStatus { .. } => input::credentials::handle_input_email_for_status_key(self, key)?,
            AppState::ShowRegistrationStatus { .. } => input::credentials::handle_show_registration_status_key(self, key)?,
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

        // Debug logging for troubleshooting
        self.debug_log(&format!("Unlock - Salt (hex): {}", hex::encode(&salt)));
        self.debug_log(&format!("Unlock - Salt length: {} bytes", salt.len()));
        self.debug_log(&format!("Unlock - Iterations: {}", iterations));
        self.debug_log(&format!("Unlock - Key (first 8 bytes): {}", hex::encode(&key[0..8])));

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
            // Update color scheme from loaded settings
            self.color_scheme = crate::ui::ColorScheme::by_name(self.settings.theme.scheme_name());
        }

        // Store password if remember checkbox was enabled
        if self.remember_password_checkbox {
            // The password is still in self.password_input at this point
            let password_to_store = self.password_input.clone();
            if let Err(e) = self.store_password_for_autounlock(&password_to_store) {
                self.error = Some(format!("Failed to store password: {}", e));
            } else {
                self.sync_status = Some(t!("password.remember_enabled").to_string());
                self.sync_status_set_at = Some(Instant::now());
            }
        }

        // Clear password fields and reset flags
        self.password_input.clear();
        self.password_confirm.clear();
        self.is_new_database = false;  // Database now exists
        self.password_confirm_focused = false;  // Reset focus
        self.remember_password_checkbox = false;  // Reset checkbox
        self.state = AppState::NoteList;

        Ok(())
    }

    /// Attempt to auto-unlock using stored password
    /// Returns Ok(true) if successfully unlocked, Ok(false) if no stored password, Err on failure
    pub fn try_auto_unlock(&mut self) -> Result<bool> {
        // First, we need to read settings with a dummy password just to check if remember_password is enabled
        // This is a chicken-and-egg problem: we need the password to read settings, but settings contain the password!
        // Solution: Use a constant "bootstrap" password to encrypt the stored_password field specifically

        // For now, try to open database with empty password to see if it exists
        if !self.db_path.exists() {
            return Ok(false); // New database, can't auto-unlock
        }

        // Try to open with a known constant to read settings (this will fail but we'll handle it)
        // Actually, this won't work with SQLCipher. We need a different approach.
        // The password must be stored in a separate unencrypted file.

        // Check for stored password in a separate config file
        let config_dir = self.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let remember_file = config_dir.join(".jottery_remember");

        if !remember_file.exists() {
            return Ok(false); // No stored password
        }

        // Read and decrypt stored password
        let encrypted_password = std::fs::read_to_string(&remember_file)
            .context("Failed to read stored password file")?;

        if encrypted_password.trim().is_empty() {
            return Ok(false);
        }

        // Decrypt using device-specific constant key
        let device_key = self.get_device_key();
        let encrypted_data: crate::crypto::EncryptedData = serde_json::from_str(&encrypted_password)
            .context("Failed to parse stored password")?;
        let password = self.crypto.decrypt_text(&encrypted_data, &device_key)
            .context("Failed to decrypt stored password")?;

        // Try to unlock with this password
        self.password_input = password;
        match self.unlock() {
            Ok(()) => {
                self.debug_log("Auto-unlock successful");
                Ok(true)
            }
            Err(e) => {
                self.password_input.clear();
                // Delete invalid stored password file
                let _ = std::fs::remove_file(&remember_file);
                Err(e).context("Auto-unlock failed")
            }
        }
    }

    /// Get device-specific encryption key for storing password
    /// WARNING: This is not cryptographically secure, just obfuscation
    fn get_device_key(&self) -> [u8; 32] {
        // Use a constant key derived from app name and version
        // Anyone with access to the code can decrypt this
        // The security warning makes this clear to users
        let constant = b"jottery-tui-device-key-v1.0.0---";
        let mut key = [0u8; 32];
        key.copy_from_slice(&constant[..32]);
        key
    }

    /// Enable/disable remember password feature
    /// When enabling, encrypts and stores the current password
    #[allow(dead_code)]
    pub fn toggle_remember_password(&mut self) -> Result<()> {
        if self.settings.remember_password {
            // Disable: clear stored password
            self.settings.remember_password = false;
            self.settings.stored_password = None;

            // Delete remember file
            let config_dir = self.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
            let remember_file = config_dir.join(".jottery_remember");
            let _ = std::fs::remove_file(&remember_file);

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
        // Encrypt password with device key
        let device_key = self.get_device_key();
        let encrypted = self.crypto.encrypt_text(password, &device_key)?;
        let encrypted_json = serde_json::to_string(&encrypted)?;

        // Save to remember file
        let config_dir = self.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let remember_file = config_dir.join(".jottery_remember");
        std::fs::write(&remember_file, &encrypted_json)
            .context("Failed to write password storage file")?;

        // Update settings
        self.settings.remember_password = true;
        self.settings.stored_password = Some(encrypted_json);

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
                sorted_notes.sort_by(|a, b| {
                    match (a.pinned, b.pinned) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => b.modified_at.cmp(&a.modified_at),
                    }
                });

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
                sorted_notes.sort_by(|a, b| {
                    match (a.pinned, b.pinned) {
                        (true, false) => std::cmp::Ordering::Less,
                        (false, true) => std::cmp::Ordering::Greater,
                        _ => b.modified_at.cmp(&a.modified_at),
                    }
                });
                self.selected_note_id = sorted_notes.first().map(|n| n.id.clone());
                self.debug_log("load_notes: no selected_note_id, defaulting to 0");
            }
        }
        Ok(())
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

    /// Filter notes based on search query and sort (pinned first, then by modified date)
    pub fn filtered_notes(&self) -> Vec<&Note> {
        let mut notes: Vec<&Note> = if self.search_input.is_empty() {
            self.notes.iter().collect()
        } else {
            let query = self.search_input.to_lowercase();

            // Parse advanced modifiers from query
            let modifiers = Self::parse_search_modifiers(&query);
            let remaining_query = Self::remove_modifiers_from_query(&query);
            let query_parts: Vec<&str> = remaining_query.split_whitespace().collect();

            self.notes
                .iter()
                .filter(|note| {
                    let content_lower = note.content.to_lowercase();

                    // Apply advanced modifiers first

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
                    for part in &query_parts {
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
                })
                .collect()
        };

        // Sort: pinned first, then by modified_at descending
        notes.sort_by(|a, b| {
            match (a.pinned, b.pinned) {
                (true, false) => std::cmp::Ordering::Less,
                (false, true) => std::cmp::Ordering::Greater,
                _ => b.modified_at.cmp(&a.modified_at),
            }
        });

        notes
    }

    /// Parse search modifiers from query string
    fn parse_search_modifiers(query: &str) -> SearchModifiers {
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
        ];

        for pattern in patterns {
            if let Ok(re) = Regex::new(pattern) {
                result = re.replace_all(&result, "").to_string();
            }
        }

        // Clean up extra whitespace
        result.split_whitespace().collect::<Vec<_>>().join(" ")
    }

    /// Trigger manual sync
    fn trigger_sync(&mut self) {
        self.debug_log("trigger_sync - Called");
        self.debug_log(&format!("trigger_sync - sync_enabled: {}", self.settings.sync_enabled));
        self.debug_log(&format!("trigger_sync - sync_endpoint: {:?}", self.settings.sync_endpoint));

        // Check if sync is configured
        if !self.settings.sync_enabled {
            self.debug_log("trigger_sync - Sync not enabled, returning");
            self.sync_status = Some(t!("sync.not_enabled").to_string());
            self.sync_status_set_at = Some(Instant::now());
            return;
        }

        if self.settings.sync_endpoint.is_none() {
            self.debug_log("trigger_sync - Sync endpoint not configured, returning");
            self.sync_status = Some(t!("sync.endpoint_not_configured").to_string());
            self.sync_status_set_at = Some(Instant::now());
            return;
        }

        // Perform sync
        self.debug_log("trigger_sync - Starting sync");
        self.sync_status = Some(t!("status.syncing").to_string());
        self.sync_status_set_at = Some(Instant::now());

        match self.perform_sync(false) {
            Ok(result) => {
                // Reload notes from database to pick up sync changes
                if let Err(e) = self.load_notes() {
                    self.error = Some(format!("Sync succeeded but failed to reload notes: {}", e));
                }
                let unit = if result == 1 { t!("sync.note").to_string() } else { t!("sync.notes").to_string() };
                self.sync_status = Some(t!("sync.complete", count = result, unit = unit).to_string());
                self.sync_status_set_at = Some(Instant::now());
            }
            Err(e) => {
                self.error = Some(format!("Sync failed: {}", e));
                self.sync_status = Some(format!("Sync failed: {}", e));
                self.sync_status_set_at = Some(Instant::now());
            }
        }
    }

    /// Perform bidirectional sync with server
    /// If force is true, pulls all notes from server regardless of last sync time
    fn perform_sync(&mut self, force: bool) -> Result<usize> {
        use crate::models::sync::{SyncPushRequest, SyncPullRequest, SyncNote, SyncPushResponse, SyncPullResponse};
        use crate::repository::sync::SyncRepository;
        use chrono::Utc;

        let db = self.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
        let key = self.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

        let sync_repo = SyncRepository::new(db.connection());
        let note_repo = NoteRepository::new(db.connection());
        let version_repo = crate::repository::NoteVersionRepository::new(db.connection());

        // Get sync metadata
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

        // Get API key
        let encrypted_api_key = metadata.api_key.as_ref()
            .ok_or_else(|| anyhow::anyhow!("No API key configured"))?;
        let api_key_encrypted: crate::crypto::EncryptedData = serde_json::from_str(encrypted_api_key)?;
        let api_key = self.crypto.decrypt_text(&api_key_encrypted, key)?;

        let endpoint = metadata.sync_endpoint.clone();

        // PUSH: Send local changes to server
        let last_sync = metadata.last_sync_at;
        let notes_to_push = if let Some(last_sync) = last_sync {
            note_repo.get_modified_after(last_sync, key)?
        } else {
            note_repo.list(false, key)?
        };

        let mut sync_count = 0;

        if !notes_to_push.is_empty() {
            use crate::models::sync::{AttachmentRef, SyncAttachment};
            use base64::{Engine as _, engine::general_purpose};

            // Collect all unique attachment IDs that need to be pushed
            let mut attachment_ids_to_push: std::collections::HashSet<String> = std::collections::HashSet::new();

            // Convert notes to sync format, encrypting content and tags
            let sync_notes: Result<Vec<SyncNote>> = notes_to_push.iter().map(|note| {
                // Encrypt content and tags for transmission to server
                let encrypted_content = self.crypto.encrypt_text(&note.content, key)?;
                let content_json = serde_json::to_string(&encrypted_content)?;

                let encrypted_tags: Result<Vec<String>> = note.tags.iter()
                    .map(|tag| {
                        // JSON-encode the tag first, then encrypt it
                        let tag_json = serde_json::to_string(tag)?;
                        let encrypted_tag = self.crypto.encrypt_text(&tag_json, key)?;
                        Ok(serde_json::to_string(&encrypted_tag)?)
                    })
                    .collect();

                // Build attachment references from note.attachments
                let attachment_refs: Vec<AttachmentRef> = note.attachments.iter().map(|att| {
                    attachment_ids_to_push.insert(att.id.clone());
                    AttachmentRef {
                        id: att.id.clone(),
                        filename: att.filename.clone(), // Already encrypted in database
                        mime_type: att.mime_type.clone(),
                        size: att.size,
                        data: att.data.clone(),
                    }
                }).collect();

                Ok(SyncNote {
                    id: note.id.clone(),
                    created_at: note.created_at,
                    modified_at: note.modified_at,
                    content: content_json,
                    tags: encrypted_tags?,
                    attachments: attachment_refs,
                    pinned: note.pinned,
                    deleted: note.deleted,
                    deleted_at: note.deleted_at,
                    version: note.version,
                    word_wrap: Some(note.word_wrap),
                    syntax_language: Some(note.syntax_language.to_string()),
                })
            }).collect();

            let sync_notes = sync_notes?;

            // Collect versions for all notes being pushed
            use crate::models::sync::SyncNoteVersion;
            let mut sync_versions: Vec<SyncNoteVersion> = Vec::new();

            for note in &notes_to_push {
                let note_versions = version_repo.get_versions_for_note(&note.id, key)?;

                for version in note_versions {
                    // Encrypt content and tags for transmission
                    let encrypted_content = self.crypto.encrypt_text(&version.content, key)?;
                    let content_json = serde_json::to_string(&encrypted_content)?;

                    let encrypted_tags: Result<Vec<String>> = version.tags.iter()
                        .map(|tag| {
                            let tag_json = serde_json::to_string(tag)?;
                            let encrypted_tag = self.crypto.encrypt_text(&tag_json, key)?;
                            Ok(serde_json::to_string(&encrypted_tag)?)
                        })
                        .collect();

                    // Build attachment references
                    let attachment_refs: Vec<AttachmentRef> = version.attachments.iter().map(|att| {
                        AttachmentRef {
                            id: att.id.clone(),
                            filename: att.filename.clone(), // Already encrypted
                            mime_type: att.mime_type.clone(),
                            size: att.size,
                            data: att.data.clone(),
                        }
                    }).collect();

                    sync_versions.push(SyncNoteVersion {
                        version_key: format!("{}:{}", version.note_id, version.version),
                        note_id: version.note_id.clone(),
                        version: version.version,
                        created_at: version.created_at,
                        synced_at: version.synced_at,
                        content: content_json,
                        tags: encrypted_tags?,
                        attachments: attachment_refs,
                        syntax_language: version.syntax_language.as_ref().map(|s| s.to_string()),
                        word_wrap: version.word_wrap,
                        reason: version.reason.to_string(),
                    });
                }
            }

            // Fetch binary data for all attachments that need to be pushed
            let attachment_repo = AttachmentRepository::new(db.connection());
            let sync_attachments: Result<Vec<SyncAttachment>> = attachment_ids_to_push.iter().map(|att_id| {
                // Get encrypted binary data from database
                let (_filename, _mime_type, _size, encrypted_data) = attachment_repo
                    .get(att_id, key)?
                    .context(format!("Attachment {} not found", att_id))?;

                // Re-encrypt and base64 encode for transmission
                let encrypted_blob = self.crypto.encrypt_binary(&encrypted_data, key)?;
                let base64_data = general_purpose::STANDARD.encode(serde_json::to_vec(&encrypted_blob)?);

                Ok(SyncAttachment {
                    id: att_id.clone(),
                    data: base64_data,
                })
            }).collect();

            let push_request = SyncPushRequest {
                notes: sync_notes,
                attachments: sync_attachments?,
                versions: sync_versions,
            };

            // Create HTTP client
            let client = reqwest::blocking::Client::new();
            let push_url = format!("{}/api/v1/sync/push", endpoint);

            let response = client
                .post(&push_url)
                .header("Authorization", format!("Bearer {}", api_key))
                .json(&push_request)
                .send()
                .context("Failed to send push request")?;

            if !response.status().is_success() {
                let status = response.status();
                let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

                // Provide user-friendly error messages
                let error_msg = if status == 403 {
                    "Your account has been deactivated or is pending admin approval. Please contact the administrator."
                } else if status == 401 {
                    "Invalid API key or authentication failed. Try re-registering your device."
                } else if status == 409 {
                    "Sync conflict detected. Some notes have conflicting changes on the server."
                } else {
                    &error_body
                };

                anyhow::bail!("Push failed: {}", error_msg);
            }

            let push_response: SyncPushResponse = response.json()
                .context("Failed to parse push response")?;

            sync_count += push_response.accepted.len();

            // Create version snapshots for accepted notes
            for accepted in &push_response.accepted {
                if let Ok(Some(note)) = note_repo.get(&accepted.id, key) {
                    let _ = version_repo.create_version(&note, accepted.synced_at, crate::repository::VersionReason::Sync, key);
                }
            }

            // Update last push timestamp
            metadata.last_push_at = Some(Utc::now());
        }

        // PULL: Get changes from server
        let (last_sync_for_pull, known_note_ids, known_attachment_ids) = if force {
            // Force full sync: request all notes and attachments from server
            (None, vec![], vec![])
        } else {
            // Normal sync: use last sync time, known note IDs, and known attachment IDs
            let known_ids = self.notes.iter().map(|n| n.id.clone()).collect();

            // Collect all attachment IDs we already have locally
            let known_att_ids: Vec<String> = self.notes.iter()
                .flat_map(|note| note.attachments.iter().map(|att| att.id.clone()))
                .collect();

            (last_sync, known_ids, known_att_ids)
        };

        let pull_request = SyncPullRequest {
            last_sync_at: last_sync_for_pull,
            known_note_ids,
            known_attachment_ids,
        };

        let pull_url = format!("{}/api/v1/sync/pull", endpoint);
        let client = reqwest::blocking::Client::new();

        let response = client
            .post(&pull_url)
            .header("Authorization", format!("Bearer {}", api_key))
            .json(&pull_request)
            .send()
            .context("Failed to send pull request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_body = response.text().unwrap_or_else(|_| "Unknown error".to_string());

            // Provide user-friendly error messages
            let error_msg = if status == 403 {
                "Your account has been deactivated or is pending admin approval. Please contact the administrator."
            } else if status == 401 {
                "Invalid API key or authentication failed. Try re-registering your device."
            } else {
                &error_body
            };

            anyhow::bail!("Pull failed: {}", error_msg);
        }

        // Parse the JSON response
        let response_text = response.text()
            .context("Failed to read pull response text")?;
        let pull_response: SyncPullResponse = serde_json::from_str(&response_text)
            .context("Failed to parse pull response")?;

        use base64::{Engine as _, engine::general_purpose};
        let attachment_repo = AttachmentRepository::new(db.connection());

        // Decrypt attachments and build a map for quick lookup
        let mut attachment_data_map: std::collections::HashMap<String, Vec<u8>> = std::collections::HashMap::new();
        for sync_attachment in &pull_response.attachments {
            if let Ok(decoded_data) = general_purpose::STANDARD.decode(&sync_attachment.data) {
                if let Ok(encrypted_blob) = serde_json::from_slice::<crate::crypto::EncryptedData>(&decoded_data) {
                    if let Ok(decrypted_data) = self.crypto.decrypt_binary(&encrypted_blob, key) {
                        attachment_data_map.insert(sync_attachment.id.clone(), decrypted_data);
                    }
                }
            }
        }

        for remote_note in pull_response.notes {
            // Decrypt content and tags from server (they're stored encrypted on server)
            let encrypted_content: crate::crypto::EncryptedData = serde_json::from_str(&remote_note.content)?;
            let decrypted_content = self.crypto.decrypt_text(&encrypted_content, key)?;

            let decrypted_tags: Vec<String> = remote_note.tags.iter()
                .flat_map(|tag_json| {
                    // Parse and decrypt the tag
                    let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json).ok()?;
                    let tag_json_str = self.crypto.decrypt_text(&encrypted_tag, key).ok()?;

                    // Try parsing as individual string first (new format)
                    if let Ok(tag) = serde_json::from_str::<String>(&tag_json_str) {
                        if !tag.trim().is_empty() {
                            return Some(vec![tag]);
                        }
                    }

                    // Try parsing as array (legacy format)
                    if let Ok(tags) = serde_json::from_str::<Vec<String>>(&tag_json_str) {
                        let valid_tags: Vec<String> = tags.into_iter()
                            .filter(|t| !t.trim().is_empty())
                            .collect();
                        if !valid_tags.is_empty() {
                            return Some(valid_tags);
                        }
                    }

                    None
                })
                .flatten()
                .collect();

            // Process attachments for this note
            let mut note_attachments: Vec<Attachment> = Vec::new();
            self.debug_log(&format!("Pull - Processing {} attachments for note {}", remote_note.attachments.len(), remote_note.id));

            for attachment_ref in &remote_note.attachments {
                self.debug_log(&format!("Pull - Processing attachment: {} ({})", attachment_ref.id, attachment_ref.mime_type));

                // Get decrypted binary data from our map
                if let Some(decrypted_data) = attachment_data_map.get(&attachment_ref.id) {
                    self.debug_log(&format!("Pull - Found attachment data in map, size: {} bytes", decrypted_data.len()));

                    // Decrypt the filename
                    let encrypted_filename: crate::crypto::EncryptedData = match serde_json::from_str(&attachment_ref.filename) {
                        Ok(data) => data,
                        Err(e) => {
                            self.debug_log(&format!("Pull - Failed to parse filename as JSON for {}: {}, raw: {:?}", attachment_ref.id, e, &attachment_ref.filename[..100.min(attachment_ref.filename.len())]));
                            continue;
                        }
                    };

                    let decrypted_filename = match self.crypto.decrypt_text(&encrypted_filename, key) {
                        Ok(filename) => filename,
                        Err(e) => {
                            self.debug_log(&format!("Pull - Failed to decrypt filename for {}: {}", attachment_ref.id, e));
                            continue;
                        }
                    };

                    // Parse the filename - try JSON first (new format), fall back to plain string (legacy format)
                    let filename: String = serde_json::from_str(&decrypted_filename)
                        .unwrap_or(decrypted_filename);

                    self.debug_log(&format!("Pull - Decrypted filename: {}", filename));

                    // Store in database
                    attachment_repo.store(
                        &attachment_ref.id,
                        &filename,
                        &attachment_ref.mime_type,
                        attachment_ref.size,
                        decrypted_data,
                        key
                    )?;

                    self.debug_log(&"Pull - Stored attachment in database".to_string());

                    // Add to note's attachment array
                    note_attachments.push(Attachment {
                        id: attachment_ref.id.clone(),
                        filename: filename.clone(),
                        mime_type: attachment_ref.mime_type.clone(),
                        size: attachment_ref.size,
                        data: attachment_ref.data.clone(),
                        thumbnail_data: None,
                    });

                    self.debug_log(&"Pull - Added attachment to note_attachments array".to_string());
                } else {
                    self.debug_log(&format!("Pull - Attachment data NOT found in map for {}", attachment_ref.id));
                }
            }

            self.debug_log(&format!("Pull - Total attachments added to note: {}", note_attachments.len()));

            // Check if we have this note in the database (not just in-memory list)
            let existing_note = note_repo.get(&remote_note.id, key)?;

            if let Some(mut local_note) = existing_note {
                // Note exists in database - check if we should update it
                self.debug_log(&format!("Pull - Existing note found: {}", remote_note.id));
                self.debug_log(&format!("  Remote modified_at: {}", remote_note.modified_at));
                self.debug_log(&format!("  Local modified_at: {}", local_note.modified_at));
                self.debug_log(&format!("  Remote > Local? {}", remote_note.modified_at > local_note.modified_at));
                self.debug_log(&format!("  Local attachments: {}, Remote attachments: {}", local_note.attachments.len(), note_attachments.len()));

                // Conflict resolution: Last-Write-Wins, but also update if attachments differ
                let should_update = remote_note.modified_at > local_note.modified_at
                    || note_attachments.len() != local_note.attachments.len();

                if should_update {
                    if remote_note.modified_at > local_note.modified_at {
                        self.debug_log("  -> Updating note (remote is newer)");
                    } else {
                        self.debug_log("  -> Updating note (attachments differ even though timestamps match)");
                    }
                    // Capture local version BEFORE overwriting with remote
                    let _ = version_repo.create_version(&local_note, pull_response.synced_at, crate::repository::VersionReason::Sync, key);

                    // Remote is newer, update local with decrypted content
                    local_note.content = decrypted_content;
                    local_note.tags = decrypted_tags;
                    local_note.attachments = note_attachments.clone();
                    local_note.modified_at = remote_note.modified_at;
                    local_note.pinned = remote_note.pinned;
                    local_note.deleted = remote_note.deleted;
                    local_note.deleted_at = remote_note.deleted_at;
                    local_note.version = remote_note.version;
                    local_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
                    if let Some(lang_str) = &remote_note.syntax_language {
                        local_note.syntax_language = lang_str.parse().unwrap_or_default();
                    }

                    note_repo.update(&local_note, key)?;

                    // Also update in-memory list if present
                    if let Some(mem_note) = self.notes.iter_mut().find(|n| n.id == remote_note.id) {
                        *mem_note = local_note;
                    }

                    sync_count += 1;
                } else {
                    self.debug_log("  -> NOT updating note (local is same or newer) - ATTACHMENTS WILL BE LOST!");
                }
            } else {
                // New note from server, add it with decrypted content
                let mut new_note = Note::new(decrypted_content);
                new_note.id = remote_note.id.clone();
                new_note.created_at = remote_note.created_at;
                new_note.modified_at = remote_note.modified_at;
                new_note.tags = decrypted_tags;
                new_note.attachments = note_attachments;
                new_note.pinned = remote_note.pinned;
                new_note.deleted = remote_note.deleted;
                new_note.deleted_at = remote_note.deleted_at;
                new_note.version = remote_note.version;
                new_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
                if let Some(lang_str) = &remote_note.syntax_language {
                    new_note.syntax_language = lang_str.parse().unwrap_or_default();
                }

                note_repo.create(&new_note, key)?;

                // Add to in-memory list only if not deleted
                if !new_note.deleted {
                    self.notes.insert(0, new_note);
                }

                sync_count += 1;
            }
        }

        // Process incoming versions from server
        self.debug_log(&format!("Pull - Received {} versions from server", pull_response.versions.len()));

        for server_version in &pull_response.versions {
            self.debug_log(&format!("Pull - Processing version: {} (v{})", server_version.version_key, server_version.version));

            // Check if this version already exists
            let existing_version = version_repo.get_version_by_key(&server_version.version_key)?;

            if existing_version.is_none() {
                // New version from server - decrypt and store it locally

                // Decrypt content
                let encrypted_content: crate::crypto::EncryptedData = match serde_json::from_str(&server_version.content) {
                    Ok(data) => data,
                    Err(e) => {
                        self.debug_log(&format!("Pull - Failed to parse version content: {}, skipping", e));
                        continue;
                    }
                };

                let decrypted_content = match self.crypto.decrypt_text(&encrypted_content, key) {
                    Ok(content) => content,
                    Err(e) => {
                        self.debug_log(&format!("Pull - Failed to decrypt version content: {}, skipping", e));
                        continue;
                    }
                };

                // Decrypt tags
                let decrypted_tags: Vec<String> = server_version.tags.iter()
                    .flat_map(|tag_json| {
                        let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json).ok()?;
                        let tag_json_str = self.crypto.decrypt_text(&encrypted_tag, key).ok()?;
                        serde_json::from_str::<String>(&tag_json_str).ok()
                    })
                    .collect();

                // Convert attachment refs
                let version_attachments: Vec<Attachment> = server_version.attachments.iter().map(|att_ref| {
                    Attachment {
                        id: att_ref.id.clone(),
                        filename: att_ref.filename.clone(),
                        mime_type: att_ref.mime_type.clone(),
                        size: att_ref.size,
                        data: att_ref.data.clone(),
                        thumbnail_data: None,
                    }
                }).collect();

                // Parse version reason
                let reason = if server_version.reason == "manual-sync" {
                    crate::repository::VersionReason::ManualSync
                } else {
                    crate::repository::VersionReason::Sync
                };

                // Create local version
                let local_version = crate::repository::NoteVersion {
                    version_key: server_version.version_key.clone(),
                    note_id: server_version.note_id.clone(),
                    version: server_version.version,
                    created_at: server_version.created_at,
                    synced_at: server_version.synced_at,
                    content: decrypted_content,
                    tags: decrypted_tags,
                    attachments: version_attachments,
                    syntax_language: server_version.syntax_language
                        .as_ref()
                        .and_then(|s| s.parse().ok()),
                    word_wrap: Some(server_version.word_wrap.unwrap_or(true)),
                    reason,
                };

                // Store the version
                if let Err(e) = version_repo.insert_version_from_sync(&local_version, key) {
                    self.debug_log(&format!("Pull - Failed to store version {}: {}", server_version.version_key, e));
                } else {
                    self.debug_log(&format!("Pull - Stored version from server: {}", server_version.version_key));
                }
            }
        }

        // Handle deletions
        for deletion in pull_response.deletions {
            if let Some(pos) = self.notes.iter().position(|n| n.id == deletion.id) {
                note_repo.delete(&deletion.id)?;
                self.notes.remove(pos);
                sync_count += 1;
            }
        }

        // Update sync metadata
        metadata.last_sync_at = Some(Utc::now());
        metadata.last_pull_at = Some(Utc::now());
        sync_repo.update_metadata(&metadata)?;

        // Reload notes to ensure UI is up to date
        self.load_notes()?;

        Ok(sync_count)
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
            self.debug_log("Paste credentials - Saving salt with 100,000 iterations");
            encryption_repo.save(&salt, 100_000)?;
            self.debug_log("Paste credentials - Salt saved successfully");
        }

        // Save sync metadata with PLAINTEXT API key temporarily
        // It will be encrypted on next unlock with the new salt
        let sync_repo = SyncRepository::new(db.connection());
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

        // Store API key as plaintext temporarily (will be encrypted on next unlock)
        // We use a special marker to indicate it needs encryption
        self.debug_log("Paste credentials - Storing API key (will encrypt on next unlock)");
        metadata.api_key = Some(format!("PLAINTEXT:{}", creds.api_key));
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

            // Automatically lock the database
            self.key = None;
            self.notes.clear();
            self.selected_note = 0;
            self.password_input.clear();
            self.password_confirm.clear();
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
        match &self.state {
            AppState::Locked => rendering::locked::render_locked(self, frame),
            AppState::NoteList => rendering::note_list::render_note_list(self, frame),
            AppState::NoteView => rendering::note_view::render_note_view(self, frame),
            AppState::Settings { .. } => rendering::settings::render_settings(self, frame),
            AppState::Help { .. } => rendering::help::render_help(self, frame),
            AppState::ShowSyncCredentials { credentials, .. } => {
                rendering::credentials::render_show_credentials(self, frame, credentials)
            }
            AppState::InputSyncCredentials { .. } => rendering::credentials::render_input_credentials(self, frame),
            AppState::InputEmailForStatus { .. } => rendering::credentials::render_input_email_for_status(self, frame),
            AppState::ShowRegistrationStatus { status_message, .. } => {
                rendering::credentials::render_registration_status(self, frame, status_message)
            }
            AppState::Quit => {}
        }
    }

    /// Check if app should quit
    pub fn should_quit(&self) -> bool {
        matches!(self.state, AppState::Quit)
    }
}
