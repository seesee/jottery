use anyhow::{Context, Result};
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::{
    env,
    fs::File,
    io::{self, Write},
    path::PathBuf,
    process::Command,
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;

use crate::{
    crypto::{CryptoService, KeyManager},
    db::Database,
    models::{Note, UserSettings, sync::SyncCredentials},
    repository::{EncryptionRepository, NoteRepository, SettingsRepository, sync::SyncRepository},
};

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
}

/// Application
pub struct App {
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
    /// Note content input buffer
    pub note_input: String,
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
    /// Selected settings field (0-5: language, theme, sort_order, auto_lock_timeout, sync_enabled, sync_endpoint)
    pub selected_setting: usize,
    /// Settings input buffer (for string/number fields)
    pub setting_input: String,
    /// Database path
    db_path: PathBuf,
    /// Database connection (when unlocked)
    db: Option<Database>,
    /// Master key (when unlocked)
    key: Option<[u8; 32]>,
    /// Key manager
    key_manager: KeyManager,
    /// Crypto service
    crypto: CryptoService,
    /// Loaded notes
    notes: Vec<Note>,
    /// Selected note index
    selected_note: usize,
    /// Currently editing note ID (None = creating new note)
    editing_note_id: Option<String>,
    /// Settings
    settings: UserSettings,
    /// Debug log file (for troubleshooting)
    debug_log: Option<Arc<Mutex<File>>>,
}

impl App {
    /// Create a new app
    pub fn new(db_path: PathBuf, debug_log: Option<Arc<Mutex<File>>>) -> Result<Self> {
        let is_new_database = !db_path.exists();

        Ok(Self {
            state: AppState::Locked,
            input_mode: InputMode::Normal,
            password_input: String::new(),
            password_confirm: String::new(),
            is_new_database,
            password_confirm_focused: false,
            note_input: String::new(),
            tag_input: String::new(),
            current_tags: Vec::new(),
            search_input: String::new(),
            search_active: false,
            sync_status: None,
            error: None,
            selected_setting: 0,
            setting_input: String::new(),
            db_path,
            db: None,
            key: None,
            key_manager: KeyManager::new(),
            crypto: CryptoService::new(),
            notes: Vec::new(),
            selected_note: 0,
            editing_note_id: None,
            settings: UserSettings::default(),
            debug_log,
        })
    }

    /// Write to debug log if enabled
    fn debug_log(&self, message: &str) {
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
            return self.handle_help_key(key);
        }
        if let AppState::Settings { .. } = &self.state {
            return self.handle_settings_key(key);
        }

