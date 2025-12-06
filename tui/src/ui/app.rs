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
    io::{self, Write},
    path::PathBuf,
    process::Command,
};
use tempfile::NamedTempFile;

use crate::{
    crypto::{CryptoService, KeyManager},
    db::Database,
    models::{Note, UserSettings},
    repository::{EncryptionRepository, NoteRepository, SettingsRepository},
};

/// Application state
pub enum AppState {
    /// Locked - password input screen
    Locked,
    /// Unlocked - main note list
    NoteList,
    /// Viewing/editing a note
    NoteView,
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
    /// Current error message
    pub error: Option<String>,
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
}

impl App {
    /// Create a new app
    pub fn new(db_path: PathBuf) -> Result<Self> {
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
            error: None,
            db_path,
            db: None,
            key: None,
            key_manager: KeyManager::new(),
            crypto: CryptoService::new(),
            notes: Vec::new(),
            selected_note: 0,
            editing_note_id: None,
            settings: UserSettings::default(),
        })
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        // Handle help screen separately to avoid borrow issues
        if let AppState::Help { .. } = &self.state {
            return self.handle_help_key(key);
        }

        match &self.state {
            AppState::Locked => self.handle_locked_key(key)?,
            AppState::NoteList => self.handle_note_list_key(key)?,
            AppState::NoteView => self.handle_note_view_key(key)?,
            AppState::Quit => {}
            AppState::Help { .. } => unreachable!(), // Handled above
        }
        Ok(())
    }

    /// Handle key events in locked state
    fn handle_locked_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
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
            KeyCode::Esc | KeyCode::Char('q') => {
                self.state = AppState::Quit;
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events in note list state
    fn handle_note_list_key(&mut self, key: KeyEvent) -> Result<()> {
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
            KeyCode::Char('n') => {
                // New note
                self.note_input.clear();
                self.current_tags.clear();
                self.editing_note_id = None; // Creating new note
                self.state = AppState::NoteView;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Enter => {
                // Open selected note
                if !self.notes.is_empty() {
                    let note = &self.notes[self.selected_note];
                    self.note_input = note.content.clone();
                    self.current_tags = note.tags.clone();
                    self.editing_note_id = Some(note.id.clone()); // Editing existing note
                    self.state = AppState::NoteView;
                    self.input_mode = InputMode::Normal;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if !self.notes.is_empty() && self.selected_note < self.notes.len() - 1 {
                    self.selected_note += 1;
                }
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if self.selected_note > 0 {
                    self.selected_note -= 1;
                }
            }
            KeyCode::Char('d') => {
                // Delete selected note
                if !self.notes.is_empty() {
                    self.delete_note()?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events in note view state
    fn handle_note_view_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
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
        let key = self
            .crypto
            .derive_key(&self.password_input, &salt, iterations)?;

        self.key_manager.set_master_key(key);
        self.key = Some(key);
        self.db = Some(db);

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

    /// Render note list
    fn render_note_list(&self, frame: &mut Frame) {
        let size = frame.area();

        let block = Block::default()
            .title("Jottery - Notes")
            .borders(Borders::ALL);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(size);

        let items: Vec<ListItem> = self
            .notes
            .iter()
            .enumerate()
            .map(|(i, note)| {
                let content = note.content.lines().next().unwrap_or("");
                let preview = if content.len() > 50 {
                    format!("{}...", &content[..50])
                } else {
                    content.to_string()
                };

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

        let list = List::new(items).block(block);

        let help = Paragraph::new("n: new | Enter: open | d: delete | j/k: navigate | Ctrl+q: quit")
            .style(Style::default().fg(Color::DarkGray))
            .alignment(Alignment::Center);

        frame.render_widget(list, chunks[0]);
        frame.render_widget(help, chunks[1]);
    }

    /// Render note view
    fn render_note_view(&self, frame: &mut Frame) {
        let size = frame.area();

        let mode_text = match self.input_mode {
            InputMode::Normal => "NORMAL",
            InputMode::Insert => "INSERT",
            InputMode::Tag => "TAG",
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
            InputMode::Normal => {
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
            Line::from("  q / Esc               Quit application"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE LIST", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  n                     Create new note"),
            Line::from("  Enter                 Open selected note"),
            Line::from("  d                     Delete selected note"),
            Line::from("  j / ↓                 Move down"),
            Line::from("  k / ↑                 Move up"),
            Line::from("  ?                     Show this help"),
            Line::from("  Ctrl+q                Quit application"),
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
            Line::from("  Backspace             Delete character"),
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
