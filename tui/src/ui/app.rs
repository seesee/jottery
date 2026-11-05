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
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use rust_i18n::t;
use ratatui::Frame;
use zeroize::{Zeroize, Zeroizing};
use std::{
    cell::{Cell, RefCell},
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
    models::{get_note_title, Note, SortOrder, UserSettings, sync::ConflictData},
    password_storage::{self, PasswordStorage, RetrieveResult, StorageBackendType},
    repository::{SettingsRepository, sync::SyncRepository},
    search::SearchQuery,
};


use super::state::{AppState, InputMode, ViewMode};
use super::rendering;
use super::input;

/// Write a line to the debug log, if there is one.
pub(crate) fn write_debug_log(debug_log: &Option<Arc<Mutex<File>>>, message: &str) {
    if let Some(log) = debug_log {
        if let Ok(mut file) = log.lock() {
            let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
            let _ = writeln!(file, "[{}] {}", timestamp, message);
            let _ = file.flush();
        }
    }
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
    /// Saved unlock password (for envelope encryption when server password differs from notes password)
    pub unlock_password: Option<Zeroizing<String>>,
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
    /// How many notes in the vault couldn't be read at the last load
    pub(crate) unreadable_notes: usize,
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
    /// Automatic syncs wait until then, because the server asked or refused
    pub(crate) sync_paused_until: Option<Instant>,
    /// Sync on a thread of its own, so a slow server can't freeze the screen. Only the
    /// terminal app does: a CLI command waits for its result anyway.
    pub(crate) sync_in_background: bool,
    /// The sync running in the background
    pub(crate) sync_job: Option<crate::ui::operations::sync::BackgroundSync>,
    /// A sync was asked for that has to wait, and whether it's a forced one
    pub(crate) sync_requested: Option<bool>,
    /// When the list last caught up with a running sync
    pub(crate) last_sync_absorb: Option<Instant>,
    /// Keeps a background sync and this thread from changing the vault at the same time
    pub(crate) sync_gate: crate::ui::operations::sync::VaultGate,
    /// The next successful sync should look at the server's copy of the key
    pub(crate) server_key_check_owed: bool,
    /// Current color scheme (cached from settings)
    pub color_scheme: crate::ui::ColorScheme,
    /// Cached color palette names (for color cycling without allocation)
    pub(crate) color_palette_cache: Vec<String>,
    /// Cached unique tags from all notes (for fast tag completion)
    pub(crate) all_tags_cache: Vec<String>,
    /// Indices into `notes` of the visible notes, in display order, with the length
    /// of `notes` when they were worked out. A length that no longer matches means
    /// they're stale. RefCell so that filtered_notes(&self) can update it.
    filtered_notes_cache: RefCell<Option<(usize, Vec<usize>)>>,
    /// How many notes the current search matches in the other mode (archive/active)
    opposite_count_cache: Cell<Option<usize>>,
    /// Counts loads of `notes`, so a tick can tell whether anything arrived
    notes_generation: u64,
    /// Selected attachment index in preview pane
    pub selected_attachment: usize,
    /// Which panel is focused in preview pane (content or attachments)
    pub focused_panel: super::state::FocusedPanel,
    /// File path input buffer (when adding or saving attachments)
    pub attachment_path_input: String,
    /// The attachment the path prompt is saving. None when it's adding a file.
    pub attachment_to_save: Option<crate::models::Attachment>,
    /// An attachment of a type that won't be opened, waiting for an answer about saving it
    pub attachment_to_confirm: Option<crate::models::Attachment>,
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
    /// Show the confirmation for permanently emptying the recycle bin
    pub show_empty_bin_confirm: bool,
    /// Notes the server says to delete, waiting for an answer. Above zero, it asks.
    pub held_deletions: usize,
    /// Show the confirmation for disconnecting from the sync server
    pub show_disconnect_confirm: bool,
    /// The plain HTTP address the user has been warned about. Entering it again accepts that.
    pub insecure_endpoint_warned: Option<String>,
    /// Show note info modal
    pub show_note_info: bool,
    /// Rendered area for note list (for mouse click detection)
    pub note_list_area: Option<ratatui::layout::Rect>,
    /// Index of the first note drawn in the list, kept between frames so that
    /// moving the selection scrolls only as far as it has to
    pub note_list_offset: usize,
    /// Where each drawn note landed in the list area
    pub note_list_rows: Vec<rendering::note_list::NoteRow>,
    /// Rendered area for tags line in preview (for mouse click detection)
    pub tags_line_area: Option<ratatui::layout::Rect>,
    /// Individual tag positions for click detection: (tag_name, x_start, x_end)
    pub tag_positions: Vec<(String, u16, u16)>,
    /// Note link positions for click detection: (note_id, y_line, x_start, x_end)
    pub note_link_positions: Vec<(String, u16, u16, u16)>,
    /// Which of the links on screen the keyboard is on
    pub selected_link: Option<usize>,
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
    /// Whether this build encrypts the database file
    pub(crate) database_encrypted: bool,
    /// Password storage backend (keychain or file-based fallback)
    pub(crate) password_storage: Box<dyn PasswordStorage>,
    /// SSE handle for real-time sync notifications (when connected)
    pub(crate) sse_handle: Option<crate::sse::SseHandle>,
    /// The stream stopped trying. A sync that works shows the device is welcome again.
    pub(crate) sse_gave_up: bool,
    /// Inbox items from server (unencrypted)
    pub inbox_items: Vec<crate::models::sync::InboxItem>,
    /// Selected inbox item index
    pub selected_inbox_item: usize,
    /// Show inbox delete confirmation
    pub show_inbox_delete_confirm: bool,
    /// Show inbox delete-all confirmation
    pub show_inbox_delete_all_confirm: bool,
    /// A file an export would replace, waiting for an answer
    pub export_overwrite_path: Option<PathBuf>,
    /// Earlier searches this session, the latest last
    pub search_history: Vec<String>,
    /// How far back Ctrl+P has gone, and what was typed before it started
    pub search_history_position: Option<(usize, String)>,
    /// The list of saved searches, when it's showing
    pub saved_searches: Vec<crate::repository::SavedSearch>,
    /// Selected index in the list of saved searches
    pub selected_saved_search: usize,
    /// Show the confirmation for deleting a saved search
    pub show_saved_search_delete_confirm: bool,
    /// What the query in the search bar is to be saved as
    pub saved_search_name_input: String,
}


