use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame,
};
use std::path::PathBuf;

use crate::{
    crypto::{CryptoService, KeyManager},
    db::Database,
    models::{Note, UserSettings},
    repository::{NoteRepository, SettingsRepository},
};

/// Application state
pub enum AppState {
    /// Locked - password input screen
    Locked,
    /// Unlocked - main note list
    NoteList,
    /// Viewing/editing a note
    NoteView,
    /// Quit
    Quit,
}

/// Current input mode
pub enum InputMode {
    /// Normal mode (navigation)
    Normal,
    /// Insert mode (typing)
    Insert,
}

/// Application
pub struct App {
    /// Current state
    pub state: AppState,
    /// Input mode
    pub input_mode: InputMode,
    /// Password input buffer
    pub password_input: String,
    /// Note content input buffer
    pub note_input: String,
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
    /// Settings
    settings: UserSettings,
}

impl App {
    /// Create a new app
    pub fn new(db_path: PathBuf) -> Result<Self> {
        Ok(Self {
            state: AppState::Locked,
            input_mode: InputMode::Normal,
            password_input: String::new(),
            note_input: String::new(),
            error: None,
            db_path,
            db: None,
            key: None,
            key_manager: KeyManager::new(),
            crypto: CryptoService::new(),
            notes: Vec::new(),
            selected_note: 0,
            settings: UserSettings::default(),
        })
    }

    /// Handle key events
    pub fn handle_key(&mut self, key: KeyEvent) -> Result<()> {
        match &self.state {
            AppState::Locked => self.handle_locked_key(key)?,
            AppState::NoteList => self.handle_note_list_key(key)?,
            AppState::NoteView => self.handle_note_view_key(key)?,
            AppState::Quit => {}
        }
        Ok(())
    }

    /// Handle key events in locked state
    fn handle_locked_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Enter => {
                // Try to unlock
                self.error = None;
                if let Err(e) = self.unlock() {
                    self.error = Some(format!("Failed to unlock: {}", e));
                    self.password_input.clear();
                }
            }
            KeyCode::Char(c) => {
                self.password_input.push(c);
            }
            KeyCode::Backspace => {
                self.password_input.pop();
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
            KeyCode::Char('n') => {
                // New note
                self.note_input.clear();
                self.state = AppState::NoteView;
                self.input_mode = InputMode::Insert;
            }
            KeyCode::Enter => {
                // Open selected note
                if !self.notes.is_empty() {
                    let note = &self.notes[self.selected_note];
                    self.note_input = note.content.clone();
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
        }
        Ok(())
    }

    /// Unlock the database
    fn unlock(&mut self) -> Result<()> {
        // Open database
        let db = Database::open(&self.db_path, &self.password_input)
            .context("Failed to open database")?;

        // Derive key
        let salt = if db.is_initialized()? {
            // Load existing salt
            // For now, we'll generate a new salt if not initialized
            self.crypto.generate_salt()
        } else {
            // Generate new salt for first-time setup
            self.crypto.generate_salt()
        };

        let key = self
            .crypto
            .derive_key(&self.password_input, &salt, 256_000)?;

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

            // Check if we're editing an existing note or creating new one
            if !self.notes.is_empty()
                && self.selected_note < self.notes.len()
                && !self.note_input.is_empty()
            {
                // Update existing note
                let note = &mut self.notes[self.selected_note];
                note.content = self.note_input.clone();
                note.touch();
                repo.update(note, key)?;
            } else if !self.note_input.is_empty() {
                // Create new note
                let mut note = Note::new(self.note_input.clone());
                repo.create(&note, key)?;
                self.notes.insert(0, note);
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

    /// Render the UI
    pub fn render(&mut self, frame: &mut Frame) {
        match &self.state {
            AppState::Locked => self.render_locked(frame),
            AppState::NoteList => self.render_note_list(frame),
            AppState::NoteView => self.render_note_view(frame),
            AppState::Quit => {}
        }
    }

    /// Render locked screen
    fn render_locked(&self, frame: &mut Frame) {
        let size = frame.area();

        let block = Block::default()
            .title("Jottery TUI - Unlock")
            .borders(Borders::ALL);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(2)
            .constraints([
                Constraint::Length(3),
                Constraint::Length(3),
                Constraint::Min(0),
            ])
            .split(size);

        let password_text = "*".repeat(self.password_input.len());
        let password = Paragraph::new(password_text)
            .block(Block::default().title("Password").borders(Borders::ALL));

        frame.render_widget(block, size);
        frame.render_widget(password, chunks[0]);

        if let Some(err) = &self.error {
            let error = Paragraph::new(err.clone())
                .style(Style::default().fg(Color::Red))
                .block(Block::default().title("Error").borders(Borders::ALL));
            frame.render_widget(error, chunks[1]);
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
        };

        let block = Block::default()
            .title(format!("Note - {}", mode_text))
            .borders(Borders::ALL);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(size);

        let text = Paragraph::new(self.note_input.clone())
            .block(block)
            .wrap(Wrap { trim: false });

        let help = match self.input_mode {
            InputMode::Normal => {
                Paragraph::new("i: insert mode | q/Esc: save & quit")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center)
            }
            InputMode::Insert => {
                Paragraph::new("Esc: normal mode | Type to edit")
                    .style(Style::default().fg(Color::DarkGray))
                    .alignment(Alignment::Center)
            }
        };

        frame.render_widget(text, chunks[0]);
        frame.render_widget(help, chunks[1]);
    }

    /// Check if app should quit
    pub fn should_quit(&self) -> bool {
        matches!(self.state, AppState::Quit)
    }
}