        match &self.state {
            AppState::Locked => self.handle_locked_key(key)?,
            AppState::NoteList => self.handle_note_list_key(key)?,
            AppState::NoteView => self.handle_note_view_key(key)?,
            AppState::Quit => {}
            AppState::Settings { .. } => unreachable!(), // Handled above
            AppState::Help { .. } => unreachable!(), // Handled above
        }
        Ok(())
    }

    /// Handle key events in locked state
    fn handle_locked_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                self.state = AppState::Quit;
            }
            KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.state = AppState::Quit;
            }
            KeyCode::Tab if self.is_new_database => {
                // Switch between password and confirm fields
                self.password_confirm_focused = !self.password_confirm_focused;
            }
            KeyCode::Enter => {
                // Try to unlock/create
                self.error = None;

                // Validate password confirmation for new databases
                if self.is_new_database {
                    if self.password_input.is_empty() {
                        self.error = Some("Password cannot be empty".to_string());
                        return Ok(());
                    }
                    if self.password_input != self.password_confirm {
                        self.error = Some("Passwords do not match".to_string());
                        return Ok(());
                    }
                }

                if let Err(e) = self.unlock() {
                    self.error = Some(format!("Failed to unlock: {}", e));
                    self.password_input.clear();
                    self.password_confirm.clear();
                }
            }
            KeyCode::Char(c) => {
                if self.is_new_database && self.password_confirm_focused {
                    self.password_confirm.push(c);
                } else {
                    self.password_input.push(c);
                }
            }
            KeyCode::Backspace => {
                if self.is_new_database && self.password_confirm_focused {
                    self.password_confirm.pop();
                } else {
                    self.password_input.pop();
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events in note list state
    fn handle_note_list_key(&mut self, key: KeyEvent) -> Result<()> {
        // Clear sync status on any key (except 'y' which sets it)
        if key.code != KeyCode::Char('y') {
            self.sync_status = None;
        }

        // Handle search mode
        if self.search_active {
            match key.code {
                KeyCode::Esc => {
                    self.search_active = false;
                    self.search_input.clear();
                    self.selected_note = 0;
                }
                KeyCode::Enter => {
                    // Exit search and open selected note
                    if !self.filtered_notes().is_empty() {
                        let filtered = self.filtered_notes();
                        if self.selected_note < filtered.len() {
                            // Clone the data we need before modifying self
                            let content = filtered[self.selected_note].content.clone();
                            let tags = filtered[self.selected_note].tags.clone();
                            let note_id = filtered[self.selected_note].id.clone();

                            self.note_input = content;
                            self.current_tags = tags;
                            self.editing_note_id = Some(note_id);
                            self.state = AppState::NoteView;
                            self.input_mode = InputMode::Normal;
                            self.search_input.clear();
                            self.search_active = false;
                        }
                    }
                }
                KeyCode::Char(c) => {
                    self.search_input.push(c);
                    self.selected_note = 0; // Reset selection when search changes
                }
                KeyCode::Backspace => {
                    self.search_input.pop();
                    self.selected_note = 0;
                }
                KeyCode::Down => {
                    let filtered_count = self.filtered_notes().len();
                    if filtered_count > 0 && self.selected_note < filtered_count - 1 {
                        self.selected_note += 1;
                    }
                }
                KeyCode::Up => {
                    if self.selected_note > 0 {
                        self.selected_note -= 1;
                    }
                }
                _ => {}
            }
        } else {
            // Normal note list mode
            match key.code {
                KeyCode::Char('q') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    self.state = AppState::Quit;
                }
                KeyCode::Char('?') => {
                    // Show help
                    let prev = std::mem::replace(&mut self.state, AppState::Quit);
                    self.state = AppState::Help {
                        previous: Box::new(prev),
                    };
                }
                KeyCode::Char('s') => {
                    // Show settings
                    let prev = std::mem::replace(&mut self.state, AppState::Quit);
                    self.state = AppState::Settings {
                        previous: Box::new(prev),
                    };
                    self.input_mode = InputMode::Normal;
                    self.selected_setting = 0;
                    self.setting_input.clear();
                    self.error = None;
                }
                KeyCode::Char('y') => {
                    // Sync notes
                    self.trigger_sync();
                }
                KeyCode::Char('/') => {
                    // Enter search mode
                    self.search_active = true;
                    self.search_input.clear();
                }
                KeyCode::Char('n') => {
                    // New note
                    self.note_input.clear();
                    self.current_tags.clear();
                    self.editing_note_id = None;
                    self.state = AppState::NoteView;
                    self.input_mode = InputMode::Insert;
                }
                KeyCode::Char('i') | KeyCode::Enter => {
                    // Edit selected note
                    let filtered = self.filtered_notes();
                    if !filtered.is_empty() && self.selected_note < filtered.len() {
                        // Clone data before modifying self
                        let content = filtered[self.selected_note].content.clone();
                        let tags = filtered[self.selected_note].tags.clone();
                        let note_id = filtered[self.selected_note].id.clone();

                        self.note_input = content;
                        self.current_tags = tags;
                        self.editing_note_id = Some(note_id);
                        self.state = AppState::NoteView;
                        self.input_mode = InputMode::Normal;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    let note_count = self.filtered_notes().len();
                    if note_count > 0 && self.selected_note < note_count - 1 {
                        self.selected_note += 1;
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if self.selected_note > 0 {
                        self.selected_note -= 1;
                    }
                }
                KeyCode::Char('p') => {
                    // Toggle pin on selected note
                    let filtered = self.filtered_notes();
                    if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let note_id = filtered[self.selected_note].id.clone();
                        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                            note.pinned = !note.pinned;

                            // Save to database
                            if let (Some(db), Some(key)) = (&self.db, &self.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    self.error = Some(format!("Failed to update pin status: {}", e));
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('d') => {
                    // Delete selected note
                    let filtered = self.filtered_notes();
                    if !filtered.is_empty() && self.selected_note < filtered.len() {
                        // Find the actual note in the full list
                        let note_to_delete = filtered[self.selected_note];
                        if let Some(pos) = self.notes.iter().position(|n| n.id == note_to_delete.id) {
                            self.selected_note = pos;
                            self.delete_note()?;
                            // Adjust selection after delete
                            let new_count = self.filtered_notes().len();
                            if self.selected_note >= new_count && self.selected_note > 0 {
                                self.selected_note -= 1;
                            }
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Handle key events in note view state
    fn handle_note_view_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::SettingsEdit => {
                // Settings edit mode should not be active in note view
                // Reset to normal mode if somehow this happens
                self.input_mode = InputMode::Normal;
            }
            InputMode::Normal => match key.code {
                KeyCode::Char('i') => {
                    self.input_mode = InputMode::Insert;
                }
                KeyCode::Char('e') => {
                    // Edit with external $EDITOR
                    if let Ok(content) = self.edit_with_external_editor() {
                        self.note_input = content;
                    }
                }
                KeyCode::Char('t') => {
                    // Enter tag mode
                    self.tag_input.clear();
                    self.input_mode = InputMode::Tag;
                }
                KeyCode::Char('?') => {
                    // Show help
                    let prev = std::mem::replace(&mut self.state, AppState::Quit);
                    self.state = AppState::Help {
                        previous: Box::new(prev),
                    };
                }
                KeyCode::Char('q') | KeyCode::Esc => {
                    // Save and return to list
                    self.save_note()?;
                    self.load_notes()?;
                    self.state = AppState::NoteList;
                }
                _ => {}
            },
            InputMode::Insert => match key.code {
                KeyCode::Esc => {
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Char(c) => {
                    self.note_input.push(c);
                }
                KeyCode::Backspace => {
                    self.note_input.pop();
                }
                KeyCode::Delete => {
                    // For append-only editor, Delete acts like Backspace
                    self.note_input.pop();
                }
                KeyCode::Enter => {
                    self.note_input.push('\n');
                }
                _ => {}
            },
            InputMode::Tag => match key.code {
                KeyCode::Esc => {
                    // Exit tag mode
                    self.tag_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Enter => {
                    // Add tag
                    let tag = self.tag_input.trim().to_string();
                    if !tag.is_empty() && !self.current_tags.contains(&tag) {
                        self.current_tags.push(tag);
                    }
                    self.tag_input.clear();
                }
                KeyCode::Char(c) => {
                    self.tag_input.push(c);
                }
                KeyCode::Backspace => {
                    if self.tag_input.is_empty() && !self.current_tags.is_empty() {
                        // Remove last tag if input is empty
                        self.current_tags.pop();
                    } else {
                        self.tag_input.pop();
                    }
                }
                _ => {}
            },
        }
        Ok(())
    }

    /// Handle key events in help screen
    fn handle_help_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('?') => {
                // Return to previous state
                if let AppState::Help { previous } = std::mem::replace(&mut self.state, AppState::Quit) {
                    self.state = *previous;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events in settings screen
    fn handle_settings_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::Normal => {
                // Navigation mode
                match key.code {
                    KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('s') => {
                        // Return to previous state
                        if let AppState::Settings { previous } = std::mem::replace(&mut self.state, AppState::Quit) {
                            self.state = *previous;
                        }
                    }
                    KeyCode::Down | KeyCode::Char('j') => {
                        // Move down through settings fields
                        if self.selected_setting < 5 {
                            self.selected_setting += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        // Move up through settings fields
                        if self.selected_setting > 0 {
                            self.selected_setting -= 1;
                        }
                    }
                    KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                        // Edit selected field
                        self.start_editing_setting();
                    }
                    KeyCode::Char('p') => {
                        // Paste sync credentials from clipboard
                        if let Err(e) = self.paste_sync_credentials() {
                            self.error = Some(format!("Failed to paste credentials: {}", e));
                        } else {
                            self.sync_status = Some("Sync credentials pasted successfully!".to_string());
                        }
                    }
                    KeyCode::Char('c') => {
                        // Copy sync credentials to clipboard
                        if let Err(e) = self.copy_sync_credentials() {
                            self.error = Some(format!("Failed to copy credentials: {}", e));
                        } else {
                            self.sync_status = Some("Sync credentials copied to clipboard!".to_string());
                        }
                    }
                    KeyCode::Char('y') => {
                        // Trigger manual sync
                        self.trigger_sync();
                    }
                    _ => {}
                }
            }
            InputMode::SettingsEdit => {
                // Editing mode
                match key.code {
                    KeyCode::Esc => {
                        // Cancel editing
                        self.setting_input.clear();
                        self.input_mode = InputMode::Normal;
                    }
                    KeyCode::Enter => {
                        // Save edited value
                        if let Err(e) = self.save_setting_value() {
                            self.error = Some(format!("Failed to save setting: {}", e));
                        }
                        self.setting_input.clear();
                        self.input_mode = InputMode::Normal;
                    }
                    KeyCode::Char(c) => {
                        // For boolean and enum fields, handle cycling
                        match self.selected_setting {
                            1 => {
                                // Theme: cycle through Light/Dark/Auto
                                self.cycle_theme();
                                self.input_mode = InputMode::Normal;
                            }
                            2 => {
                                // Sort order: cycle through Recent/Oldest/Alpha/Created
                                self.cycle_sort_order();
                                self.input_mode = InputMode::Normal;
                            }
                            4 => {
                                // Sync enabled: toggle
                                self.settings.sync_enabled = !self.settings.sync_enabled;
                                if let Err(e) = self.save_settings() {
                                    self.error = Some(format!("Failed to save settings: {}", e));
                                }
                                self.input_mode = InputMode::Normal;
                            }
                            _ => {
                                // String/number fields: type normally
                                self.setting_input.push(c);
                            }
                        }
                    }
                    KeyCode::Backspace => {
                        self.setting_input.pop();
                    }
                    _ => {}
                }
            }
            _ => {}
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
        }

        self.password_input.clear();
        self.state = AppState::NoteList;

        Ok(())
    }

    /// Load notes from database
    fn load_notes(&mut self) -> Result<()> {
        if let (Some(db), Some(key)) = (&self.db, &self.key) {
            let repo = NoteRepository::new(db.connection());
            self.notes = repo.list(false, key)?;
            self.selected_note = 0;
        }
        Ok(())
    }

    /// Save current note
    fn save_note(&mut self) -> Result<()> {
        if let (Some(db), Some(key)) = (&self.db, &self.key) {
            let repo = NoteRepository::new(db.connection());

            if !self.note_input.is_empty() {
                if let Some(note_id) = &self.editing_note_id {
                    // Update existing note
                    if let Some(note) = self.notes.iter_mut().find(|n| &n.id == note_id) {
                        note.content = self.note_input.clone();
                        note.tags = self.current_tags.clone();
                        note.touch();
                        repo.update(note, key)?;
                    }
                } else {
                    // Create new note
                    let mut note = Note::new(self.note_input.clone());
                    note.tags = self.current_tags.clone();
                    repo.create(&note, key)?;
                    self.notes.insert(0, note);
                }
            }
        }
        Ok(())
    }

    /// Filter notes based on search query and sort (pinned first, then by modified date)
    fn filtered_notes(&self) -> Vec<&Note> {
        let mut notes: Vec<&Note> = if self.search_input.is_empty() {
            self.notes.iter().collect()
        } else {
            let query = self.search_input.to_lowercase();
            let query_parts: Vec<&str> = query.split_whitespace().collect();

            self.notes
                .iter()
                .filter(|note| {
                    let content_lower = note.content.to_lowercase();

                    // Check each query part
                    for part in &query_parts {
                        if part.starts_with('#') {
                            // Tag search
                            let tag = &part[1..];
                            if !note.tags.iter().any(|t| t.to_lowercase().contains(tag)) {
                                return false;
                            }
                        } else if part.starts_with('-') {
                            // Negation
                            let neg_word = &part[1..];
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

    /// Trigger manual sync
    fn trigger_sync(&mut self) {
        self.debug_log("trigger_sync - Called");
        self.debug_log(&format!("trigger_sync - sync_enabled: {}", self.settings.sync_enabled));
        self.debug_log(&format!("trigger_sync - sync_endpoint: {:?}", self.settings.sync_endpoint));

        // Check if sync is configured
        if !self.settings.sync_enabled {
            self.debug_log("trigger_sync - Sync not enabled, returning");
            self.sync_status = Some("Sync not enabled. Press 's' to configure in settings.".to_string());
            return;
        }

        if self.settings.sync_endpoint.is_none() {
            self.debug_log("trigger_sync - Sync endpoint not configured, returning");
            self.sync_status = Some("Sync endpoint not configured. Configure in database settings table.".to_string());
            return;
        }

        // Perform sync
        self.debug_log("trigger_sync - Starting sync");
        self.sync_status = Some("Syncing...".to_string());

        match self.perform_sync() {
            Ok(result) => {
                self.sync_status = Some(format!("Sync complete! {} notes synced", result));
            }
            Err(e) => {
                self.error = Some(format!("Sync failed: {}", e));
                self.sync_status = None;
            }
        }
    }

    /// Perform bidirectional sync with server
    fn perform_sync(&mut self) -> Result<usize> {
        use crate::models::sync::{SyncPushRequest, SyncPullRequest, SyncNote, SyncPushResponse, SyncPullResponse};
        use crate::repository::sync::SyncRepository;
        use chrono::Utc;

        let db = self.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not available"))?;
        let key = self.key.as_ref().ok_or_else(|| anyhow::anyhow!("Encryption key not available"))?;

        let sync_repo = SyncRepository::new(db.connection());
        let note_repo = NoteRepository::new(db.connection());

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
            // Convert notes to sync format, encrypting content and tags
            let sync_notes: Result<Vec<SyncNote>> = notes_to_push.iter().map(|note| {
                // Encrypt content and tags for transmission to server
                let encrypted_content = self.crypto.encrypt_text(&note.content, key)?;
                let content_json = serde_json::to_string(&encrypted_content)?;

                let encrypted_tags: Result<Vec<String>> = note.tags.iter()
                    .map(|tag| {
                        let encrypted_tag = self.crypto.encrypt_text(tag, key)?;
                        Ok(serde_json::to_string(&encrypted_tag)?)
                    })
                    .collect();

                Ok(SyncNote {
                    id: note.id.clone(),
                    created_at: note.created_at,
                    modified_at: note.modified_at,
                    content: content_json,
                    tags: encrypted_tags?,
                    attachments: vec![], // TODO: Handle attachments
                    pinned: note.pinned,
                    deleted: note.deleted,
                    deleted_at: note.deleted_at,
                    version: note.version,
                    word_wrap: Some(note.word_wrap),
                    syntax_language: Some(note.syntax_language.to_string()),
                })
            }).collect();

            let sync_notes = sync_notes?;

            let push_request = SyncPushRequest {
                notes: sync_notes,
                attachments: vec![],
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
                let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!("Push failed: {} - {}", status, error_text);
            }

            let push_response: SyncPushResponse = response.json()
                .context("Failed to parse push response")?;

            sync_count += push_response.accepted.len();

            // Update last push timestamp
            metadata.last_push_at = Some(Utc::now());
        }

        // PULL: Get changes from server
        let known_note_ids: Vec<String> = self.notes.iter().map(|n| n.id.clone()).collect();

        let pull_request = SyncPullRequest {
            last_sync_at: last_sync,
            known_note_ids,
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
            let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Pull failed: {} - {}", status, error_text);
        }

        let pull_response: SyncPullResponse = response.json()
            .context("Failed to parse pull response")?;

        // Apply remote changes
        self.debug_log(&format!("Pull - Received {} notes from server", pull_response.notes.len()));

        for remote_note in pull_response.notes {
            self.debug_log(&format!("Pull - Processing note: {}", remote_note.id));

            // Decrypt content and tags from server (they're stored encrypted on server)
            self.debug_log(&format!("Pull - Encrypted content JSON: {}", &remote_note.content));

            let encrypted_content: crate::crypto::EncryptedData = serde_json::from_str(&remote_note.content)?;
            self.debug_log(&format!("Pull - Encrypted data - ciphertext len: {}, nonce len: {}, tag len: {}",
                encrypted_content.ciphertext.len(),
                encrypted_content.nonce.len(),
                encrypted_content.tag.len()));

            let decrypted_content = self.crypto.decrypt_text(&encrypted_content, key)?;
            self.debug_log(&format!("Pull - Successfully decrypted content, length: {} chars", decrypted_content.len()));

            let decrypted_tags: Vec<String> = remote_note.tags.iter()
                .map(|tag_json| {
                    let encrypted_tag: crate::crypto::EncryptedData = serde_json::from_str(tag_json)?;
                    self.crypto.decrypt_text(&encrypted_tag, key)
                })
                .collect::<Result<Vec<_>>>()?;

            // Check if we have this note locally
            if let Some(local_note) = self.notes.iter_mut().find(|n| n.id == remote_note.id) {
                // Conflict resolution: Last-Write-Wins
                if remote_note.modified_at > local_note.modified_at {
                    // Remote is newer, update local with decrypted content
                    local_note.content = decrypted_content;
                    local_note.tags = decrypted_tags;
                    local_note.modified_at = remote_note.modified_at;
                    local_note.pinned = remote_note.pinned;
                    local_note.deleted = remote_note.deleted;
                    local_note.deleted_at = remote_note.deleted_at;
                    local_note.version = remote_note.version;
                    local_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
                    if let Some(lang_str) = &remote_note.syntax_language {
                        local_note.syntax_language = lang_str.parse().unwrap_or_default();
                    }

                    note_repo.update(local_note, key)?;
                    sync_count += 1;
                }
            } else {
                // New note from server, add it with decrypted content
                let mut new_note = Note::new(decrypted_content);
                new_note.id = remote_note.id;
                new_note.created_at = remote_note.created_at;
                new_note.modified_at = remote_note.modified_at;
                new_note.tags = decrypted_tags;
                new_note.pinned = remote_note.pinned;
                new_note.deleted = remote_note.deleted;
                new_note.deleted_at = remote_note.deleted_at;
                new_note.version = remote_note.version;
                new_note.word_wrap = remote_note.word_wrap.unwrap_or(true);
                if let Some(lang_str) = &remote_note.syntax_language {
                    new_note.syntax_language = lang_str.parse().unwrap_or_default();
                }

                note_repo.create(&new_note, key)?;
                self.notes.insert(0, new_note);
                sync_count += 1;
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

    /// Start editing a setting field
    fn start_editing_setting(&mut self) {
        // Populate input buffer with current value for string/number fields
        match self.selected_setting {
            0 => {
                // Language
                self.setting_input = self.settings.language.clone();
                self.input_mode = InputMode::SettingsEdit;
            }
            1 => {
                // Theme: cycle immediately, no input needed
                self.cycle_theme();
            }
            2 => {
                // Sort order: cycle immediately, no input needed
                self.cycle_sort_order();
            }
            3 => {
                // Auto-lock timeout
                self.setting_input = self.settings.auto_lock_timeout.to_string();
                self.input_mode = InputMode::SettingsEdit;
            }
            4 => {
                // Sync enabled: toggle immediately
                self.settings.sync_enabled = !self.settings.sync_enabled;
                if let Err(e) = self.save_settings() {
                    self.error = Some(format!("Failed to save settings: {}", e));
                }
            }
            5 => {
                // Sync endpoint
                self.setting_input = self.settings.sync_endpoint.clone().unwrap_or_default();
                self.input_mode = InputMode::SettingsEdit;
            }
            _ => {}
        }
    }

    /// Save edited setting value
    fn save_setting_value(&mut self) -> Result<()> {
        match self.selected_setting {
            0 => {
                // Language
                self.settings.language = self.setting_input.clone();
            }
            3 => {
                // Auto-lock timeout
                if let Ok(timeout) = self.setting_input.parse::<i32>() {
                    if timeout >= 1 && timeout <= 1440 {
                        self.settings.auto_lock_timeout = timeout;
                    } else {
                        anyhow::bail!("Auto-lock timeout must be between 1 and 1440 minutes");
                    }
                } else {
                    anyhow::bail!("Invalid number");
                }
            }
            5 => {
                // Sync endpoint
                if self.setting_input.is_empty() {
                    self.settings.sync_endpoint = None;
                } else {
                    if !self.setting_input.starts_with("http://") && !self.setting_input.starts_with("https://") {
                        anyhow::bail!("Sync endpoint must start with http:// or https://");
                    }
                    self.settings.sync_endpoint = Some(self.setting_input.clone());
                }
            }
            _ => {}
        }

        self.save_settings()
    }

    /// Cycle through theme options
    fn cycle_theme(&mut self) {
        use crate::models::Theme;
        self.settings.theme = match self.settings.theme {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Auto,
            Theme::Auto => Theme::Light,
        };
        if let Err(e) = self.save_settings() {
            self.error = Some(format!("Failed to save settings: {}", e));
        }
    }

    /// Cycle through sort order options
    fn cycle_sort_order(&mut self) {
        use crate::models::SortOrder;
        self.settings.sort_order = match self.settings.sort_order {
            SortOrder::Recent => SortOrder::Oldest,
            SortOrder::Oldest => SortOrder::Alpha,
            SortOrder::Alpha => SortOrder::Created,
            SortOrder::Created => SortOrder::Recent,
        };
        if let Err(e) = self.save_settings() {
            self.error = Some(format!("Failed to save settings: {}", e));
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
    fn paste_sync_credentials(&mut self) -> Result<()> {
        // Get clipboard content
        let mut clipboard = arboard::Clipboard::new()
            .context("Failed to access clipboard")?;
        let clipboard_text = clipboard.get_text()
            .context("Failed to read from clipboard")?;

        // Decode credentials
        let creds = SyncCredentials::from_base64(&clipboard_text.trim())
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
            self.error = Some("Salt synchronized! Please re-enter your password to unlock with the new encryption salt.".to_string());
        }

        Ok(())
    }

    /// Copy sync credentials to clipboard
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

    /// Delete selected note
    fn delete_note(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            if !self.notes.is_empty() && self.selected_note < self.notes.len() {
                let note = &self.notes[self.selected_note];
                let repo = NoteRepository::new(db.connection());
                repo.delete(&note.id)?;
                self.notes.remove(self.selected_note);
                if self.selected_note >= self.notes.len() && self.selected_note > 0 {
                    self.selected_note -= 1;
                }
            }
        }
        Ok(())
    }

    /// Edit note content with external $EDITOR
    fn edit_with_external_editor(&self) -> Result<String> {
        // Create temporary file with current note content
        let mut temp_file = NamedTempFile::new()
            .context("Failed to create temporary file")?;
        temp_file
            .write_all(self.note_input.as_bytes())
            .context("Failed to write to temporary file")?;
        temp_file.flush()?;

        let temp_path = temp_file.path();

        // Suspend TUI
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(io::stdout(), LeaveAlternateScreen)
            .context("Failed to leave alternate screen")?;

        // Get editor from environment (default to vi)
        let editor = env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());

        // Launch editor
        let status = Command::new(&editor)
            .arg(temp_path)
            .status()
            .context(format!("Failed to launch editor: {}", editor))?;

        // Resume TUI
        execute!(io::stdout(), EnterAlternateScreen)
            .context("Failed to enter alternate screen")?;
        enable_raw_mode().context("Failed to enable raw mode")?;

        if !status.success() {
            anyhow::bail!("Editor exited with non-zero status");
        }

        // Read modified content
        let content = std::fs::read_to_string(temp_path)
            .context("Failed to read modified content")?;

        Ok(content)
    }

    /// Render the UI
    pub fn render(&mut self, frame: &mut Frame) {
        match &self.state {
            AppState::Locked => self.render_locked(frame),
            AppState::NoteList => self.render_note_list(frame),
            AppState::NoteView => self.render_note_view(frame),
            AppState::Settings { .. } => self.render_settings(frame),
            AppState::Help { .. } => self.render_help(frame),
            AppState::Quit => {}
        }
    }

    /// Render locked screen
    fn render_locked(&self, frame: &mut Frame) {
        let size = frame.area();

        let title = if self.is_new_database {
            "Jottery TUI - Create Password"
        } else {
            "Jottery TUI - Unlock"
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL);

        let constraints = if self.is_new_database {
            vec![
                Constraint::Length(3),  // Password field
                Constraint::Length(3),  // Confirm field
                Constraint::Length(2),  // Help text
                Constraint::Length(3),  // Error (if any)
                Constraint::Min(0),     // Remaining space
            ]
        } else {
            vec![
                Constraint::Length(3),  // Password field
                Constraint::Length(3),  // Error (if any)
                Constraint::Min(0),     // Remaining space
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints(constraints)
            .split(size);

        frame.render_widget(block, size);

        // Password field
        let password_style = if self.is_new_database && !self.password_confirm_focused {
            Style::default().fg(Color::Yellow)
        } else if !self.is_new_database {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default()
        };

        let password_text = "*".repeat(self.password_input.len());
        let password = Paragraph::new(password_text)
            .style(password_style)
            .block(Block::default().title("Password").borders(Borders::ALL));
        frame.render_widget(password, chunks[0]);

        if self.is_new_database {
            // Confirm field
            let confirm_style = if self.password_confirm_focused {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let confirm_text = "*".repeat(self.password_confirm.len());
            let confirm = Paragraph::new(confirm_text)
                .style(confirm_style)
                .block(Block::default().title("Confirm Password").borders(Borders::ALL));
            frame.render_widget(confirm, chunks[1]);

            // Help text
            let help = Paragraph::new("Tab: switch fields | Enter: create")
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            frame.render_widget(help, chunks[2]);

            // Cursor position
            if self.password_confirm_focused {
                frame.set_cursor_position((
                    chunks[1].x + self.password_confirm.len() as u16 + 1,
                    chunks[1].y + 1,
                ));
            } else {
                frame.set_cursor_position((
                    chunks[0].x + self.password_input.len() as u16 + 1,
                    chunks[0].y + 1,
                ));
            }

            // Error (if any)
            if let Some(err) = &self.error {
                let error = Paragraph::new(err.clone())
                    .style(Style::default().fg(Color::Red))
                    .block(Block::default().title("Error").borders(Borders::ALL));
                frame.render_widget(error, chunks[3]);
            }
        } else {
            // Show cursor at end of password input
            frame.set_cursor_position((
                chunks[0].x + self.password_input.len() as u16 + 1,
                chunks[0].y + 1,
            ));

            // Error (if any)
            if let Some(err) = &self.error {
                let error = Paragraph::new(err.clone())
                    .style(Style::default().fg(Color::Red))
                    .block(Block::default().title("Error").borders(Borders::ALL));
                frame.render_widget(error, chunks[1]);
            }
        }
    }

    /// Render note list (split pane view)
    fn render_note_list(&self, frame: &mut Frame) {
        let size = frame.area();

        // Main layout: content + help at bottom
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(size);

        let content_area = main_layout[0];
        let help_area = main_layout[1];

        // Split content into left (list) and right (preview) panes
        // Notes pane is fixed width (40 chars), preview takes the rest
        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(42), Constraint::Min(0)])
            .split(content_area);

        // Left pane: note list
        let left_pane = main_chunks[0];
        let right_pane = main_chunks[1];

        // Left pane layout: search bar (optional), list
        let title = if self.search_active {
            "Notes (Search)"
        } else {
            "Notes"
        };

        let left_constraints = if self.search_active {
            vec![Constraint::Length(3), Constraint::Min(0)]
        } else {
            vec![Constraint::Min(0)]
        };

        let left_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(left_constraints)
            .split(left_pane);

        // Render search bar if active
        let list_chunk = if self.search_active {
            let search_text = format!("Search: {}", self.search_input);
            let search_bar = Paragraph::new(search_text)
                .style(Style::default().fg(Color::Yellow))
                .block(Block::default().title("Search").borders(Borders::ALL));
            frame.render_widget(search_bar, left_chunks[0]);
            left_chunks[1]
        } else {
            left_chunks[0]
        };

        // Render note list
        let list_block = Block::default()
            .title(title)
            .borders(Borders::ALL);

        let filtered = self.filtered_notes();
        let items: Vec<ListItem> = filtered
            .iter()
            .enumerate()
            .map(|(i, note)| {
                let content = note.content.lines().next().unwrap_or("");
                let mut preview = if content.len() > 30 {
                    format!("{}...", &content[..30])
                } else {
                    content.to_string()
                };

                // Add indicators for pinned and attachments
                let mut indicators = String::new();
                if note.pinned {
                    indicators.push_str("📌 ");
                }
                if !note.attachments.is_empty() {
                    indicators.push_str(&format!("📎{} ", note.attachments.len()));
                }

                if !indicators.is_empty() {
                    preview = format!("{}{}", indicators, preview);
                }

                let style = if i == self.selected_note {
                    Style::default()
                        .fg(Color::Yellow)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                ListItem::new(preview).style(style)
            })
            .collect();

        let list = List::new(items).block(list_block);
        frame.render_widget(list, list_chunk);

        // Help text (full width at bottom)
        let status_text = if let Some(ref status) = self.sync_status {
            status.clone()
        } else if self.search_active {
            "Type: search | Esc: exit | ↑/↓: navigate".to_string()
        } else {
            "/: search | p: pin | y: sync | s: settings | n: new | i: edit".to_string()
        };
        let help = Paragraph::new(status_text)
            .style(if self.sync_status.is_some() {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default().fg(Color::DarkGray)
            })
            .alignment(Alignment::Center);
        frame.render_widget(help, help_area);

        // Right pane: note preview
        let preview_block = Block::default()
            .title("Preview")
            .borders(Borders::ALL);

        if !filtered.is_empty() && self.selected_note < filtered.len() {
            let note = filtered[self.selected_note];

            // Build metadata line (tags and syntax language)
            let mut metadata_parts = Vec::new();

            // Show tags (or n/a if none)
            let tags_str = if !note.tags.is_empty() {
                note.tags.iter()
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ")
            } else {
                "n/a".to_string()
            };
            metadata_parts.push(format!("Tags: {}", tags_str));

            // Show syntax language
            metadata_parts.push(format!("Type: {}", note.syntax_language));

            let metadata_line = if !metadata_parts.is_empty() {
                format!("{}\n\n", metadata_parts.join(" | "))
            } else {
                String::new()
            };

            let preview_text = format!("{}{}", metadata_line, note.content);
            let preview = Paragraph::new(preview_text)
                .block(preview_block)
                .wrap(Wrap { trim: false });
            frame.render_widget(preview, right_pane);
        } else {
            let preview = Paragraph::new("No notes")
                .block(preview_block)
                .alignment(Alignment::Center);
            frame.render_widget(preview, right_pane);
        }
    }

    /// Render note view
    fn render_note_view(&self, frame: &mut Frame) {
        let size = frame.area();

        let mode_text = match self.input_mode {
            InputMode::Normal => "NORMAL",
            InputMode::Insert => "INSERT",
            InputMode::Tag => "TAG",
            InputMode::SettingsEdit => "NORMAL", // Should not happen in note view
        };

        let block = Block::default()
            .title(format!("Note - {}", mode_text))
            .borders(Borders::ALL);

        let constraints = vec![
            Constraint::Length(2),  // Tags display
            Constraint::Min(0),     // Note content
            Constraint::Length(3),  // Help text
        ];

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints(constraints)
            .split(size);

        // Render tags
        let tags_text = if self.current_tags.is_empty() {
            if matches!(self.input_mode, InputMode::Tag) {
                format!("Tags: {}_", self.tag_input)
            } else {
                "Tags: (none - press 't' to add)".to_string()
            }
        } else {
            if matches!(self.input_mode, InputMode::Tag) {
                format!("Tags: {} {}_",
                    self.current_tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "),
                    self.tag_input
                )
            } else {
                format!("Tags: {}",
                    self.current_tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")
                )
            }
        };

        let tags_style = if matches!(self.input_mode, InputMode::Tag) {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::Blue)
        };

        let tags = Paragraph::new(tags_text)
            .style(tags_style);
        frame.render_widget(tags, chunks[0]);

        // Render note content
        let text = Paragraph::new(self.note_input.clone())
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(text, chunks[1]);

        // Help text
        let help = match self.input_mode {
            InputMode::Normal | InputMode::SettingsEdit => {
                Paragraph::new("i: insert | t: tags | q/Esc: save & quit")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center)
            }
            InputMode::Insert => {
                Paragraph::new("Esc: normal mode | Type to edit")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center)
            }
            InputMode::Tag => {
                Paragraph::new("Type tag name | Enter: add | Backspace: remove last | Esc: exit")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center)
            }
        };
        frame.render_widget(help, chunks[2]);

        // Show cursor
        match self.input_mode {
            InputMode::Insert => {
                // Calculate cursor position at end of text
                let lines: Vec<&str> = self.note_input.lines().collect();
                let line_count = if self.note_input.is_empty() {
                    0
                } else {
                    lines.len().saturating_sub(1)
                };
                let last_line_len = lines.last().map(|l| l.len()).unwrap_or(0);

                frame.set_cursor_position((
                    chunks[1].x + 1 + last_line_len as u16,
                    chunks[1].y + 1 + line_count as u16,
                ));
            }
            InputMode::Tag => {
                // Cursor after tag input
                let tag_prefix_len = if self.current_tags.is_empty() {
                    "Tags: ".len()
                } else {
                    format!("Tags: {} ",
                        self.current_tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")
                    ).len()
                };

                frame.set_cursor_position((
                    chunks[0].x + tag_prefix_len as u16 + self.tag_input.len() as u16,
                    chunks[0].y,
                ));
            }
            _ => {}
        }
    }

    /// Render settings screen
    fn render_settings(&self, frame: &mut Frame) {
        let size = frame.area();

        let mode_text = match self.input_mode {
            InputMode::SettingsEdit => " [EDIT]",
            _ => "",
        };

        let block = Block::default()
            .title(format!("Settings{} - ↑/↓: navigate | Enter/i: edit | s/q: close", mode_text))
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Green));

        // Helper to create field line with selection indicator
        let field_line = |index: usize, label: String, value: String| -> Line {
            let selected = index == self.selected_setting;
            let editing = selected && matches!(self.input_mode, InputMode::SettingsEdit);

            let display_value = if editing && (index == 0 || index == 3 || index == 5) {
                // Show input buffer for editable fields
                format!("{}_", self.setting_input)
            } else {
                value
            };

            let prefix = if selected { "→ " } else { "  " };
            let label_style = if selected {
                Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let value_style = if editing {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Yellow)
            };

            Line::from(vec![
                Span::styled(prefix, label_style.clone()),
                Span::styled(label, label_style),
                Span::styled(display_value, value_style),
            ])
        };

        let settings_text = vec![
            Line::from(vec![
                Span::styled("Application Settings", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            field_line(0, "Language:              ".to_string(), self.settings.language.clone()),
            field_line(1, "Theme:                 ".to_string(), format!("{} (press Enter to cycle)", self.settings.theme)),
            field_line(2, "Sort Order:            ".to_string(), format!("{} (press Enter to cycle)", self.settings.sort_order)),
            field_line(3, "Auto-lock Timeout:     ".to_string(), format!("{} minutes", self.settings.auto_lock_timeout)),
            Line::from(""),
            Line::from(vec![
                Span::styled("Sync Settings", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            field_line(4, "Sync Enabled:          ".to_string(), format!("{} (press Enter to toggle)", if self.settings.sync_enabled { "Yes" } else { "No" })),
            field_line(5, "Sync Endpoint:         ".to_string(), self.settings.sync_endpoint.clone().unwrap_or_else(|| "Not configured".to_string())),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("Instructions: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  • Use ↑/↓ or j/k to navigate between fields"),
            Line::from("  • Press Enter, i, or Space to edit a field"),
            Line::from("  • For text fields: type and press Enter to save, Esc to cancel"),
            Line::from("  • For toggles and cycles: press Enter to change value immediately"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Sync Credentials: ", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  • Press 'p' to paste sync credentials from clipboard"),
            Line::from("  • Press 'c' to copy sync credentials to clipboard"),
        ];

        // Add status and error messages if present
        let mut all_lines = settings_text;
        if let Some(status) = &self.sync_status {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
                Span::styled(status.clone(), Style::default().fg(Color::Green)),
            ]));
        }
        if let Some(err) = &self.error {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
                Span::styled(err.clone(), Style::default().fg(Color::Red)),
            ]));
        }

        let paragraph = Paragraph::new(all_lines)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, size);

        // Show cursor when editing text fields
        if matches!(self.input_mode, InputMode::SettingsEdit) && (self.selected_setting == 0 || self.selected_setting == 3 || self.selected_setting == 5) {
            // Calculate cursor position based on selected field
            let line_offset = match self.selected_setting {
                0 => 2,  // Language is on line 2
                3 => 5,  // Auto-lock timeout is on line 5
                5 => 10, // Sync endpoint is on line 10
                _ => 0,
            };

            let cursor_x = 26 + self.setting_input.len() as u16; // After label
            let cursor_y = line_offset + 1; // +1 for border

            frame.set_cursor_position((cursor_x, cursor_y));
        }
    }

    /// Render help screen
    fn render_help(&self, frame: &mut Frame) {
        let size = frame.area();

        let block = Block::default()
            .title("Keyboard Shortcuts - Press ? or q to close")
            .borders(Borders::ALL)
            .style(Style::default().fg(Color::Yellow));

        let help_text = vec![
            Line::from(vec![
                Span::styled("UNLOCK SCREEN", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Enter password"),
            Line::from("  Enter                 Unlock database"),
            Line::from("  Tab                   Switch password/confirm (new DB)"),
            Line::from("  Backspace             Delete character"),
            Line::from("  Ctrl+q / Esc          Quit application"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE LIST", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  /                     Enter search mode"),
            Line::from("  y                     Sync notes (if configured)"),
            Line::from("  s                     Show settings"),
            Line::from("  n                     Create new note"),
            Line::from("  i / Enter             Edit selected note"),
            Line::from("  d                     Delete selected note"),
            Line::from("  j / ↓                 Move down"),
            Line::from("  k / ↑                 Move up"),
            Line::from("  ?                     Show this help"),
            Line::from("  Ctrl+q                Quit application"),
            Line::from(""),
            Line::from(vec![
                Span::styled("SEARCH MODE", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Enter search query"),
            Line::from("  #tag                  Search by tag"),
            Line::from("  -word                 Exclude word (negation)"),
            Line::from("  word1 word2           Match all words (AND)"),
            Line::from("  Enter                 Open selected note"),
            Line::from("  Esc                   Exit search mode"),
            Line::from("  ↑ / ↓                 Navigate results"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE EDITOR - NORMAL MODE", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  i                     Enter insert mode"),
            Line::from("  e                     Edit with external $EDITOR"),
            Line::from("  t                     Enter tag mode"),
            Line::from("  ?                     Show this help"),
            Line::from("  q / Esc               Save and return to list"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE EDITOR - INSERT MODE", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Edit note content"),
            Line::from("  Enter                 New line"),
            Line::from("  Backspace / Delete    Delete character"),
            Line::from("  Esc                   Exit to normal mode"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE EDITOR - TAG MODE", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Enter tag name"),
            Line::from("  Enter                 Add tag"),
            Line::from("  Backspace (empty)     Remove last tag"),
            Line::from("  Backspace             Delete character from input"),
            Line::from("  Esc                   Exit to normal mode"),
            Line::from(""),
            Line::from(vec![
                Span::styled("SETTINGS", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  j/k or ↑/↓            Navigate between fields"),
            Line::from("  Enter / i / Space     Edit selected field"),
            Line::from("  Enter                 Save text/number fields, cycle/toggle other fields"),
            Line::from("  Esc                   Cancel editing (text/number fields)"),
            Line::from("  p                     Paste sync credentials from clipboard"),
            Line::from("  c                     Copy sync credentials to clipboard"),
            Line::from("  s / q                 Close settings panel"),
            Line::from(""),
            Line::from(vec![
                Span::styled("GLOBAL", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  ?                     Show this help screen"),
        ];

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, size);
    }

    /// Check if app should quit
    pub fn should_quit(&self) -> bool {
        matches!(self.state, AppState::Quit)
    }
}