/// The key is wiped on the way out as well as on lock. `key_manager` looks after its own
/// copy, but this one is a plain array.
impl Drop for App {
    fn drop(&mut self) {
        if let Some(key) = self.key.as_mut() {
            key.zeroize();
        }
    }
}

impl App {
    /// Create a new app
    pub fn new(db_path: PathBuf, debug_log: Option<Arc<Mutex<File>>>) -> Result<Self> {
        // Initialise password storage with migration from file to keychain if available
        let config_dir = db_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let password_storage = password_storage::create_storage(config_dir, true);

        Ok(Self::with_storage(db_path, debug_log, password_storage))
    }

    /// An unlocked app with no terminal, for CLI commands that share the TUI's
    /// logic. The keychain stays out of it: it may not be reachable from a script.
    pub fn headless(db_path: PathBuf, db: Database, key: [u8; 32]) -> Result<Self> {
        let config_dir = db_path.parent()
            .ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let password_storage = password_storage::create_file_storage(config_dir);

        let mut app = Self::with_storage(db_path, None, password_storage);
        app.db = Some(db);
        app.key = Some(key);
        app.key_manager.set_master_key(key);
        Ok(app)
    }

    fn with_storage(
        db_path: PathBuf,
        debug_log: Option<Arc<Mutex<File>>>,
        password_storage: Box<dyn PasswordStorage>,
    ) -> Self {
        let is_new_database = !db_path.exists();

        Self {
            view_mode: ViewMode::NoteList,
            state: AppState::Locked,
            input_mode: InputMode::Normal,
            password_input: String::new(),
            password_confirm: String::new(),
            unlock_password: None,
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
            unreadable_notes: 0,
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
            sync_paused_until: None,
            sync_in_background: false,
            sync_job: None,
            sync_requested: None,
            last_sync_absorb: None,
            sync_gate: Default::default(),
            server_key_check_owed: false,
            color_scheme: crate::ui::ColorScheme::default(),
            color_palette_cache: Vec::new(), // Populated when settings are loaded
            all_tags_cache: Vec::new(), // Populated when notes are loaded
            filtered_notes_cache: RefCell::new(None), // Populated on first access, invalidated on changes
            opposite_count_cache: Cell::new(None),
            notes_generation: 0,
            selected_attachment: 0,
            focused_panel: super::state::FocusedPanel::default(),
            attachment_path_input: String::new(),
            attachment_to_save: None,
            attachment_to_confirm: None,
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
            show_empty_bin_confirm: false,
            held_deletions: 0,
            show_disconnect_confirm: false,
            insecure_endpoint_warned: None,
            show_note_info: false,
            note_list_area: None,
            note_list_offset: 0,
            note_list_rows: Vec::new(),
            tags_line_area: None,
            tag_positions: Vec::new(),
            note_link_positions: Vec::new(),
            selected_link: None,
            preview_area: None,
            tag_completions: Vec::new(),
            tag_completion_index: 0,
            search_tag_completions: Vec::new(),
            search_tag_completion_index: 0,
            database_encrypted: Database::encrypts_files(),
            password_storage,
            sse_handle: None,
            sse_gave_up: false,
            inbox_items: Vec::new(),
            selected_inbox_item: 0,
            show_inbox_delete_confirm: false,
            show_inbox_delete_all_confirm: false,
            export_overwrite_path: None,
            search_history: Vec::new(),
            search_history_position: None,
            saved_searches: Vec::new(),
            selected_saved_search: 0,
            show_saved_search_delete_confirm: false,
            saved_search_name_input: String::new(),
        }
    }

    /// Write to debug log if enabled
    pub(crate) fn debug_log(&self, message: &str) {
        write_debug_log(&self.debug_log, message);
    }

    /// Tell the user something went well, in the status bar. It clears itself.
    pub(crate) fn notify(&mut self, message: String) {
        self.sync_status = Some(message);
        self.sync_status_set_at = Some(Instant::now());
    }

    /// Every loaded note, archived or not, in no particular order
    pub fn all_notes(&self) -> &[Note] {
        &self.notes
    }

    /// Give the database back to a CLI command that's finished with the app.
    pub fn into_database(mut self) -> Option<Database> {
        self.db.take()
    }

    /// The stored sync settings, if the vault is open and has any
    pub fn sync_metadata(&self) -> Option<crate::models::sync::SyncMetadata> {
        let db = self.db.as_ref()?;
        SyncRepository::new(db.connection()).get_metadata().ok().flatten()
    }

    /// Whether the vault is open
    pub fn is_unlocked(&self) -> bool {
        self.key.is_some()
    }

    /// Note that the user did something, which holds off the auto-lock
    pub fn register_activity(&self) {
        self.key_manager.register_activity();
    }

    /// Take the auto-lock timeout from the settings
    pub(crate) fn apply_auto_lock_setting(&self) {
        self.key_manager.set_auto_lock(self.settings.auto_lock_timeout.max(0) as u64);
    }

    /// Lock the vault if the user has been away for the auto-lock timeout
    pub fn check_auto_lock(&mut self) {
        if self.is_unlocked() && self.key_manager.should_lock() {
            self.debug_log("Auto-lock: timed out");
            self.lock();
        }
    }

    /// Lock the vault, keeping nothing of it in memory.
    ///
    /// Text that hasn't been saved yet is saved first, or the lock would lose it.
    /// Anything held as a plain String is wiped, since dropping it would leave the
    /// bytes behind. Unlocking starts over from the database.
    pub fn lock(&mut self) {
        if matches!(self.state, AppState::NoteView) {
            if let Err(e) = crate::ui::operations::notes::save_note(self) {
                self.debug_log(&format!("Lock - Could not save the open note: {}", e));
            }
        }

        crate::ui::operations::sync::cancel_background_sync(self);
        self.stop_sse();
        self.key_manager.clear_master_key();
        if let Some(key) = self.key.as_mut() {
            key.zeroize();
        }
        self.key = None;
        self.unlock_password = None;
        self.db = None;

        for buffer in [
            &mut self.password_input,
            &mut self.password_confirm,
            &mut self.credential_input,
            &mut self.setting_input,
            &mut self.note_input,
            &mut self.tag_input,
            &mut self.search_input,
            &mut self.saved_search_name_input,
            &mut self.bulk_tags_input,
            &mut self.bulk_export_path_input,
            &mut self.attachment_path_input,
            &mut self.conflict_local_content,
            &mut self.conflict_server_content,
        ] {
            buffer.zeroize();
        }
        for note in &mut self.notes {
            note.content.zeroize();
            note.tags.zeroize();
        }
        for version in &mut self.loaded_versions {
            version.content.zeroize();
            version.tags.zeroize();
        }
        if let Some(content) = self.conflict_ancestor_content.as_mut() {
            content.zeroize();
        }
        self.current_tags.zeroize();
        self.conflict_local_tags.zeroize();
        self.conflict_server_tags.zeroize();
        self.conflict_ancestor_tags = None;
        self.conflict_ancestor_content = None;
        self.conflict_data = None;
        self.conflict_note_id = None;

        self.notes.clear();
        self.unreadable_notes = 0;
        self.loaded_versions.clear();
        self.versions_note_id = None;
        self.inbox_items.clear();
        self.search_history.zeroize();
        self.search_history_position = None;
        for search in &mut self.saved_searches {
            search.name.zeroize();
            search.query.zeroize();
        }
        self.saved_searches.clear();
        self.selected_saved_search = 0;
        self.show_saved_search_delete_confirm = false;
        self.selected_note_ids.clear();
        self.conflict_note_ids.clear();
        self.all_tags_cache.clear();
        self.tag_completions.clear();
        self.search_tag_completions.clear();
        self.path_completions.clear();
        self.tag_positions.clear();
        self.note_link_positions.clear();
        self.selected_link = None;
        self.note_list_rows.clear();
        self.note_list_offset = 0;
        self.syntax_highlighter.clear_cache();
        self.invalidate_filter_cache();

        // What's on screen should go too, except the theme the lock screen is drawn in
        let theme = self.settings.theme.clone();
        self.settings = UserSettings { theme, ..UserSettings::default() };
        self.sync_status = None;
        self.sync_status_set_at = None;
        self.error = None;
        self.editing_note_id = None;
        self.selected_note = 0;
        self.selected_note_id = None;
        self.selected_attachment = 0;
        self.preview_scroll_offset = 0;
        self.search_active = false;
        self.archive_mode = false;
        self.is_multi_select_mode = false;
        self.last_selected_index = None;
        self.focused_panel = super::state::FocusedPanel::default();
        self.view_mode = ViewMode::NoteList;
        self.input_mode = InputMode::Normal;
        self.attachment_to_save = None;
        self.attachment_to_confirm = None;
        self.show_force_sync_confirm = false;
        self.show_empty_bin_confirm = false;
        self.held_deletions = 0;
        self.show_disconnect_confirm = false;
        self.export_overwrite_path = None;
        self.show_bulk_delete_confirm = false;
        self.show_bulk_combine_confirm = false;
        self.show_note_info = false;
        self.show_inbox_delete_confirm = false;
        self.show_inbox_delete_all_confirm = false;
        self.last_auto_sync = None;
        self.server_key_check_owed = false;

        // Dropping the old state wipes any password held in it
        self.state = AppState::Locked;
        self.need_redraw = true;
    }

    /// Handle a key press, holding off a background sync until it's finished with.
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        let gate = self.sync_gate.clone();
        let _held = gate.hold();
        crate::ui::operations::sync::absorb_sync_changes(self);

        let result = self.dispatch_key(key);

        self.sync_gate.set_open_note(self.editing_note_id.as_deref());
        result
    }

    /// Handle a mouse event, on the same terms as a key press.
    pub fn handle_mouse(&mut self, mouse: crossterm::event::MouseEvent) {
        let gate = self.sync_gate.clone();
        let _held = gate.hold();
        crate::ui::operations::sync::absorb_sync_changes(self);

        input::mouse::handle_mouse(self, mouse);

        self.sync_gate.set_open_note(self.editing_note_id.as_deref());
    }

    /// Handle key events
    fn dispatch_key(&mut self, key: KeyEvent) -> Result<()> {
        if key.code == KeyCode::Char('l')
            && key.modifiers.contains(KeyModifiers::CONTROL)
            && self.is_unlocked()
        {
            self.lock();
            return Ok(());
        }

        // The terminal is in raw mode, so this arrives as a key rather than a signal.
        // It means the same in every screen, and it isn't text for a prompt.
        if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
            if matches!(self.state, AppState::NoteView) {
                if let Err(e) = crate::ui::operations::notes::save_note(self) {
                    self.error = Some(t!("note.save_failed", error = e.to_string()).to_string());
                    return Ok(());
                }
            }
            self.state = AppState::Quit;
            return Ok(());
        }

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
            AppState::InputEmailForStatus { .. } => input::credentials::handle_input_email_for_status_key(self, key)?,
            AppState::ShowRegistrationStatus { .. } => input::credentials::handle_show_registration_status_key(self, key)?,
            AppState::RegisterInputEndpoint { .. } => input::credentials::handle_register_input_endpoint_key(self, key)?,
            AppState::RegisterInputEmail { .. } => input::credentials::handle_register_input_email_key(self, key)?,
            AppState::RegisterInputPassword { .. } => input::credentials::handle_register_input_password_key(self, key)?,
            AppState::RegisterPendingApproval { .. } => input::credentials::handle_register_pending_approval_key(self, key)?,
            AppState::RegisterInputDeviceName { .. } => input::credentials::handle_register_input_device_name_key(self, key)?,
            AppState::ChangePasswordCurrent { .. } => input::credentials::handle_change_password_current_key(self, key)?,
            AppState::ChangePasswordNew { .. } => input::credentials::handle_change_password_new_key(self, key)?,
            AppState::ChangePasswordConfirm { .. } => input::credentials::handle_change_password_confirm_key(self, key)?,
            AppState::Quit => {}
            AppState::Settings { .. } => unreachable!(), // Handled above
            AppState::Help { .. } => unreachable!(), // Handled above
        }
        Ok(())
    }




    /// Unlock the database
    ///
    /// Delegates to operations::auth::unlock() for the core implementation,
    /// then refreshes color palette cache which auth.rs doesn't handle.
    fn unlock(&mut self) -> Result<()> {
        // Use the comprehensive unlock implementation from auth module
        // This handles all credential types (PLAINTEXT:, ENCRYPTED:, clone-device)
        crate::ui::operations::auth::unlock(self)?;

        // Refresh color palette cache (not done in auth::unlock)
        self.refresh_color_palette_cache();
        self.debug_log("Unlock - Color palette cache refreshed");

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
                self.debug_log("Auto-unlock: found a stored password");
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
                // Only a wrong password makes the stored one useless. A locked file or a
                // server that's down will pass, and the user would have to set it again.
                if crate::vault::is_wrong_password(&e) {
                    let _ = self.password_storage.delete();
                }
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
    #[allow(dead_code)]
    pub fn store_password_for_autounlock(&mut self, password: &str) -> Result<()> {
        // Store password using the storage backend (keychain or file)
        self.password_storage.store(password)?;

        self.debug_log(&format!(
            "Password stored using {} backend",
            if self.password_storage.backend_type().is_secure() { "keychain" } else { "file" }
        ));

        // Update settings (note: we no longer store encrypted password in settings)
        self.settings.remember_password = true;

        if let Some(db) = &self.db {
            let settings_repo = SettingsRepository::new(db.connection());
            settings_repo.update(&self.settings)?;
        }

        Ok(())
    }

    /// Move the highlight to a position in the visible list.
    pub(crate) fn select_note_at(&mut self, index: usize) {
        self.selected_note = index;
        self.selected_note_id = self.filtered_notes().get(index).map(|note| note.id.clone());
    }

    /// The selected notes that are still in the list. The selection outlives a change of
    /// search or mode, and nothing should act on notes that can't be seen.
    pub(crate) fn visible_selection(&self) -> Vec<&Note> {
        self.filtered_notes().into_iter()
            .filter(|note| self.selected_note_ids.contains(&note.id))
            .collect()
    }

    /// The ids of the selected notes a bulk action may change, which leaves out what a
    /// single-note action would refuse.
    pub(crate) fn editable_selection(&self) -> Vec<String> {
        self.visible_selection().into_iter()
            .filter(|note| !note.locked)
            .map(|note| note.id.clone())
            .collect()
    }

    /// Point `selected_note`, an index into the visible list, at the note that was
    /// selected, or the nearest one if that note has left the list.
    pub(crate) fn keep_selection_on_note(&mut self) {
        let position = {
            let filtered = self.filtered_notes();
            self.selected_note_id.as_ref()
                .and_then(|id| filtered.iter().position(|note| &note.id == id))
                .unwrap_or_else(|| self.selected_note.min(filtered.len().saturating_sub(1)))
        };

        self.selected_note = position;
        self.selected_note_id = self.filtered_notes().get(position).map(|note| note.id.clone());
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

    /// The note drawn on a row of the list area, as an index into the visible notes
    pub fn note_at_row(&self, row: u16) -> Option<usize> {
        self.note_list_rows.iter()
            .find(|drawn| (drawn.top..drawn.bottom).contains(&row))
            .map(|drawn| drawn.index)
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
        self.opposite_count_cache.set(None);
    }

    /// Note that `notes` was reloaded
    pub(crate) fn notes_loaded(&mut self) {
        self.notes_generation = self.notes_generation.wrapping_add(1);
    }

    /// Run the timed checks, and say whether they changed anything on screen.
    ///
    /// Drawing a vault of notes isn't free, and this runs four times a second, so an
    /// idle app shouldn't redraw.
    pub fn on_tick(&mut self) -> bool {
        // A sync is applying a page. The next tick is a moment away.
        let gate = self.sync_gate.clone();
        let Some(_held) = gate.try_hold() else {
            return false;
        };

        let status = self.sync_status.clone();
        let error = self.error.clone();
        let generation = self.notes_generation;
        let inbox_items = self.inbox_items.len();

        self.check_auto_lock();
        self.check_auto_sync();

        status != self.sync_status
            || error != self.error
            || generation != self.notes_generation
            || inbox_items != self.inbox_items.len()
    }

    /// Display order for two notes: pinned notes first, then as the settings say. Alpha
    /// sorts by title, and is handled by `sort_for_display`, which works each out once.
    fn display_order(order: SortOrder, a: &Note, b: &Note) -> std::cmp::Ordering {
        b.pinned.cmp(&a.pinned).then_with(|| match order {
            SortOrder::Recent | SortOrder::Alpha => b.modified_at.cmp(&a.modified_at),
            SortOrder::Oldest => a.modified_at.cmp(&b.modified_at),
            SortOrder::Created => b.created_at.cmp(&a.created_at),
        })
    }

    /// Put indices into `notes` in display order.
    fn sort_for_display(&self, indices: &mut [usize]) {
        let order = self.settings.sort_order;

        if order == SortOrder::Alpha {
            indices.sort_by_cached_key(|&index| {
                let note = &self.notes[index];
                (!note.pinned, get_note_title(&note.content, &note.tags).to_lowercase())
            });
        } else {
            indices.sort_by(|&a, &b| Self::display_order(order, &self.notes[a], &self.notes[b]));
        }
    }

    /// Sort notes by pinned status first (pinned at top), then by modified_at descending
    #[cfg(test)]
    pub fn sort_notes_by_pinned_and_date(notes: &mut [&Note]) {
        notes.sort_by(|a, b| Self::display_order(SortOrder::Recent, a, b));
    }

    /// Filter notes based on search query and sort (pinned first, then by modified date)
    ///
    /// Uses an internal cache to avoid repeated filtering/sorting. The cache is
    /// automatically invalidated when search_input, archive_mode, or notes change.
    pub fn filtered_notes(&self) -> Vec<&Note> {
        // Check if we have a valid cache
        if let Some((length, indices)) = self.filtered_notes_cache.borrow().as_ref() {
            if *length == self.notes.len() {
                return indices.iter().map(|&index| &self.notes[index]).collect();
            }
        }

        // Cache miss - compute filtered notes
        let mut indices: Vec<usize> = if self.search_input.is_empty() {
            (0..self.notes.len())
                .filter(|&index| self.notes[index].archived == self.archive_mode)
                .collect()
        } else {
            let query = SearchQuery::parse(&self.search_input, &self.settings);
            let archived = query.archived().unwrap_or(self.archive_mode);

            (0..self.notes.len())
                .filter(|&index| {
                    let note = &self.notes[index];
                    note.archived == archived && query.matches(note, &self.settings)
                })
                .collect()
        };

        self.sort_for_display(&mut indices);

        let notes = indices.iter().map(|&index| &self.notes[index]).collect();
        *self.filtered_notes_cache.borrow_mut() = Some((self.notes.len(), indices));

        notes
    }

    /// Count how many notes match the current search in the opposite mode (archive/active)
    /// Returns count of matching notes in the opposite mode
    pub fn count_opposite_mode_matches(&self) -> usize {
        if self.search_input.is_empty() {
            return 0;
        }

        if let Some(count) = self.opposite_count_cache.get() {
            return count;
        }

        // A query that names the archive isn't asking about the other mode
        let query = SearchQuery::parse(&self.search_input, &self.settings);
        let count = if query.archived().is_some() {
            0
        } else {
            self.notes
                .iter()
                .filter(|note| note.archived != self.archive_mode && query.matches(note, &self.settings))
                .count()
        };

        self.opposite_count_cache.set(Some(count));
        count
    }

    /// Trigger manual sync (delegates to operations module)
    pub fn trigger_sync(&mut self) {
        crate::ui::operations::sync::trigger_sync(self);
    }

    /// Run syncs on their own thread from now on, as the terminal app does.
    pub fn enable_background_sync(&mut self) {
        self.sync_in_background = true;
    }

    /// Whether the user has a note open, whose text is in a buffer rather than the
    /// database. A sync that starts now could change the note under them.
    pub(crate) fn is_editing(&self) -> bool {
        matches!(self.state, AppState::NoteView) || self.editing_note_id.is_some()
    }

    /// Wait a moment for a background sync on the way out, so that what was saved last
    /// reaches the server now. If it doesn't, it's still pending for the next run.
    pub fn wait_for_sync(&self, patience: std::time::Duration) {
        if let Some(job) = &self.sync_job {
            job.wait(patience);
        }
    }

    /// Check if auto-sync should run and trigger it if needed
    /// Also handles auto-clearing sync status after timeout
    /// Call this periodically (e.g., on Tick events) to enable background sync
    pub fn check_auto_sync(&mut self) {
        crate::ui::operations::sync::poll_background_sync(self);

        // Auto-clear sync status after 5 seconds. A running sync is still news.
        if let (Some(set_at), None) = (self.sync_status_set_at, &self.sync_job) {
            let now = Instant::now();
            let elapsed = now.duration_since(set_at);
            if elapsed >= std::time::Duration::from_secs(5) {
                self.sync_status = None;
                self.sync_status_set_at = None;
            }
        }

        // Check for SSE messages (real-time sync notifications)
        // This must happen BEFORE the early returns below, as SSE should work
        // even when auto-sync interval is disabled (set to 0)
        self.check_sse_messages();

        // Anything asked for while a note was open, or while another sync ran, goes now
        if !self.sync_paused_until.is_some_and(|until| Instant::now() < until) {
            crate::ui::operations::sync::start_requested_sync(self);
        }

        // A sync that started during an edit could change the note being edited, and one
        // that's running already covers what this would do
        if self.is_editing() || self.sync_job.is_some() {
            return;
        }

        // Check if auto-sync is enabled (interval > 0)
        if self.settings.auto_sync_interval_minutes <= 0 {
            return; // Auto-sync disabled, but SSE still works above
        }

        // Check if sync is configured
        if !self.settings.sync_enabled || self.settings.sync_endpoint.is_none() {
            return; // Sync not configured
        }

        // Check if we're unlocked (have database and key)
        if self.db.is_none() || self.key.is_none() {
            return; // Not unlocked, can't sync
        }

        // The server said to wait, or that this account can't sync at the moment
        let now = Instant::now();
        if self.sync_paused_until.is_some_and(|until| now < until) {
            return;
        }

        // Check time since last auto-sync
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

    /// Start SSE connection if sync is enabled
    pub fn start_sse_if_enabled(&mut self) {
        self.debug_log("SSE: start_sse_if_enabled() called");

        // Only start if sync is enabled and we have an endpoint
        if !self.settings.sync_enabled {
            self.debug_log("SSE: sync not enabled, skipping SSE");
            return;
        }

        let endpoint = match &self.settings.sync_endpoint {
            Some(ep) => ep.clone(),
            None => {
                self.debug_log("SSE: no sync endpoint configured, skipping SSE");
                return;
            }
        };

        // Get the API key (need to decrypt it)
        let db = match &self.db {
            Some(db) => db,
            None => {
                self.debug_log("SSE: no database available, skipping SSE");
                return;
            }
        };

        let key = match &self.key {
            Some(k) => k,
            None => {
                self.debug_log("SSE: no encryption key available, skipping SSE");
                return;
            }
        };

        let sync_repo = SyncRepository::new(db.connection());
        let metadata = match sync_repo.get_metadata() {
            Ok(Some(m)) => m,
            Ok(None) => {
                self.debug_log("SSE: no sync metadata found, skipping SSE");
                return;
            }
            Err(e) => {
                self.debug_log(&format!("SSE: failed to get sync metadata: {}, skipping SSE", e));
                return;
            }
        };

        let encrypted_api_key = match &metadata.api_key {
            Some(k) => k,
            None => {
                self.debug_log("SSE: no API key in sync metadata, skipping SSE");
                return;
            }
        };

        // Decrypt the API key
        let encrypted: crate::crypto::EncryptedData = match serde_json::from_str(encrypted_api_key) {
            Ok(e) => e,
            Err(e) => {
                self.debug_log(&format!("SSE: failed to parse encrypted API key: {}, skipping SSE", e));
                return;
            }
        };

        let api_key = match self.crypto.decrypt_text(&encrypted, key) {
            Ok(k) => k,
            Err(e) => {
                self.debug_log(&format!("SSE: failed to decrypt API key: {}, skipping SSE", e));
                return;
            }
        };

        // Stop existing SSE connection if any
        self.stop_sse();
        self.sse_gave_up = false;

        // Start new SSE connection
        self.debug_log(&format!("SSE: Starting connection to {}", endpoint));
        let config = crate::sse::SseConfig {
            endpoint,
            api_key,
        };

        self.sse_handle = Some(crate::sse::start_sse_thread(config));
        self.debug_log("SSE: SSE thread started");
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
                None => {
                    // No SSE handle - not started or not configured
                    return;
                }
            }
        };

        // Now process the collected messages
        let mut should_sync = false;
        for msg in messages {
            match msg {
                crate::sse::SseMessage::SyncNotification => {
                    self.debug_log("SSE: Received sync notification, triggering sync");
                    self.sync_status = Some(t!("sync.sse_notification").to_string());
                    self.sync_status_set_at = Some(Instant::now());
                    should_sync = true;
                }
                crate::sse::SseMessage::Connected => {
                    self.debug_log("SSE: Connected to server");
                    self.sync_status = Some(t!("sync.sse_connected").to_string());
                    self.sync_status_set_at = Some(Instant::now());
                }
                crate::sse::SseMessage::Disconnected => {
                    self.debug_log("SSE: Disconnected from server (will reconnect)");
                }
                crate::sse::SseMessage::Failed(err) => {
                    self.debug_log(&format!("SSE: Connection failed: {}", err));
                    self.sync_status = Some(t!("sync.sse_failed").to_string());
                    self.sync_status_set_at = Some(Instant::now());
                    self.sse_gave_up = true;
                }
            }
        }

        // Waits its turn, as the note the user has open is what might change
        if should_sync {
            self.sync_requested = Some(self.sync_requested.unwrap_or(false));
        }
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
            AppState::ChangePasswordCurrent { .. } => {
                rendering::credentials::render_change_password(self, frame, "current")
            }
            AppState::ChangePasswordNew { .. } => {
                rendering::credentials::render_change_password(self, frame, "new")
            }
            AppState::ChangePasswordConfirm { .. } => {
                rendering::credentials::render_change_password(self, frame, "confirm")
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
mod lock_tests {
    use super::*;
    use crate::repository::NoteRepository;
    use crate::ui::operations::auth;
    use crossterm::event::KeyEvent;

    /// An app unlocked with a vault on disk, holding one note.
    fn unlocked_app() -> (App, String) {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);

        let password = uuid::Uuid::new_v4().to_string();
        app.password_input = password.clone();
        auth::unlock(&mut app).unwrap();

        let note = Note::new("Private".to_string());
        let repo = NoteRepository::new(app.db.as_ref().unwrap().connection());
        repo.create(&note, app.key.as_ref().unwrap()).unwrap();
        crate::ui::operations::notes::load_notes(&mut app).unwrap();
        (app, password)
    }

    fn ctrl_l() -> KeyEvent {
        KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL)
    }

    #[test]
    fn locking_leaves_nothing_of_the_vault_behind() {
        let (mut app, _) = unlocked_app();
        app.note_input = "A draft".to_string();
        app.credential_input = "typed".to_string();
        app.search_input = "private".to_string();

        app.lock();

        assert!(matches!(app.state, AppState::Locked));
        assert!(app.key.is_none());
        assert!(app.key_manager.is_locked());
        assert!(app.unlock_password.is_none());
        assert!(app.db.is_none());
        assert!(app.notes.is_empty());
        assert!(app.note_input.is_empty());
        assert!(app.credential_input.is_empty());
        assert!(app.search_input.is_empty());
    }

    #[test]
    fn unlocking_again_brings_everything_back() {
        let (mut app, password) = unlocked_app();
        app.lock();

        app.password_input = password;
        auth::unlock(&mut app).unwrap();

        assert!(matches!(app.state, AppState::NoteList));
        assert_eq!(app.notes.len(), 1);
        assert_eq!(app.notes[0].content, "Private");
    }

    #[test]
    fn the_wrong_password_does_not_get_back_in() {
        let (mut app, _) = unlocked_app();
        app.lock();

        app.password_input = uuid::Uuid::new_v4().to_string();

        assert!(auth::unlock(&mut app).is_err());
        assert!(app.key.is_none());
    }

    #[test]
    fn ctrl_l_locks() {
        let (mut app, _) = unlocked_app();

        app.handle_key(ctrl_l()).unwrap();

        assert!(matches!(app.state, AppState::Locked));
    }

    #[test]
    fn ctrl_l_locks_from_any_screen() {
        let (mut app, _) = unlocked_app();
        app.state = AppState::ChangePasswordNew {
            current_password: Zeroizing::new("in memory".to_string()),
            previous: Box::new(AppState::NoteList),
        };

        app.handle_key(ctrl_l()).unwrap();

        assert!(matches!(app.state, AppState::Locked));
    }

    #[test]
    fn the_app_locks_itself_after_the_timeout() {
        let (mut app, _) = unlocked_app();
        app.settings.auto_lock_timeout = 5;
        app.apply_auto_lock_setting();

        app.check_auto_lock();
        assert!(app.is_unlocked());

        app.key_manager.pretend_idle_for(std::time::Duration::from_secs(6 * 60));
        app.check_auto_lock();
        assert!(!app.is_unlocked());
    }

    #[test]
    fn activity_holds_off_the_auto_lock() {
        let (mut app, _) = unlocked_app();
        app.settings.auto_lock_timeout = 5;
        app.apply_auto_lock_setting();

        app.key_manager.pretend_idle_for(std::time::Duration::from_secs(6 * 60));
        app.register_activity();
        app.check_auto_lock();

        assert!(app.is_unlocked());
    }

    #[test]
    fn an_idle_tick_asks_for_no_redraw() {
        let (mut app, _) = unlocked_app();
        app.settings.auto_sync_interval_minutes = 0;

        assert!(!app.on_tick());
    }

    #[test]
    fn a_status_message_expiring_asks_for_a_redraw() {
        let (mut app, _) = unlocked_app();
        app.settings.auto_sync_interval_minutes = 0;
        app.sync_status = Some("Done".to_string());
        app.sync_status_set_at = Some(Instant::now() - std::time::Duration::from_secs(6));

        assert!(app.on_tick());
        assert!(app.sync_status.is_none());
    }

    #[test]
    fn a_filtered_list_survives_a_note_being_added_without_invalidating() {
        let (mut app, _) = unlocked_app();
        assert_eq!(app.filtered_notes().len(), 1);

        app.notes.push(Note::new("Another".to_string()));

        assert_eq!(app.filtered_notes().len(), 2);
    }

    #[test]
    fn locking_saves_what_is_open_in_the_note_view() {
        let (mut app, password) = unlocked_app();
        app.editing_note_id = Some(app.notes[0].id.clone());
        app.note_input = "Edited before the lock".to_string();
        app.state = AppState::NoteView;

        app.lock();

        app.password_input = password;
        auth::unlock(&mut app).unwrap();
        assert_eq!(app.notes[0].content, "Edited before the lock");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Holds one password in memory, and says whether it has been asked to forget it.
    struct RememberedPassword {
        password: String,
        deleted: Arc<Mutex<bool>>,
    }

    impl PasswordStorage for RememberedPassword {
        fn store(&self, _password: &str) -> Result<()> {
            Ok(())
        }

        fn retrieve(&self) -> RetrieveResult {
            RetrieveResult::Found(self.password.clone())
        }

        fn delete(&self) -> Result<()> {
            *self.deleted.lock().unwrap() = true;
            Ok(())
        }

        fn backend_type(&self) -> StorageBackendType {
            StorageBackendType::File
        }
    }

    fn app_remembering(path: PathBuf, password: &str) -> (App, Arc<Mutex<bool>>) {
        let deleted = Arc::new(Mutex::new(false));
        let storage = RememberedPassword { password: password.to_string(), deleted: deleted.clone() };
        (App::with_storage(path, None, Box::new(storage)), deleted)
    }

    #[test]
    fn a_wrong_stored_password_is_forgotten() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.db");
        let password = uuid::Uuid::new_v4().to_string();
        let db = Database::open(&path, &password).unwrap();
        crate::vault::unlock(db.connection(), &CryptoService::new(), &password).unwrap();
        drop(db);

        let (mut app, deleted) = app_remembering(path, &uuid::Uuid::new_v4().to_string());

        assert!(app.try_auto_unlock().is_err());
        assert!(*deleted.lock().unwrap());
    }

    /// The vault can't be opened just now, which says nothing about the password.
    #[test]
    fn a_stored_password_survives_a_vault_it_cannot_reach() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("not-a-file");
        std::fs::create_dir(&path).unwrap();

        let (mut app, deleted) = app_remembering(path, &uuid::Uuid::new_v4().to_string());

        assert!(app.try_auto_unlock().is_err());
        assert!(!*deleted.lock().unwrap());
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
