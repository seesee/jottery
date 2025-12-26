use anyhow::{Context, Result};
use crossterm::{
    event::{KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen, Clear, ClearType},
    cursor::MoveTo,
};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
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
    time::Instant,
    sync::{Arc, Mutex},
};
use tempfile::NamedTempFile;

use crate::{
    crypto::{CryptoService, KeyManager},
    db::Database,
    models::{Attachment, Note, UserSettings, sync::SyncCredentials},
    repository::{attachment::AttachmentRepository, EncryptionRepository, NoteRepository, SettingsRepository, sync::SyncRepository},
};

/// Strip markdown formatting from text (for display in note list)
fn strip_markdown(text: &str) -> String {
    let mut result = text.to_string();

    // Remove markdown headers (# ## ### etc.)
    if let Some(stripped) = result.strip_prefix('#') {
        let mut chars = stripped.chars();
        // Skip additional # characters
        while chars.as_str().starts_with('#') {
            chars.next();
        }
        // Skip whitespace after #
        result = chars.as_str().trim_start().to_string();
    }

    // Remove bold (**text** or __text__)
    result = result.replace("**", "").replace("__", "");

    // Remove italic (*text* or _text_) - simple approach
    let mut cleaned = String::new();
    let mut chars = result.chars().peekable();
    let mut in_code = false;

    while let Some(ch) = chars.next() {
        match ch {
            '`' => {
                in_code = !in_code;
                cleaned.push(ch);
            }
            '*' | '_' if !in_code => {
                // Skip single * or _ used for emphasis
                continue;
            }
            '[' if !in_code => {
                // Handle links [text](url) - extract text only
                let mut link_text = String::new();
                let mut found_closing = false;

                while let Some(c) = chars.next() {
                    if c == ']' {
                        found_closing = true;
                        break;
                    }
                    link_text.push(c);
                }

                if found_closing {
                    // Skip the (url) part
                    if chars.peek() == Some(&'(') {
                        chars.next(); // skip (
                        while let Some(c) = chars.next() {
                            if c == ')' {
                                break;
                            }
                        }
                    }
                    cleaned.push_str(&link_text);
                } else {
                    cleaned.push('[');
                    cleaned.push_str(&link_text);
                }
            }
            _ => cleaned.push(ch),
        }
    }

    cleaned.trim().to_string()
}

/// Render markdown for terminal display using pulldown-cmark parser
fn render_markdown_for_terminal(
    content: &str,
    syntax_highlighter: &crate::ui::syntax::SyntaxHighlighter,
    debug_log: &Option<Arc<Mutex<File>>>
) -> Vec<Line<'static>> {
    use pulldown_cmark::{Parser, Event, Tag, TagEnd, CodeBlockKind, Options};
    use ratatui::style::{Style, Modifier, Color};
    use ratatui::text::{Line, Span};
    use crate::models::SyntaxLanguage;

    // Helper to write debug messages
    let log_debug = |msg: &str| {
        if let Some(log) = debug_log {
            if let Ok(mut file) = log.lock() {
                let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
            }
        }
    };

    let mut lines = Vec::new();
    let mut current_line_spans: Vec<Span<'static>> = Vec::new();
    let mut current_style = Style::default();
    let mut in_code_block = false;
    let mut code_block_content = String::new();
    let mut code_block_lang = String::new();
    let mut in_table = false;
    let mut in_table_head = false;
    let mut in_list_item = false;
    let mut list_item_started = false;
    let mut table_cells: Vec<String> = Vec::new();
    let mut current_cell_text = String::new();
    let mut table_header_rows: Vec<Vec<String>> = Vec::new();
    let mut table_body_rows: Vec<Vec<String>> = Vec::new();

    // Style stack for nested formatting
    let mut style_stack: Vec<Style> = vec![Style::default()];

    // Enable extensions: tables, strikethrough, tasklists, footnotes
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(content, options);

    for event in parser {
        match event {
            Event::Start(tag) => {
                match tag {
                    Tag::Heading { .. } => {
                        // Flush any current line before heading
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        // Add blank line before heading (for spacing)
                        if !lines.is_empty() {
                            lines.push(Line::raw(""));
                        }

                        // Headers in cyan + bold
                        let header_style = Style::default()
                            .fg(Color::Cyan)
                            .add_modifier(Modifier::BOLD);
                        style_stack.push(header_style);
                        current_style = header_style;
                    }
                    Tag::Emphasis => {
                        // Italic -> cyan color (better terminal support than ITALIC modifier)
                        let italic_style = current_style.fg(Color::Cyan);
                        style_stack.push(italic_style);
                        current_style = italic_style;
                    }
                    Tag::Strong => {
                        // Bold -> white + BOLD
                        let bold_style = current_style
                            .fg(Color::White)
                            .add_modifier(Modifier::BOLD);
                        style_stack.push(bold_style);
                        current_style = bold_style;
                    }
                    Tag::Link { .. } => {
                        // Links -> blue + underlined
                        let link_style = current_style
                            .fg(Color::Blue)
                            .add_modifier(Modifier::UNDERLINED);
                        style_stack.push(link_style);
                        current_style = link_style;
                    }
                    Tag::CodeBlock(kind) => {
                        // Flush current line before code block
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }

                        in_code_block = true;
                        code_block_content.clear();
                        code_block_lang = match kind {
                            CodeBlockKind::Fenced(lang) => lang.to_string(),
                            CodeBlockKind::Indented => String::new(),
                        };
                    }
                    Tag::Table(_) => {
                        // Flush current line before table
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        in_table = true;
                        table_header_rows.clear();
                        table_body_rows.clear();
                        log_debug(">>> Tag::Table start");
                    }
                    Tag::TableHead => {
                        in_table_head = true;
                        log_debug(">>> Tag::TableHead start (setting in_table_head=true)");
                    }
                    Tag::TableRow => {
                        table_cells.clear();
                        log_debug(&format!(">>> Tag::TableRow start (in_table_head={})", in_table_head));
                    }
                    Tag::TableCell => {
                        current_cell_text.clear();
                    }
                    Tag::Paragraph => {
                        // Paragraphs just group text, no special styling needed
                    }
                    Tag::List(_) => {
                        // List started - no special action needed
                    }
                    Tag::Item => {
                        // Flush current line before list item
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        in_list_item = true;
                        list_item_started = false;
                    }
                    _ => {}
                }
            }
            Event::End(tag_end) => {
                match tag_end {
                    TagEnd::Heading(_) => {
                        // Pop style from stack
                        if style_stack.len() > 1 {
                            style_stack.pop();
                            current_style = *style_stack.last().unwrap();
                        }
                        // Headings end with a line break
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                    }
                    TagEnd::Emphasis | TagEnd::Strong | TagEnd::Link => {
                        // Pop style from stack
                        if style_stack.len() > 1 {
                            style_stack.pop();
                            current_style = *style_stack.last().unwrap();
                        }
                    }
                    TagEnd::CodeBlock => {
                        in_code_block = false;

                        // Determine syntax language
                        let lang = match code_block_lang.to_lowercase().as_str() {
                            "javascript" | "js" => SyntaxLanguage::Javascript,
                            "python" | "py" => SyntaxLanguage::Python,
                            "markdown" | "md" => SyntaxLanguage::Markdown,
                            "json" => SyntaxLanguage::Json,
                            "html" => SyntaxLanguage::Html,
                            "css" => SyntaxLanguage::Css,
                            "sql" => SyntaxLanguage::Sql,
                            "bash" | "sh" => SyntaxLanguage::Bash,
                            "perl" | "pl" => SyntaxLanguage::Perl,
                            _ => SyntaxLanguage::Plain,
                        };

                        // Apply syntax highlighting
                        let highlighted = syntax_highlighter.highlight(&code_block_content, lang);

                        // Convert borrowed lines to owned
                        for line in highlighted.lines {
                            let owned_line: Line<'static> = Line::from(
                                line.spans.into_iter()
                                    .map(|span| Span::styled(span.content.to_string(), span.style))
                                    .collect::<Vec<_>>()
                            );
                            lines.push(owned_line);
                        }

                        code_block_content.clear();
                        code_block_lang.clear();
                        // Add blank line after code block
                        lines.push(Line::raw(""));
                    }
                    TagEnd::TableCell => {
                        // Save the current cell text
                        table_cells.push(current_cell_text.clone());
                        current_cell_text.clear();
                    }
                    TagEnd::TableRow => {
                        // Save the current row to appropriate vector
                        if !table_cells.is_empty() {
                            log_debug(&format!("TableRow end - in_table_head={}, cells={:?}", in_table_head, table_cells));
                            if in_table_head {
                                table_header_rows.push(table_cells.clone());
                            } else {
                                table_body_rows.push(table_cells.clone());
                            }
                            table_cells.clear();
                        }
                    }
                    TagEnd::TableHead => {
                        // Save header cells (they come directly in TableHead, not in a TableRow)
                        if !table_cells.is_empty() {
                            log_debug(&format!(">>> TagEnd::TableHead - saving header cells: {:?}", table_cells));
                            table_header_rows.push(table_cells.clone());
                            table_cells.clear();
                        }
                        in_table_head = false;
                        log_debug(">>> TagEnd::TableHead (setting in_table_head=false)");
                    }
                    TagEnd::Table => {
                        in_table = false;
                        log_debug(&format!("Table end - header_rows={}, body_rows={}", table_header_rows.len(), table_body_rows.len()));

                        // Calculate column widths from both header and body
                        let mut col_widths: Vec<usize> = Vec::new();

                        // Check header rows
                        for row in &table_header_rows {
                            for (i, cell) in row.iter().enumerate() {
                                let width = cell.len();
                                if i >= col_widths.len() {
                                    col_widths.push(width);
                                } else if width > col_widths[i] {
                                    col_widths[i] = width;
                                }
                            }
                        }

                        // Check body rows
                        for row in &table_body_rows {
                            for (i, cell) in row.iter().enumerate() {
                                let width = cell.len();
                                if i >= col_widths.len() {
                                    col_widths.push(width);
                                } else if width > col_widths[i] {
                                    col_widths[i] = width;
                                }
                            }
                        }

                        // Render header rows (bold)
                        for row in &table_header_rows {
                            let mut row_text = String::from("| ");
                            for (i, cell) in row.iter().enumerate() {
                                let width = if i < col_widths.len() { col_widths[i] } else { 0 };
                                row_text.push_str(&format!("{:<width$} | ", cell, width = width));
                            }
                            lines.push(Line::styled(
                                row_text,
                                Style::default().add_modifier(Modifier::BOLD)
                            ));
                        }

                        // Add separator line after header
                        if !table_header_rows.is_empty() {
                            let mut separator = String::from("|-");
                            for &width in &col_widths {
                                separator.push_str(&"-".repeat(width));
                                separator.push_str("-|-");
                            }
                            // Remove trailing dash
                            separator.pop();
                            lines.push(Line::styled(separator, Style::default().fg(Color::DarkGray)));
                        }

                        // Render body rows (normal)
                        for row in &table_body_rows {
                            let mut row_text = String::from("| ");
                            for (i, cell) in row.iter().enumerate() {
                                let width = if i < col_widths.len() { col_widths[i] } else { 0 };
                                row_text.push_str(&format!("{:<width$} | ", cell, width = width));
                            }
                            lines.push(Line::raw(row_text));
                        }

                        table_header_rows.clear();
                        table_body_rows.clear();
                        // Add blank line after table
                        lines.push(Line::raw(""));
                    }
                    TagEnd::Paragraph => {
                        // End paragraph with a line break
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                        // Don't add extra blank lines - let block spacing handle it
                    }
                    TagEnd::Item => {
                        // End list item with line break
                        in_list_item = false;
                        if !current_line_spans.is_empty() {
                            lines.push(Line::from(current_line_spans.clone()));
                            current_line_spans.clear();
                        }
                    }
                    TagEnd::List(_) => {
                        // Add blank line after list
                        lines.push(Line::raw(""));
                    }
                    _ => {}
                }
            }
            Event::Text(text) => {
                if in_code_block {
                    code_block_content.push_str(&text);
                } else if in_table {
                    // Collect text for current table cell
                    current_cell_text.push_str(&text);
                } else if in_list_item && !list_item_started {
                    // Add bullet point at start of list item
                    list_item_started = true;
                    current_line_spans.push(Span::raw("• "));
                    current_line_spans.push(Span::styled(text.to_string(), current_style));
                } else {
                    current_line_spans.push(Span::styled(text.to_string(), current_style));
                }
            }
            Event::Code(code) => {
                if in_table {
                    // In tables, just add code text without styling
                    current_cell_text.push_str(&code);
                } else {
                    // Add bullet point at start of list item if not started
                    if in_list_item && !list_item_started {
                        list_item_started = true;
                        current_line_spans.push(Span::raw("• "));
                    }
                    // Inline code -> yellow
                    current_line_spans.push(Span::styled(
                        code.to_string(),
                        Style::default().fg(Color::Yellow)
                    ));
                }
            }
            Event::SoftBreak => {
                // Soft break in markdown (single newline) - keep on same line in most contexts
                if in_table {
                    // In tables, add space to cell text
                    current_cell_text.push(' ');
                } else {
                    // Add a space for soft breaks in regular text
                    current_line_spans.push(Span::raw(" "));
                }
            }
            Event::HardBreak => {
                // Hard break (two spaces + newline or <br>) - always break line
                if !current_line_spans.is_empty() {
                    lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                }
            }
            Event::Rule => {
                // Flush current line before rule
                if !current_line_spans.is_empty() {
                    lines.push(Line::from(current_line_spans.clone()));
                    current_line_spans.clear();
                }
                // Horizontal rule
                lines.push(Line::styled(
                    "─".repeat(80),
                    Style::default().fg(Color::DarkGray)
                ));
                lines.push(Line::raw(""));
            }
            Event::TaskListMarker(checked) => {
                // Render task list checkbox
                let checkbox = if checked { "[x] " } else { "[ ] " };
                current_line_spans.push(Span::raw(checkbox));
            }
            _ => {}
        }
    }

    // Flush any remaining spans
    if !current_line_spans.is_empty() {
        lines.push(Line::from(current_line_spans));
    }

    // Return empty line if no content
    if lines.is_empty() {
        lines.push(Line::raw(""));
    }

    lines
}

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
    AttachmentPath,
}

/// Current view mode
pub enum ViewMode {
    /// Normal note list view
    NoteList,
    /// Recycle bin view (deleted notes)
    RecycleBin,
    /// Attachment viewer modal
    AttachmentViewer,
}

/// Application
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
    /// Preview scroll offset (number of lines scrolled down)
    preview_scroll_offset: usize,
    /// Currently editing note ID (None = creating new note)
    editing_note_id: Option<String>,
    /// Settings
    settings: UserSettings,
    /// Sync credentials input buffer (for manual paste)
    credential_input: String,
    /// Debug log file (for troubleshooting)
    debug_log: Option<Arc<Mutex<File>>>,
    /// Syntax highlighter for code preview
    syntax_highlighter: crate::ui::syntax::SyntaxHighlighter,
    /// Last auto-sync time (for periodic sync)
    last_auto_sync: Option<Instant>,
    /// When sync status was set (for auto-clearing)
    sync_status_set_at: Option<Instant>,
    /// Current color scheme (cached from settings)
    color_scheme: crate::ui::ColorScheme,
    /// Selected attachment index in preview pane
    selected_attachment: usize,
    /// File path input buffer (when adding attachments)
    attachment_path_input: String,
    /// Whether chafa is available for image preview (lazy-loaded)
    chafa_available: Option<bool>,
    /// Track if 'a' key was pressed (for a1, a2 sequence)
    last_key_was_a: bool,
    /// Show force resync confirmation modal
    show_force_sync_confirm: bool,
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
            attachment_path_input: String::new(),
            chafa_available: None,
            last_key_was_a: false,
            show_force_sync_confirm: false,
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
            AppState::ShowSyncCredentials { .. } => self.handle_show_credentials_key(key)?,
            AppState::InputSyncCredentials { .. } => self.handle_input_credentials_key(key)?,
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
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                // Toggle remember password checkbox
                self.remember_password_checkbox = !self.remember_password_checkbox;
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

        // Handle attachment path input mode
        if matches!(self.input_mode, InputMode::AttachmentPath) {
            match key.code {
                KeyCode::Enter => {
                    // Add attachment from file path
                    let path = self.attachment_path_input.clone();
                    self.attachment_path_input.clear();
                    self.input_mode = InputMode::Normal;

                    if !path.is_empty() {
                        if let Err(e) = self.add_attachment_to_current_note(&path) {
                            self.error = Some(format!("Failed to add attachment: {}", e));
                        }
                    }
                }
                KeyCode::Esc => {
                    // Cancel attachment input
                    self.attachment_path_input.clear();
                    self.input_mode = InputMode::Normal;
                }
                KeyCode::Backspace => {
                    self.attachment_path_input.pop();
                }
                KeyCode::Char(c) => {
                    self.attachment_path_input.push(c);
                }
                _ => {}
            }
            return Ok(());
        }

        // Handle search mode
        if self.search_active {
            match key.code {
                KeyCode::Esc => {
                    self.search_active = false;
                    self.search_input.clear();
                    self.selected_note = 0;
                }
                KeyCode::Char('a') => {
                    // Open attachment viewer modal (works in search mode too)
                    let filtered = self.filtered_notes();
                    if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let note = filtered[self.selected_note];
                        if !note.attachments.is_empty() {
                            self.view_mode = ViewMode::AttachmentViewer;
                            self.selected_attachment = 0;
                        } else {
                            self.error = Some("No attachments in this note".to_string());
                        }
                    }
                }
                KeyCode::Enter => {
                    // Exit search and edit selected note directly
                    if !self.filtered_notes().is_empty() {
                        let filtered = self.filtered_notes();
                        if self.selected_note < filtered.len() {
                            // Clone the data we need before modifying self
                            let content = filtered[self.selected_note].content.clone();
                            let note_id = filtered[self.selected_note].id.clone();
                            let syntax_lang = filtered[self.selected_note].syntax_language;
                            let tags = filtered[self.selected_note].tags.clone();

                            // Set up for editing
                            self.note_input = content;
                            self.note_syntax = syntax_lang;
                            self.current_tags = tags;
                            self.editing_note_id = Some(note_id.clone());
                            self.search_input.clear();
                            self.search_active = false;

                            // Open external editor immediately
                            if let Ok(new_content) = self.edit_with_external_editor() {
                                self.note_input = new_content;
                                // Save the note
                                if let Err(e) = self.save_note() {
                                    self.error = Some(format!("Failed to save note: {}", e));
                                }
                                // Reload notes to refresh the list
                                if let Err(e) = self.load_notes() {
                                    self.error = Some(format!("Failed to reload notes: {}", e));
                                }
                            }

                            // Clear editing state
                            self.editing_note_id = None;
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
                    // Handle based on context
                    if self.show_force_sync_confirm {
                        // Confirm force full sync
                        self.show_force_sync_confirm = false;
                        self.force_full_sync();
                    } else {
                        // Normal sync
                        self.trigger_sync();
                    }
                }
                KeyCode::Char('Y') => {
                    // Show force full sync confirmation
                    self.show_force_sync_confirm = true;
                }
                KeyCode::Char('n') if self.show_force_sync_confirm => {
                    // Cancel force sync
                    self.show_force_sync_confirm = false;
                }
                KeyCode::Char('/') => {
                    // Enter search mode (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        self.search_active = true;
                        self.search_input.clear();
                    }
                }
                KeyCode::Char('n') => {
                    // New note - open editor immediately (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        self.note_input.clear();
                        self.note_syntax = crate::models::SyntaxLanguage::default();
                        self.current_tags.clear();
                        self.editing_note_id = None;

                        // Open external editor immediately
                        if let Ok(new_content) = self.edit_with_external_editor() {
                            self.note_input = new_content;
                            // Save the note
                            if let Err(e) = self.save_note() {
                                self.error = Some(format!("Failed to save note: {}", e));
                            }
                            // Reload notes to refresh the list
                            if let Err(e) = self.load_notes() {
                                self.error = Some(format!("Failed to reload notes: {}", e));
                            }
                        }
                    }
                }
                KeyCode::Char('i') | KeyCode::Enter => {
                    // In attachment viewer: view selected attachment
                    if matches!(self.view_mode, ViewMode::AttachmentViewer) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                            let note = filtered[self.selected_note];
                            if self.selected_attachment < note.attachments.len() {
                                let attachment = note.attachments[self.selected_attachment].clone();
                                if let Err(e) = self.view_attachment(&attachment) {
                                    self.error = Some(format!("Failed to view attachment: {}", e));
                                }
                            }
                        }
                    }
                    // Edit selected note directly with external editor (only in note list view)
                    else if matches!(self.view_mode, ViewMode::NoteList) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                        // Clone data before modifying self
                        let content = filtered[self.selected_note].content.clone();
                        let note_id = filtered[self.selected_note].id.clone();
                        let syntax_lang = filtered[self.selected_note].syntax_language;
                        let tags = filtered[self.selected_note].tags.clone();

                        // Set up for editing
                        self.note_input = content;
                        self.note_syntax = syntax_lang;
                        self.current_tags = tags;
                        self.editing_note_id = Some(note_id.clone());

                        // Open external editor immediately
                        if let Ok(new_content) = self.edit_with_external_editor() {
                            self.note_input = new_content;
                            // Save the note
                            if let Err(e) = self.save_note() {
                                self.error = Some(format!("Failed to save note: {}", e));
                            }
                            // Reload notes to refresh the list
                            if let Err(e) = self.load_notes() {
                                self.error = Some(format!("Failed to reload notes: {}", e));
                            }
                        }

                        // Clear editing state
                        self.editing_note_id = None;
                        }
                    }
                }
                KeyCode::Char('j') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    // Navigate attachments down (Shift+j)
                    let filtered = self.filtered_notes();
                    if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let note = filtered[self.selected_note];
                        if !note.attachments.is_empty() && self.selected_attachment < note.attachments.len() - 1 {
                            self.selected_attachment += 1;
                        }
                    }
                }
                KeyCode::Char('k') if key.modifiers.contains(KeyModifiers::SHIFT) => {
                    // Navigate attachments up (Shift+k)
                    if self.selected_attachment > 0 {
                        self.selected_attachment -= 1;
                    }
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    if matches!(self.view_mode, ViewMode::AttachmentViewer) {
                        // Navigate attachments in viewer
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                            let note = filtered[self.selected_note];
                            if self.selected_attachment < note.attachments.len().saturating_sub(1) {
                                self.selected_attachment += 1;
                            }
                        }
                    } else {
                        // Navigate notes
                        let note_count = self.filtered_notes().len();
                        if note_count > 0 && self.selected_note < note_count - 1 {
                            self.selected_note += 1;
                            self.preview_scroll_offset = 0;
                        }
                    }
                }
                KeyCode::Up | KeyCode::Char('k') => {
                    if matches!(self.view_mode, ViewMode::AttachmentViewer) {
                        // Navigate attachments in viewer
                        if self.selected_attachment > 0 {
                            self.selected_attachment -= 1;
                        }
                    } else {
                        // Navigate notes
                        if self.selected_note > 0 {
                            self.selected_note -= 1;
                            self.preview_scroll_offset = 0;
                        }
                    }
                }
                KeyCode::Char('p') => {
                    // Toggle pin on selected note (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                            let note_id = filtered[self.selected_note].id.clone();
                            if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                            note.pinned = !note.pinned;
                            // Update modified_at to match web client behavior (triggers sync)
                            note.modified_at = chrono::Utc::now();
                            // Increment version for optimistic locking
                            note.version += 1;

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
                }
                KeyCode::Char('t') => {
                    // Edit tags for selected note (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let content = filtered[self.selected_note].content.clone();
                        let note_id = filtered[self.selected_note].id.clone();
                        let syntax_lang = filtered[self.selected_note].syntax_language;
                        let tags = filtered[self.selected_note].tags.clone();

                        // Set up for tag editing
                        self.note_input = content;
                        self.note_syntax = syntax_lang;
                        self.current_tags = tags;
                        self.editing_note_id = Some(note_id);
                        self.state = AppState::NoteView;
                        self.input_mode = InputMode::Tag;
                        self.tag_input.clear();
                        }
                    }
                }
                KeyCode::Char('l') => {
                    // Cycle syntax language forward for selected note (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let note_id = filtered[self.selected_note].id.clone();
                        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                            note.syntax_language = note.syntax_language.next();

                            // Save to database
                            if let (Some(db), Some(key)) = (&self.db, &self.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    self.error = Some(format!("Failed to update syntax language: {}", e));
                                }
                            }
                        }
                        }
                    }
                }
                KeyCode::Char('L') => {
                    // Cycle syntax language backward for selected note (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let note_id = filtered[self.selected_note].id.clone();
                        if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                            note.syntax_language = note.syntax_language.prev();

                            // Save to database
                            if let (Some(db), Some(key)) = (&self.db, &self.key) {
                                let repo = NoteRepository::new(db.connection());
                                if let Err(e) = repo.update(note, key) {
                                    self.error = Some(format!("Failed to update syntax language: {}", e));
                                }
                            }
                        }
                        }
                    }
                }
                KeyCode::Char('r') => {
                    // Toggle recycle bin view or restore note
                    match self.view_mode {
                        ViewMode::NoteList => {
                            // Switch to recycle bin view
                            self.view_mode = ViewMode::RecycleBin;
                            self.selected_note = 0;
                            self.preview_scroll_offset = 0;
                            if let Err(e) = self.load_deleted_notes() {
                                self.error = Some(format!("Failed to load deleted notes: {}", e));
                            }
                        }
                        ViewMode::RecycleBin => {
                            // Restore selected note
                            if let Err(e) = self.restore_note() {
                                self.error = Some(format!("Failed to restore note: {}", e));
                            }
                        }
                        ViewMode::AttachmentViewer => {
                            // 'r' does nothing in attachment viewer
                        }
                    }
                }
                KeyCode::Char('E') => {
                    // Empty recycle bin (only in recycle bin view)
                    if matches!(self.view_mode, ViewMode::RecycleBin) {
                        if let Err(e) = self.empty_trash() {
                            self.error = Some(format!("Failed to empty trash: {}", e));
                        }
                    }
                }
                KeyCode::Esc => {
                    // Cancel force sync confirmation if showing
                    if self.show_force_sync_confirm {
                        self.show_force_sync_confirm = false;
                    } else if matches!(self.view_mode, ViewMode::AttachmentViewer) {
                        // Exit attachment viewer
                        self.view_mode = ViewMode::NoteList;
                    } else if matches!(self.view_mode, ViewMode::RecycleBin) {
                        // Exit recycle bin view
                        self.view_mode = ViewMode::NoteList;
                        self.selected_note = 0;
                        self.preview_scroll_offset = 0;
                        // Reload normal notes
                        if let Err(e) = self.load_notes() {
                            self.error = Some(format!("Failed to reload notes: {}", e));
                        }
                    }
                }
                // Vim-style preview scrolling (must come before plain 'd' key)
                KeyCode::Char('d') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl-d: scroll preview down half page (10 lines)
                    self.preview_scroll_offset = self.preview_scroll_offset.saturating_add(10);
                }
                KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl-u: scroll preview up half page (10 lines)
                    self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(10);
                }
                KeyCode::Char('f') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl-f: scroll preview down full page (20 lines)
                    self.preview_scroll_offset = self.preview_scroll_offset.saturating_add(20);
                }
                KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                    // Ctrl-b: scroll preview up full page (20 lines)
                    self.preview_scroll_offset = self.preview_scroll_offset.saturating_sub(20);
                }
                KeyCode::Char('d') | KeyCode::Delete => {
                    // In attachment viewer: delete selected attachment
                    if matches!(self.view_mode, ViewMode::AttachmentViewer) {
                        if let Err(e) = self.delete_current_attachment() {
                            self.error = Some(format!("Failed to delete attachment: {}", e));
                        }
                    }
                    // Delete selected note (only in note list view)
                    else if matches!(self.view_mode, ViewMode::NoteList) {
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
                }
                KeyCode::Char('a') => {
                    // Open attachment viewer modal
                    let filtered = self.filtered_notes();
                    if !filtered.is_empty() && self.selected_note < filtered.len() {
                        let note = filtered[self.selected_note];
                        if !note.attachments.is_empty() {
                            self.view_mode = ViewMode::AttachmentViewer;
                            self.selected_attachment = 0;
                        } else {
                            self.error = Some("No attachments in this note".to_string());
                        }
                    }
                }
                KeyCode::Char(c @ '1'..='9') => {
                    // In attachment viewer: select and view attachment by number (1-9)
                    if matches!(self.view_mode, ViewMode::AttachmentViewer) {
                        let attachment_index = (c as usize) - ('1' as usize);
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                            let note = filtered[self.selected_note];
                            if attachment_index < note.attachments.len() {
                                let attachment = note.attachments[attachment_index].clone();
                                if let Err(e) = self.view_attachment(&attachment) {
                                    self.error = Some(format!("Failed to view attachment: {}", e));
                                }
                            }
                        }
                    }
                }
                KeyCode::Char('A') => {
                    // Enter attachment path input mode (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        let filtered = self.filtered_notes();
                        if !filtered.is_empty() && self.selected_note < filtered.len() {
                            self.input_mode = InputMode::AttachmentPath;
                            self.attachment_path_input.clear();
                        } else {
                            self.error = Some("No note selected".to_string());
                        }
                    }
                }
                KeyCode::Char('X') => {
                    // Remove selected attachment (only in note list view)
                    if matches!(self.view_mode, ViewMode::NoteList) {
                        if let Err(e) = self.remove_attachment_from_current_note() {
                            self.error = Some(format!("Failed to remove attachment: {}", e));
                        }
                    }
                }
                _ => {
                    // Reset 'a' key flag for any other key
                    self.last_key_was_a = false;
                }
            }
        }
        Ok(())
    }

    /// Handle key events in note view state
    fn handle_note_view_key(&mut self, key: KeyEvent) -> Result<()> {
        match self.input_mode {
            InputMode::SettingsEdit | InputMode::PasswordVerify => {
                // Settings modes should not be active in note view
                // Reset to normal mode if somehow this happens
                self.input_mode = InputMode::Normal;
            }
            InputMode::Normal => match key.code {
                KeyCode::Char('e') | KeyCode::Enter => {
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
            InputMode::Insert => {
                // Insert mode not used in note view - redirect to normal mode
                self.input_mode = InputMode::Normal;
            }
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
            InputMode::AttachmentPath => {
                // AttachmentPath mode is handled in note list view, not note view
                // Reset to normal if somehow we end up here
                self.input_mode = InputMode::Normal;
            }
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
                        if self.selected_setting < 7 {
                            self.selected_setting += 1;
                        }
                    }
                    KeyCode::Up | KeyCode::Char('k') => {
                        // Move up through settings fields
                        if self.selected_setting > 0 {
                            self.selected_setting -= 1;
                        }
                    }
                    KeyCode::Enter if key.modifiers.contains(KeyModifiers::SHIFT) => {
                        // Edit selected field (backward for cyclic fields)
                        self.start_editing_setting_backward();
                    }
                    KeyCode::Enter | KeyCode::Char('i') | KeyCode::Char(' ') => {
                        // Edit selected field (forward for cyclic fields)
                        self.start_editing_setting();
                    }
                    KeyCode::Char('I') => {
                        // Edit selected field (backward for cyclic fields)
                        self.start_editing_setting_backward();
                    }
                    KeyCode::Char('p') => {
                        // Show text input for sync credentials (try clipboard first as convenience)
                        // Try clipboard paste as convenience (may fail over SSH)
                        if let Ok(mut clipboard) = arboard::Clipboard::new() {
                            if let Ok(text) = clipboard.get_text() {
                                self.credential_input = text;
                            }
                        }

                        // Show input modal for manual paste
                        let prev = std::mem::replace(&mut self.state, AppState::Quit);
                        self.state = AppState::InputSyncCredentials {
                            previous: Box::new(prev),
                        };
                        self.input_mode = InputMode::Insert;
                    }
                    KeyCode::Char('c') => {
                        // Show sync credentials as text (and try clipboard copy)
                        match self.generate_sync_credentials_text() {
                            Ok(creds_text) => {
                                // Try clipboard copy (best effort - may fail over SSH)
                                let _ = arboard::Clipboard::new()
                                    .and_then(|mut clip| clip.set_text(&creds_text));

                                // Always show text modal for manual copy
                                let prev = std::mem::replace(&mut self.state, AppState::Quit);
                                self.state = AppState::ShowSyncCredentials {
                                    credentials: creds_text,
                                    previous: Box::new(prev),
                                };
                            }
                            Err(e) => {
                                self.error = Some(format!("Failed to generate credentials: {}", e));
                            }
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
                                // Color Scheme: cycle through Light/Dark/Auto
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
            InputMode::PasswordVerify => {
                // Password verification mode for enabling remember password
                match key.code {
                    KeyCode::Esc => {
                        // Cancel password verification
                        self.setting_input.clear();
                        self.input_mode = InputMode::Normal;
                        self.sync_status = Some("Password storage not enabled.".to_string());
                        self.sync_status_set_at = Some(Instant::now());
                    }
                    KeyCode::Enter => {
                        // Verify password and save if correct
                        if self.setting_input.is_empty() {
                            self.error = Some("Password cannot be empty.".to_string());
                        } else {
                            // Verify password by attempting to derive the key
                            match self.verify_password_for_remember(&self.setting_input) {
                                Ok(true) => {
                                    // Password is correct - store it
                                    let password_to_store = self.setting_input.clone();
                                    self.setting_input.clear();
                                    self.input_mode = InputMode::Normal;

                                    if let Err(e) = self.store_password_for_autounlock(&password_to_store) {
                                        self.error = Some(format!("Failed to store password: {}", e));
                                    } else {
                                        self.sync_status = Some("Password stored for auto-unlock. WARNING: Accessible to anyone with device access!".to_string());
                                        self.sync_status_set_at = Some(Instant::now());
                                    }
                                }
                                Ok(false) => {
                                    self.error = Some("Incorrect password. Try again.".to_string());
                                    self.setting_input.clear();
                                }
                                Err(e) => {
                                    self.error = Some(format!("Password verification failed: {}", e));
                                    self.setting_input.clear();
                                    self.input_mode = InputMode::Normal;
                                }
                            }
                        }
                    }
                    KeyCode::Char(c) => {
                        // Add character to password input
                        self.setting_input.push(c);
                    }
                    KeyCode::Backspace => {
                        // Remove last character
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
                self.sync_status = Some("Password stored for auto-unlock. WARNING: Accessible to anyone with device access!".to_string());
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

            self.sync_status = Some("Password storage disabled. You will be prompted on next start.".to_string());
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

    /// Forget stored password (disable auto-unlock)
    fn forget_stored_password(&mut self) -> Result<()> {
        // Delete remember file
        let config_dir = self.db_path.parent().ok_or_else(|| anyhow::anyhow!("Invalid db path"))?;
        let remember_file = config_dir.join(".jottery_remember");
        let _ = std::fs::remove_file(&remember_file);

        // Update settings
        self.settings.remember_password = false;
        self.settings.stored_password = None;

        if let Some(db) = &self.db {
            let settings_repo = SettingsRepository::new(db.connection());
            settings_repo.update(&self.settings)?;
        }

        Ok(())
    }

    /// Verify password is correct for enabling remember password
    /// Returns Ok(true) if password is correct, Ok(false) if incorrect, Err on database error
    fn verify_password_for_remember(&self, password: &str) -> Result<bool> {
        // Try to open the database with the provided password
        // This verifies the password without actually unlocking the app
        match Database::open(&self.db_path, password) {
            Ok(_) => Ok(true),  // Password is correct
            Err(e) => {
                // Check if it's a password error or other error
                let error_msg = format!("{:?}", e);
                if error_msg.contains("wrong password") || error_msg.contains("corrupted") {
                    Ok(false)  // Password is incorrect
                } else {
                    Err(e)  // Other database error
                }
            }
        }
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
            self.sync_status_set_at = Some(Instant::now());
            return;
        }

        if self.settings.sync_endpoint.is_none() {
            self.debug_log("trigger_sync - Sync endpoint not configured, returning");
            self.sync_status = Some("Sync endpoint not configured. Configure in database settings table.".to_string());
            self.sync_status_set_at = Some(Instant::now());
            return;
        }

        // Perform sync
        self.debug_log("trigger_sync - Starting sync");
        self.sync_status = Some("Syncing...".to_string());
        self.sync_status_set_at = Some(Instant::now());

        match self.perform_sync(false) {
            Ok(result) => {
                self.sync_status = Some(format!("Sync complete! {} {} synced", result, if result == 1 { "note" } else { "notes" }));
                self.sync_status_set_at = Some(Instant::now());
            }
            Err(e) => {
                self.error = Some(format!("Sync failed: {}", e));
                self.sync_status = Some(format!("Sync failed: {}", e));
                self.sync_status_set_at = Some(Instant::now());
            }
        }
    }

    /// Force full resync from server
    fn force_full_sync(&mut self) {
        self.debug_log("force_full_sync - Called");

        // Check if sync is configured
        if !self.settings.sync_enabled {
            self.debug_log("force_full_sync - Sync not enabled, returning");
            self.sync_status = Some("Sync not enabled. Press 's' to configure in settings.".to_string());
            self.sync_status_set_at = Some(Instant::now());
            return;
        }

        if self.settings.sync_endpoint.is_none() {
            self.debug_log("force_full_sync - Sync endpoint not configured, returning");
            self.sync_status = Some("Sync endpoint not configured. Configure in database settings table.".to_string());
            self.sync_status_set_at = Some(Instant::now());
            return;
        }

        // Perform force sync
        self.debug_log("force_full_sync - Starting force full sync");
        self.sync_status = Some("Force syncing all notes...".to_string());
        self.sync_status_set_at = Some(Instant::now());

        match self.perform_sync(true) {
            Ok(result) => {
                self.sync_status = Some(format!("Force sync complete! {} {} synced from server", result, if result == 1 { "note" } else { "notes" }));
                self.sync_status_set_at = Some(Instant::now());
            }
            Err(e) => {
                self.error = Some(format!("Force sync failed: {}", e));
                self.sync_status = Some(format!("Force sync failed: {}", e));
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
                let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
                anyhow::bail!("Push failed: {} - {}", status, error_text);
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
            let error_text = response.text().unwrap_or_else(|_| "Unknown error".to_string());
            anyhow::bail!("Pull failed: {} - {}", status, error_text);
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

            for attachment_ref in &remote_note.attachments {
                // Get decrypted binary data from our map
                if let Some(decrypted_data) = attachment_data_map.get(&attachment_ref.id) {
                    // Decrypt the filename
                    let encrypted_filename: crate::crypto::EncryptedData = match serde_json::from_str(&attachment_ref.filename) {
                        Ok(data) => data,
                        Err(_) => continue,
                    };

                    let decrypted_filename = match self.crypto.decrypt_text(&encrypted_filename, key) {
                        Ok(filename) => filename,
                        Err(_) => continue,
                    };

                    // Parse the filename - try JSON first (new format), fall back to plain string (legacy format)
                    let filename: String = serde_json::from_str(&decrypted_filename)
                        .unwrap_or(decrypted_filename);

                    // Store in database
                    attachment_repo.store(
                        &attachment_ref.id,
                        &filename,
                        &attachment_ref.mime_type,
                        attachment_ref.size,
                        decrypted_data,
                        key
                    )?;

                    // Add to note's attachment array
                    note_attachments.push(Attachment {
                        id: attachment_ref.id.clone(),
                        filename,
                        mime_type: attachment_ref.mime_type.clone(),
                        size: attachment_ref.size,
                        data: attachment_ref.data.clone(),
                        thumbnail_data: None,
                    });
                }
            }

            // Check if we have this note in the database (not just in-memory list)
            let existing_note = note_repo.get(&remote_note.id, key)?;

            if let Some(mut local_note) = existing_note {
                // Note exists in database - check if we should update it
                // Conflict resolution: Last-Write-Wins
                if remote_note.modified_at > local_note.modified_at {
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

    /// Start editing a setting field (forward for cyclic fields)
    fn start_editing_setting(&mut self) {
        // Populate input buffer with current value for string/number fields
        match self.selected_setting {
            0 => {
                // Language
                self.setting_input = self.settings.language.clone();
                self.input_mode = InputMode::SettingsEdit;
            }
            1 => {
                // Theme: cycle forward immediately, no input needed
                self.cycle_theme();
            }
            2 => {
                // Sort order: cycle forward immediately, no input needed
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
            6 => {
                // Auto-sync interval
                self.setting_input = self.settings.auto_sync_interval_minutes.to_string();
                self.input_mode = InputMode::SettingsEdit;
            }
            7 => {
                // Remember password: toggle with password verification
                if self.settings.remember_password {
                    // Currently ON -> turn OFF and delete stored password
                    if let Err(e) = self.forget_stored_password() {
                        self.error = Some(format!("Failed to forget password: {}", e));
                    } else {
                        self.sync_status = Some("Password storage disabled. You will be prompted on next start.".to_string());
                        self.sync_status_set_at = Some(Instant::now());
                    }
                } else {
                    // Currently OFF -> prompt for password to enable
                    self.setting_input.clear();
                    self.input_mode = InputMode::PasswordVerify;
                    self.sync_status = Some("Enter your password to enable auto-unlock:".to_string());
                    self.sync_status_set_at = Some(Instant::now());
                }
            }
            _ => {}
        }
    }

    /// Start editing a setting field (backward for cyclic fields)
    fn start_editing_setting_backward(&mut self) {
        // For cyclic fields, cycle backward; for others, behave like forward
        match self.selected_setting {
            0 => {
                // Language
                self.setting_input = self.settings.language.clone();
                self.input_mode = InputMode::SettingsEdit;
            }
            1 => {
                // Theme: cycle backward immediately, no input needed
                self.cycle_theme_backward();
            }
            2 => {
                // Sort order: cycle backward immediately, no input needed
                self.cycle_sort_order_backward();
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
            6 => {
                // Auto-sync interval
                self.setting_input = self.settings.auto_sync_interval_minutes.to_string();
                self.input_mode = InputMode::SettingsEdit;
            }
            7 => {
                // Remember password: toggle with password verification
                if self.settings.remember_password {
                    // Currently ON -> turn OFF and delete stored password
                    if let Err(e) = self.forget_stored_password() {
                        self.error = Some(format!("Failed to forget password: {}", e));
                    } else {
                        self.sync_status = Some("Password storage disabled. You will be prompted on next start.".to_string());
                        self.sync_status_set_at = Some(Instant::now());
                    }
                } else {
                    // Currently OFF -> prompt for password to enable
                    self.setting_input.clear();
                    self.input_mode = InputMode::PasswordVerify;
                    self.sync_status = Some("Enter your password to enable auto-unlock:".to_string());
                    self.sync_status_set_at = Some(Instant::now());
                }
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
            6 => {
                // Auto-sync interval
                if let Ok(interval) = self.setting_input.parse::<i32>() {
                    if interval >= 0 && interval <= 1440 {
                        self.settings.auto_sync_interval_minutes = interval;
                    } else {
                        anyhow::bail!("Auto-sync interval must be between 0 and 1440 minutes");
                    }
                } else {
                    anyhow::bail!("Invalid number");
                }
            }
            _ => {}
        }

        self.save_settings()
    }

    /// Cycle forward through color scheme options
    fn cycle_theme(&mut self) {
        self.settings.theme.cycle_next();
        // Update cached color scheme
        self.color_scheme = crate::ui::ColorScheme::by_name(self.settings.theme.scheme_name());
        if let Err(e) = self.save_settings() {
            self.error = Some(format!("Failed to save settings: {}", e));
        }
    }

    /// Cycle backward through color scheme options
    fn cycle_theme_backward(&mut self) {
        self.settings.theme.cycle_prev();
        // Update cached color scheme
        self.color_scheme = crate::ui::ColorScheme::by_name(self.settings.theme.scheme_name());
        if let Err(e) = self.save_settings() {
            self.error = Some(format!("Failed to save settings: {}", e));
        }
    }

    /// Cycle forward through sort order options
    fn cycle_sort_order(&mut self) {
        self.settings.sort_order = self.settings.sort_order.next();
        if let Err(e) = self.save_settings() {
            self.error = Some(format!("Failed to save settings: {}", e));
        }
    }

    /// Cycle backward through sort order options
    fn cycle_sort_order_backward(&mut self) {
        self.settings.sort_order = self.settings.sort_order.prev();
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
    #[allow(dead_code)]
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

    /// Generate sync credentials text (base64 encoded)
    fn generate_sync_credentials_text(&self) -> Result<String> {
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

            // Get encryption metadata for salt
            use crate::repository::encryption::EncryptionRepository;
            let encryption_repo = EncryptionRepository::new(db.connection());
            let encryption_meta = encryption_repo.get()?
                .ok_or_else(|| anyhow::anyhow!("Encryption metadata not found"))?;

            // Convert salt to base64 string
            use base64::Engine;
            let salt_b64 = base64::engine::general_purpose::STANDARD.encode(&encryption_meta.salt);

            // Create credentials payload with salt
            let mut creds = SyncCredentials::new(
                metadata.sync_endpoint,
                api_key,
                client_id,
            );
            creds.salt = Some(salt_b64);

            // Encode to base64
            creds.to_base64()
        } else {
            anyhow::bail!("Database not available")
        }
    }

    /// Handle key events when showing sync credentials
    fn handle_show_credentials_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => {
                // Return to previous state
                if let AppState::ShowSyncCredentials { previous, .. } =
                    std::mem::replace(&mut self.state, AppState::Quit) {
                    self.state = *previous;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Handle key events when inputting sync credentials
    fn handle_input_credentials_key(&mut self, key: KeyEvent) -> Result<()> {
        match key.code {
            KeyCode::Esc => {
                // Cancel input
                self.credential_input.clear();
                self.input_mode = InputMode::Normal;
                if let AppState::InputSyncCredentials { previous } =
                    std::mem::replace(&mut self.state, AppState::Quit) {
                    self.state = *previous;
                }
            }
            KeyCode::Enter => {
                // Process the credentials
                let input = self.credential_input.trim().to_string();
                self.credential_input.clear();
                self.input_mode = InputMode::Normal;

                // Return to previous state
                if let AppState::InputSyncCredentials { previous } =
                    std::mem::replace(&mut self.state, AppState::Quit) {
                    self.state = *previous;
                }

                // Try to process credentials
                if let Err(e) = self.process_credentials_input(&input) {
                    self.error = Some(format!("Failed to paste credentials: {}", e));
                } else {
                    self.sync_status = Some("Sync credentials configured successfully!".to_string());
                }
            }
            KeyCode::Char(c) => {
                self.credential_input.push(c);
            }
            KeyCode::Backspace => {
                self.credential_input.pop();
            }
            _ => {}
        }
        Ok(())
    }

    /// Process credentials input from text
    fn process_credentials_input(&mut self, input: &str) -> Result<()> {
        // Decode credentials
        let creds = SyncCredentials::from_base64(input.trim())
            .context("Invalid sync credentials format")?;

        self.debug_log(&format!("Process credentials - endpoint: {}", creds.endpoint));
        self.debug_log(&format!("Process credentials - client_id: {}", creds.client_id));
        self.debug_log(&format!("Process credentials - has salt: {}", creds.salt.is_some()));

        // Get database
        let db = self.db.as_ref().ok_or_else(|| anyhow::anyhow!("Database not unlocked"))?;

        // If web app salt is provided, update it first
        if let Some(salt_b64) = &creds.salt {
            use base64::Engine;
            use crate::repository::encryption::EncryptionRepository;
            let encryption_repo = EncryptionRepository::new(db.connection());

            // Decode the base64 salt from web app
            let salt = base64::engine::general_purpose::STANDARD.decode(salt_b64)
                .context("Invalid base64 salt from sync credentials")?;

            self.debug_log(&format!("Process credentials - Salt (base64): {}", salt_b64));
            self.debug_log(&format!("Process credentials - Salt (hex): {}", hex::encode(&salt)));
            self.debug_log(&format!("Process credentials - Salt length: {} bytes", salt.len()));

            // Validate salt length
            if salt.len() < 32 {
                anyhow::bail!("Invalid salt length: {} bytes (expected at least 32 bytes)", salt.len());
            }

            // Update encryption metadata with web app's salt AND iteration count
            self.debug_log("Process credentials - Saving salt with 100,000 iterations");
            encryption_repo.save(&salt, 100_000)?;
            self.debug_log("Process credentials - Salt saved successfully");
        }

        // Save sync metadata with PLAINTEXT API key temporarily
        let sync_repo = SyncRepository::new(db.connection());
        let mut metadata = sync_repo.get_metadata()?.unwrap_or_default();

        // Store API key as plaintext temporarily (will be encrypted on next unlock)
        self.debug_log("Process credentials - Storing API key (will encrypt on next unlock)");
        metadata.api_key = Some(format!("PLAINTEXT:{}", creds.api_key));
        metadata.client_id = Some(creds.client_id);
        metadata.sync_endpoint = creds.endpoint.clone();
        metadata.sync_enabled = true;

        sync_repo.update_metadata(&metadata)?;

        // Update settings
        self.settings.sync_endpoint = Some(creds.endpoint);
        self.settings.sync_enabled = true;
        self.save_settings()?;

        // If web app salt was provided, lock and force re-unlock
        if creds.salt.is_some() {
            self.debug_log("Process credentials - Locking database to force re-unlock with new salt");

            // Automatically lock the database
            self.key = None;
            self.notes.clear();
            self.selected_note = 0;
            self.password_input.clear();
            self.password_confirm.clear();
            self.password_confirm_focused = false;  // Ensure single password field on unlock
            self.input_mode = InputMode::Normal;
            self.state = AppState::Locked;

            // Show message about what happened
            self.error = Some("Salt synchronized! Please re-enter your password to unlock with the new encryption salt.".to_string());
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

    /// Load deleted notes for recycle bin view
    fn load_deleted_notes(&mut self) -> Result<()> {
        if let (Some(db), Some(key)) = (&self.db, &self.key) {
            let repo = NoteRepository::new(db.connection());
            self.notes = repo.get_deleted(key)?;
        }
        Ok(())
    }

    /// Restore a deleted note
    fn restore_note(&mut self) -> Result<()> {
        if let (Some(db), Some(key)) = (&self.db, &self.key) {
            if !self.notes.is_empty() && self.selected_note < self.notes.len() {
                let note_id = self.notes[self.selected_note].id.clone();

                // Restore the note by setting deleted = false
                if let Some(note) = self.notes.iter_mut().find(|n| n.id == note_id) {
                    note.restore();

                    // Save to database
                    let repo = NoteRepository::new(db.connection());
                    repo.update(note, key)?;
                }

                // Reload deleted notes to refresh the list
                self.load_deleted_notes()?;

                // Adjust selection after restore
                if self.selected_note >= self.notes.len() && self.selected_note > 0 {
                    self.selected_note -= 1;
                }
            }
        }
        Ok(())
    }

    /// Permanently delete all notes in recycle bin
    fn empty_trash(&mut self) -> Result<()> {
        if let Some(db) = &self.db {
            let repo = NoteRepository::new(db.connection());
            let count = repo.empty_trash()?;

            // Clear the notes list
            self.notes.clear();
            self.selected_note = 0;

            // Set success message
            self.sync_status = Some(format!("Permanently deleted {} note{}", count, if count == 1 { "" } else { "s" }));
        }
        Ok(())
    }

    /// Check if chafa is available for image preview (cached result)
    fn is_chafa_available(&mut self) -> bool {
        if let Some(available) = self.chafa_available {
            return available;
        }

        // Try to run chafa --version
        let available = Command::new("chafa")
            .arg("--version")
            .output()
            .map(|output| output.status.success())
            .unwrap_or(false);

        self.chafa_available = Some(available);
        available
    }

    /// Detect MIME type from file extension
    fn detect_mime_type(path: &std::path::Path) -> String {
        if let Some(ext) = path.extension() {
            match ext.to_str().unwrap_or("").to_lowercase().as_str() {
                // Images
                "png" => "image/png",
                "jpg" | "jpeg" => "image/jpeg",
                "gif" => "image/gif",
                "webp" => "image/webp",
                "svg" => "image/svg+xml",
                "bmp" => "image/bmp",
                // Documents
                "pdf" => "application/pdf",
                "txt" => "text/plain",
                "md" => "text/markdown",
                "json" => "application/json",
                "xml" => "application/xml",
                // Archives
                "zip" => "application/zip",
                "tar" => "application/x-tar",
                "gz" => "application/gzip",
                // Default
                _ => "application/octet-stream",
            }
        } else {
            "application/octet-stream"
        }
        .to_string()
    }

    /// Expand ~ to home directory in file path
    fn expand_tilde(path: &str) -> Result<PathBuf> {
        if path.starts_with("~/") {
            let home = env::var("HOME")
                .context("HOME environment variable not set")?;
            Ok(PathBuf::from(home).join(&path[2..]))
        } else {
            Ok(PathBuf::from(path))
        }
    }

    /// View an attachment (decrypt to temp file, then view with appropriate tool)
    fn view_attachment(&mut self, attachment: &Attachment) -> Result<()> {
        let db = self.db.as_ref().context("Database not available")?;
        let key = self.key.as_ref().context("Key not available")?;

        // Retrieve and decrypt attachment data
        let attachment_repo = AttachmentRepository::new(db.connection());
        let (filename, mime_type, _size, data) = attachment_repo
            .get(&attachment.id, key)?
            .context("Attachment not found in database")?;

        // Create temporary file with original extension
        let extension = std::path::Path::new(&filename)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("");

        let temp_file = tempfile::Builder::new()
            .suffix(&format!(".{}", extension))
            .tempfile()
            .context("Failed to create temporary file")?;

        // Write decrypted data to temp file
        std::fs::write(temp_file.path(), &data)
            .context("Failed to write attachment to temporary file")?;

        // Determine viewer based on MIME type
        let is_image = mime_type.starts_with("image/");

        if is_image && self.is_chafa_available() {
            // Use chafa for terminal image display
            self.view_with_chafa(temp_file.path())?;
        } else {
            // Use system default opener
            self.view_with_system_default(temp_file.path())?;
        }

        Ok(())
    }

    /// View image with chafa in terminal
    fn view_with_chafa(&mut self, path: &std::path::Path) -> Result<()> {
        // Suspend TUI
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(io::stdout(), LeaveAlternateScreen)
            .context("Failed to leave alternate screen")?;

        // Launch chafa
        let status = Command::new("chafa")
            .arg("--size=80x40") // Reasonable terminal size
            .arg("--animate=false") // No animation for static images
            .arg(path)
            .status()
            .context("Failed to launch chafa")?;

        if status.success() {
            // Wait for user to press key to continue
            println!("\nPress Enter to continue...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
        }

        // Resume TUI (same pattern as edit_with_external_editor)
        execute!(io::stdout(), EnterAlternateScreen)
            .context("Failed to enter alternate screen")?;
        enable_raw_mode().context("Failed to enable raw mode")?;
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            Clear(ClearType::Purge),
            MoveTo(0, 0)
        )
        .context("Failed to clear screen")?;
        io::stdout().flush().context("Failed to flush stdout")?;
        self.need_redraw = true;

        if !status.success() {
            anyhow::bail!("Chafa exited with non-zero status");
        }

        Ok(())
    }

    /// View file with system default application
    fn view_with_system_default(&mut self, path: &std::path::Path) -> Result<()> {
        // Suspend TUI
        disable_raw_mode().context("Failed to disable raw mode")?;
        execute!(io::stdout(), LeaveAlternateScreen)
            .context("Failed to leave alternate screen")?;

        // Determine command based on OS
        #[cfg(target_os = "macos")]
        let open_cmd = "open";
        #[cfg(target_os = "linux")]
        let open_cmd = "xdg-open";
        #[cfg(target_os = "windows")]
        let open_cmd = "start";
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        let open_cmd = "xdg-open"; // Fallback

        // Launch viewer (detached process)
        let status = Command::new(open_cmd)
            .arg(path)
            .status()
            .context(format!("Failed to launch viewer: {}", open_cmd))?;

        if status.success() {
            // Wait for user acknowledgment (file opened in background)
            println!("File opened in default application.");
            println!("Press Enter to continue...");
            let mut input = String::new();
            std::io::stdin().read_line(&mut input)?;
        }

        // Resume TUI
        execute!(io::stdout(), EnterAlternateScreen)
            .context("Failed to enter alternate screen")?;
        enable_raw_mode().context("Failed to enable raw mode")?;
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            Clear(ClearType::Purge),
            MoveTo(0, 0)
        )
        .context("Failed to clear screen")?;
        io::stdout().flush().context("Failed to flush stdout")?;
        self.need_redraw = true;

        Ok(())
    }

    /// Delete the currently selected attachment
    fn delete_current_attachment(&mut self) -> Result<()> {
        let db = self.db.as_ref().context("Database not available")?;
        let key = self.key.as_ref().context("Key not available")?;

        // Get the note ID and attachment info first (without holding a borrow)
        let (note_id, attachment_id, filename) = {
            let filtered = self.filtered_notes();
            if filtered.is_empty() || self.selected_note >= filtered.len() {
                anyhow::bail!("No note selected");
            }

            let note = filtered[self.selected_note];
            if self.selected_attachment >= note.attachments.len() {
                anyhow::bail!("No attachment selected");
            }

            let attachment_id = note.attachments[self.selected_attachment].id.clone();
            let filename = note.attachments[self.selected_attachment].filename.clone();
            (note.id.clone(), attachment_id, filename)
        };

        // Now we can mutate self.notes
        if let Some(note_in_list) = self.notes.iter_mut().find(|n| n.id == note_id) {
            // Remove attachment from note
            note_in_list.attachments.retain(|att| att.id != attachment_id);

            // Update note in database
            note_in_list.modified_at = chrono::Utc::now();
            note_in_list.version += 1;

            let note_repo = NoteRepository::new(db.connection());
            note_repo.update(note_in_list, key)?;

            // Delete attachment data from database
            let attachment_repo = AttachmentRepository::new(db.connection());
            attachment_repo.delete(&attachment_id)?;

            // Adjust selection
            if note_in_list.attachments.is_empty() {
                // No more attachments, close viewer
                self.view_mode = ViewMode::NoteList;
            } else if self.selected_attachment >= note_in_list.attachments.len() {
                self.selected_attachment = note_in_list.attachments.len() - 1;
            }

            self.error = Some(format!("Deleted: {}", filename));
        }

        Ok(())
    }

    /// Add attachment to the current note
    fn add_attachment_to_current_note(&mut self, file_path: &str) -> Result<()> {
        // Clone debug_log so we can use it in the closure without borrowing self
        let debug_log = self.debug_log.clone();
        // Helper to write debug messages
        let log_debug = |msg: &str| {
            if let Some(log) = &debug_log {
                if let Ok(mut file) = log.lock() {
                    let _ = writeln!(file, "[{}] {}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f"), msg);
                }
            }
        };

        log_debug(&format!("add_attachment: Starting with path: {}", file_path));

        // Expand tilde in path
        let expanded_path = Self::expand_tilde(file_path)?;
        log_debug(&format!("add_attachment: Expanded path: {:?}", expanded_path));

        // Validate file exists
        if !expanded_path.exists() {
            log_debug("add_attachment: File not found");
            anyhow::bail!("File not found: {}", file_path);
        }

        if !expanded_path.is_file() {
            anyhow::bail!("Path is not a file: {}", file_path);
        }

        // Check file size (warn if > 10MB, reject if > 50MB)
        let metadata = std::fs::metadata(&expanded_path)
            .context("Failed to read file metadata")?;
        let size_bytes = metadata.len();

        if size_bytes > 50 * 1024 * 1024 {
            anyhow::bail!("File too large (>50MB): {} bytes", size_bytes);
        }

        // Read file data
        let data = std::fs::read(&expanded_path)
            .context("Failed to read file")?;

        // Detect MIME type
        let mime_type = Self::detect_mime_type(&expanded_path);

        // Get filename
        let filename = expanded_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unnamed")
            .to_string();

        // Get current note
        let filtered = self.filtered_notes();
        log_debug(&format!("add_attachment: filtered_notes count: {}", filtered.len()));
        let note_id = if !filtered.is_empty() && self.selected_note < filtered.len() {
            let id = filtered[self.selected_note].id.clone();
            log_debug(&format!("add_attachment: Selected note ID: {}", id));
            id
        } else {
            log_debug("add_attachment: No note selected");
            anyhow::bail!("No note selected");
        };

        // Store encrypted attachment
        let db = self.db.as_ref().context("Database not available")?;
        let key = self.key.as_ref().context("Key not available")?;

        // Generate UUID for attachment
        let attachment_id = uuid::Uuid::new_v4().to_string();
        log_debug(&format!("add_attachment: Generated attachment ID: {}", attachment_id));

        let attachment_repo = AttachmentRepository::new(db.connection());
        attachment_repo.store(&attachment_id, &filename, &mime_type, size_bytes as i64, &data, key)?;
        log_debug("add_attachment: Stored in attachments table");

        // Create Attachment struct
        let attachment = Attachment {
            id: attachment_id.clone(),
            filename: filename.clone(),
            mime_type: mime_type.clone(),
            size: size_bytes as i64,
            data: attachment_id, // Reference to blob store
            thumbnail_data: None,
        };

        // Load the note, add attachment, and save
        let note_repo = NoteRepository::new(db.connection());
        let mut note = note_repo
            .get(&note_id, key)?
            .context("Note not found")?;

        log_debug(&format!("add_attachment: Note has {} attachments before adding", note.attachments.len()));
        note.attachments.push(attachment);
        note.modified_at = chrono::Utc::now();
        note.version += 1;

        note_repo.update(&note, key)?;
        log_debug(&format!("add_attachment: Updated note, now has {} attachments", note.attachments.len()));

        // Refresh the note list to show updated note
        self.load_notes()?;
        log_debug("add_attachment: Reloaded notes list");

        Ok(())
    }

    /// Remove attachment from the current note
    fn remove_attachment_from_current_note(&mut self) -> Result<()> {
        // Get current note
        let filtered = self.filtered_notes();
        let note_id = if !filtered.is_empty() && self.selected_note < filtered.len() {
            filtered[self.selected_note].id.clone()
        } else {
            anyhow::bail!("No note selected");
        };

        let db = self.db.as_ref().context("Database not available")?;
        let key = self.key.as_ref().context("Key not available")?;

        // Load the note
        let note_repo = NoteRepository::new(db.connection());
        let mut note = note_repo
            .get(&note_id, key)?
            .context("Note not found")?;

        // Check if there are attachments
        if note.attachments.is_empty() {
            anyhow::bail!("No attachments to remove");
        }

        // Validate selected_attachment index
        if self.selected_attachment >= note.attachments.len() {
            anyhow::bail!("Invalid attachment selection");
        }

        // Get attachment to remove
        let attachment = note.attachments.remove(self.selected_attachment);

        // Delete from database
        let attachment_repo = AttachmentRepository::new(db.connection());
        attachment_repo.delete(&attachment.id)?;

        // Update note
        note.modified_at = chrono::Utc::now();
        note.version += 1;

        note_repo.update(&note, key)?;

        // Adjust selected_attachment if needed
        if self.selected_attachment >= note.attachments.len() && !note.attachments.is_empty() {
            self.selected_attachment = note.attachments.len() - 1;
        } else if note.attachments.is_empty() {
            self.selected_attachment = 0;
        }

        // Refresh the note list to show updated note
        self.load_notes()?;

        Ok(())
    }

    /// Edit note content with external $EDITOR
    fn edit_with_external_editor(&mut self) -> Result<String> {
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

        // Clear screen with crossterm (this clears the visible screen)
        execute!(
            io::stdout(),
            Clear(ClearType::All),
            Clear(ClearType::Purge),
            MoveTo(0, 0)
        ).context("Failed to clear screen")?;
        io::stdout().flush().context("Failed to flush stdout")?;

        // Set flag to force ratatui buffer clear on next render
        self.need_redraw = true;

        if !status.success() {
            anyhow::bail!("Editor exited with non-zero status");
        }

        // Read modified content
        let content = std::fs::read_to_string(temp_path)
            .context("Failed to read modified content")?;

        Ok(content)
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
            AppState::Locked => self.render_locked(frame),
            AppState::NoteList => self.render_note_list(frame),
            AppState::NoteView => self.render_note_view(frame),
            AppState::Settings { .. } => self.render_settings(frame),
            AppState::Help { .. } => self.render_help(frame),
            AppState::ShowSyncCredentials { credentials, .. } => {
                self.render_show_credentials(frame, credentials)
            }
            AppState::InputSyncCredentials { .. } => self.render_input_credentials(frame),
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
                Constraint::Length(2),  // Remember password checkbox
                Constraint::Length(3),  // Help text
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
            Style::default().fg(self.color_scheme.accent)
        } else if !self.is_new_database {
            Style::default().fg(self.color_scheme.accent)
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
                Style::default().fg(self.color_scheme.accent)
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
                .style(Style::default().fg(self.color_scheme.muted))
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
                    .style(Style::default().fg(self.color_scheme.error))
                    .block(Block::default().title("Error").borders(Borders::ALL));
                frame.render_widget(error, chunks[3]);
            }
        } else {
            // Show cursor at end of password input
            frame.set_cursor_position((
                chunks[0].x + self.password_input.len() as u16 + 1,
                chunks[0].y + 1,
            ));

            // Remember password checkbox
            let checkbox_text = if self.remember_password_checkbox {
                "[X] Remember password (Ctrl+R to toggle)"
            } else {
                "[ ] Remember password (Ctrl+R to toggle)"
            };
            let checkbox = Paragraph::new(checkbox_text)
                .style(if self.remember_password_checkbox {
                    Style::default().fg(self.color_scheme.accent)
                } else {
                    Style::default().fg(self.color_scheme.muted)
                })
                .alignment(Alignment::Center);
            frame.render_widget(checkbox, chunks[1]);

            // Help text with security warning
            let help_text = if self.remember_password_checkbox {
                "WARNING: Password will be accessible to anyone with physical access to this device!"
            } else {
                "Enter: unlock | Ctrl+Q: quit"
            };
            let help = Paragraph::new(help_text)
                .style(if self.remember_password_checkbox {
                    Style::default().fg(self.color_scheme.error)
                } else {
                    Style::default().fg(self.color_scheme.muted)
                })
                .alignment(Alignment::Center);
            frame.render_widget(help, chunks[2]);

            // Error (if any)
            if let Some(err) = &self.error {
                let error = Paragraph::new(err.clone())
                    .style(Style::default().fg(self.color_scheme.error))
                    .block(Block::default().title("Error").borders(Borders::ALL));
                frame.render_widget(error, chunks[3]);
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
        let title = match self.view_mode {
            ViewMode::RecycleBin => "Recycle Bin",
            ViewMode::AttachmentViewer => "Attachment Viewer",
            ViewMode::NoteList => {
                if self.search_active {
                    "Notes (Search)"
                } else {
                    "Notes"
                }
            }
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
                .style(Style::default().fg(self.color_scheme.accent))
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
                let first_line = note.content.lines().next().unwrap_or("");
                let content = strip_markdown(first_line);
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
                        .fg(self.color_scheme.accent)
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
            match self.view_mode {
                ViewMode::RecycleBin => {
                    "r: restore | E: empty bin | Esc: back to notes | ↑/↓: navigate".to_string()
                }
                ViewMode::AttachmentViewer => {
                    "↑/↓: navigate | 1-9: quick select | Enter: view | d: delete | Esc: close".to_string()
                }
                ViewMode::NoteList => {
                    "/: search | p: pin | t: tags | l: type | r: recycle bin | n: new | i: edit".to_string()
                }
            }
        };
        let help = Paragraph::new(status_text)
            .style(if let Some(ref status) = self.sync_status {
                // Show red for errors, yellow for other sync status, green for success
                if status.contains("failed") || status.contains("error") {
                    Style::default().fg(self.color_scheme.error)
                } else if status.contains("complete") {
                    Style::default().fg(self.color_scheme.success)
                } else {
                    Style::default().fg(self.color_scheme.accent)
                }
            } else {
                Style::default().fg(self.color_scheme.muted)
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

            let metadata_line = metadata_parts.join(" | ");

            // Render content based on type
            use ratatui::text::{Line, Text};
            let mut lines = vec![
                Line::styled(metadata_line, Style::default().fg(self.color_scheme.accent_secondary)),
                Line::raw(""),  // Blank line
            ];

            // For markdown, render it cleanly; for other types, use syntax highlighting
            use crate::models::SyntaxLanguage;
            if note.syntax_language == SyntaxLanguage::Markdown {
                // Render markdown with inline formatting and code block highlighting
                lines.extend(render_markdown_for_terminal(&note.content, &self.syntax_highlighter, &self.debug_log));
            } else {
                // Apply syntax highlighting to code
                let highlighted_content = self.syntax_highlighter.highlight(&note.content, note.syntax_language);
                lines.extend(highlighted_content.lines);
            }

            // Add attachments section if there are any attachments
            if !note.attachments.is_empty() {
                lines.push(Line::raw("")); // Blank line before attachments
                lines.push(Line::styled(
                    "─".repeat(40),
                    Style::default().fg(self.color_scheme.muted)
                ));
                lines.push(Line::styled(
                    "Attachments:",
                    Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)
                ));

                for (i, attachment) in note.attachments.iter().enumerate() {
                    let size_kb = (attachment.size as f64) / 1024.0;
                    let size_str = if size_kb < 1024.0 {
                        format!("{:.1} KB", size_kb)
                    } else {
                        format!("{:.1} MB", size_kb / 1024.0)
                    };

                    let prefix = format!("  a{}. ", i + 1);
                    let filename = &attachment.filename;
                    let mime = &attachment.mime_type;
                    let attachment_line = format!("{}{} ({}) [{}]", prefix, filename, size_str, mime);

                    let style = if i == self.selected_attachment {
                        Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(self.color_scheme.foreground)
                    };

                    lines.push(Line::styled(attachment_line, style));
                }

                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Press 'a' + number to view, 'A' to add, 'X' to remove selected",
                    Style::default().fg(self.color_scheme.muted)
                ));
            }

            let preview = Paragraph::new(Text::from(lines))
                .block(preview_block)
                .wrap(Wrap { trim: false })
                .scroll((self.preview_scroll_offset as u16, 0));
            frame.render_widget(preview, right_pane);
        } else {
            let preview = Paragraph::new("No notes")
                .block(preview_block)
                .alignment(Alignment::Center);
            frame.render_widget(preview, right_pane);
        }

        // Render attachment path input overlay if in AttachmentPath mode
        if matches!(self.input_mode, InputMode::AttachmentPath) {
            use ratatui::layout::Rect;

            // Calculate centered modal size
            let modal_width = 60;
            let modal_height = 3;
            let x = (size.width.saturating_sub(modal_width)) / 2;
            let y = (size.height.saturating_sub(modal_height)) / 2;

            let modal_area = Rect::new(x, y, modal_width, modal_height);

            // Render modal background
            let modal_block = Block::default()
                .title("Add Attachment (~/path or /absolute/path)")
                .borders(Borders::ALL)
                .style(Style::default().bg(self.color_scheme.background).fg(self.color_scheme.accent));

            let modal_text = format!("Enter file path: {}", self.attachment_path_input);
            let modal_paragraph = Paragraph::new(modal_text)
                .block(modal_block)
                .style(Style::default().fg(self.color_scheme.foreground));

            frame.render_widget(modal_paragraph, modal_area);
        }

        // Render force sync confirmation modal if showing
        if self.show_force_sync_confirm {
            // Create centered modal
            let modal_width = 60;
            let modal_height = 7;
            let modal_x = (size.width.saturating_sub(modal_width)) / 2;
            let modal_y = (size.height.saturating_sub(modal_height)) / 2;

            let modal_area = Rect {
                x: modal_x,
                y: modal_y,
                width: modal_width,
                height: modal_height,
            };

            // Clear background
            let clear_block = Block::default()
                .style(Style::default().bg(self.color_scheme.background));
            frame.render_widget(clear_block, modal_area);

            // Render modal
            let modal_block = Block::default()
                .title("Force Full Resync")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(self.color_scheme.accent))
                .style(Style::default().bg(self.color_scheme.background));

            let modal_text = vec![
                Line::from(""),
                Line::from("Pull ALL notes and attachments from server?").style(Style::default().fg(self.color_scheme.foreground)),
                Line::from("This will overwrite local changes if server is newer.").style(Style::default().fg(self.color_scheme.muted)),
                Line::from(""),
                Line::from(vec![
                    Span::raw("Press "),
                    Span::styled("y", Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)),
                    Span::raw(" to confirm, "),
                    Span::styled("n", Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)),
                    Span::raw(" or "),
                    Span::styled("Esc", Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)),
                    Span::raw(" to cancel"),
                ]).style(Style::default().fg(self.color_scheme.foreground)),
            ];

            let modal_paragraph = Paragraph::new(modal_text)
                .block(modal_block)
                .alignment(ratatui::layout::Alignment::Center);

            frame.render_widget(modal_paragraph, modal_area);
        }

        // Render attachment viewer modal if showing
        if matches!(self.view_mode, ViewMode::AttachmentViewer) {
            let filtered = self.filtered_notes();

            // Calculate modal size
            let modal_width = 80.min(size.width.saturating_sub(4));
            let modal_height = 10.min(size.height.saturating_sub(4));
            let modal_x = (size.width.saturating_sub(modal_width)) / 2;
            let modal_y = (size.height.saturating_sub(modal_height)) / 2;

            let modal_area = Rect {
                x: modal_x,
                y: modal_y,
                width: modal_width,
                height: modal_height,
            };

            // Clear background
            let clear_block = Block::default()
                .style(Style::default().bg(self.color_scheme.background));
            frame.render_widget(clear_block, modal_area);

            // Check if we have valid data to show
            let has_note = !filtered.is_empty() && self.selected_note < filtered.len();
            let note = if has_note { Some(filtered[self.selected_note]) } else { None };
            let has_attachments = note.map(|n| !n.attachments.is_empty()).unwrap_or(false);

            if has_attachments {
                let note = note.unwrap();

                // Render modal with attachments
                let modal_block = Block::default()
                    .title(format!(" Attachments ({}) ", note.attachments.len()))
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.color_scheme.accent))
                    .style(Style::default().bg(self.color_scheme.background));

                let mut modal_lines = vec![Line::from("")];

                // Render attachment list
                for (i, attachment) in note.attachments.iter().enumerate() {
                    let size_kb = (attachment.size as f64) / 1024.0;
                    let size_str = if size_kb < 1024.0 {
                        format!("{:.1} KB", size_kb)
                    } else {
                        format!("{:.1} MB", size_kb / 1024.0)
                    };

                    let number = if i < 9 { format!("{}", i + 1) } else { " ".to_string() };
                    let line_text = format!(
                        " {} │ {} │ {} │ {}",
                        number,
                        attachment.filename,
                        size_str,
                        attachment.mime_type
                    );

                    let style = if i == self.selected_attachment {
                        Style::default()
                            .fg(self.color_scheme.background)
                            .bg(self.color_scheme.accent)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(self.color_scheme.foreground)
                    };

                    modal_lines.push(Line::styled(line_text, style));
                }

                modal_lines.push(Line::from(""));
                modal_lines.push(Line::styled(
                    "↑/↓: navigate │ 1-9: quick select │ Enter: view │ d: delete │ Esc: close",
                    Style::default().fg(self.color_scheme.muted)
                ));

                let modal_paragraph = Paragraph::new(modal_lines)
                    .block(modal_block)
                    .alignment(ratatui::layout::Alignment::Left);

                frame.render_widget(modal_paragraph, modal_area);
            } else {
                // Show error state - no attachments or no note
                let modal_block = Block::default()
                    .title(" Attachments ")
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(self.color_scheme.error))
                    .style(Style::default().bg(self.color_scheme.background));

                let error_text = if !has_note {
                    "No note selected"
                } else {
                    "No attachments in this note"
                };

                let modal_lines = vec![
                    Line::from(""),
                    Line::styled(error_text, Style::default().fg(self.color_scheme.error)),
                    Line::from(""),
                    Line::styled("Press Esc to close", Style::default().fg(self.color_scheme.muted)),
                ];

                let modal_paragraph = Paragraph::new(modal_lines)
                    .block(modal_block)
                    .alignment(ratatui::layout::Alignment::Center);

                frame.render_widget(modal_paragraph, modal_area);
            }
        }
    }

    /// Render note view
    fn render_note_view(&self, frame: &mut Frame) {
        let size = frame.area();

        let mode_text = match self.input_mode {
            InputMode::Normal => "PREVIEW",
            InputMode::Tag => "TAG",
            InputMode::AttachmentPath => "ATTACHMENT",
            InputMode::Insert | InputMode::SettingsEdit | InputMode::PasswordVerify => "PREVIEW", // Should not happen in note view
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
            Style::default().fg(self.color_scheme.accent)
        } else {
            Style::default().fg(self.color_scheme.accent_secondary)
        };

        let tags = Paragraph::new(tags_text)
            .style(tags_style);
        frame.render_widget(tags, chunks[0]);

        // Render note content with syntax highlighting
        let highlighted_text = self.syntax_highlighter.highlight(&self.note_input, self.note_syntax);
        let text = Paragraph::new(highlighted_text)
            .block(block)
            .wrap(Wrap { trim: false });
        frame.render_widget(text, chunks[1]);

        // Help text
        let help = match self.input_mode {
            InputMode::Normal | InputMode::Insert | InputMode::SettingsEdit | InputMode::PasswordVerify => {
                Paragraph::new("Enter/e: edit with $EDITOR | t: tags | q/Esc: save & quit")
                    .style(Style::default().fg(self.color_scheme.muted))
                    .alignment(Alignment::Center)
            }
            InputMode::Tag => {
                Paragraph::new("Type tag name | Enter: add | Backspace: remove last | Esc: exit")
                    .style(Style::default().fg(self.color_scheme.muted))
                    .alignment(Alignment::Center)
            }
            InputMode::AttachmentPath => {
                Paragraph::new("Type file path | Enter: attach | Esc: cancel")
                    .style(Style::default().fg(self.color_scheme.muted))
                    .alignment(Alignment::Center)
            }
        };
        frame.render_widget(help, chunks[2]);

        // Show cursor
        match self.input_mode {
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
            InputMode::PasswordVerify => " [ENTER PASSWORD]",
            _ => "",
        };

        let block = Block::default()
            .title(format!("Settings{} - ↑/↓: navigate | Enter/i: edit | s/q: close", mode_text))
            .borders(Borders::ALL)
            .style(Style::default().fg(self.color_scheme.success));

        // Helper to create field line with selection indicator
        let field_line = |index: usize, label: String, value: String| -> Line {
            let selected = index == self.selected_setting;
            let editing = selected && matches!(self.input_mode, InputMode::SettingsEdit);
            let password_verify = selected && matches!(self.input_mode, InputMode::PasswordVerify);

            let display_value = if password_verify && index == 7 {
                // Show masked password input for remember password verification
                format!("{}_{}", "*".repeat(self.setting_input.len()), if self.setting_input.is_empty() { " (enter password)" } else { "" })
            } else if editing && (index == 0 || index == 3 || index == 5 || index == 6) {
                // Show input buffer for editable fields (language, auto-lock, sync endpoint, auto-sync interval)
                format!("{}_", self.setting_input)
            } else {
                value
            };

            let prefix = if selected { "→ " } else { "  " };
            let label_style = if selected {
                Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let value_style = if editing || password_verify {
                Style::default().fg(self.color_scheme.success).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.color_scheme.accent)
            };

            Line::from(vec![
                Span::styled(prefix, label_style.clone()),
                Span::styled(label, label_style),
                Span::styled(display_value, value_style),
            ])
        };

        let settings_text = vec![
            Line::from(vec![
                Span::styled("Application Settings", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            field_line(0, "Language:              ".to_string(), self.settings.language.clone()),
            field_line(1, "Color Scheme:          ".to_string(), format!("{} (press Enter to cycle)", self.settings.theme)),
            field_line(2, "Sort Order:            ".to_string(), format!("{} (press Enter to cycle)", self.settings.sort_order)),
            field_line(3, "Auto-lock Timeout:     ".to_string(), format!("{} minutes", self.settings.auto_lock_timeout)),
            Line::from(""),
            Line::from(vec![
                Span::styled("Sync Settings", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            field_line(4, "Sync Enabled:          ".to_string(), format!("{} (press Enter to toggle)", if self.settings.sync_enabled { "Yes" } else { "No" })),
            field_line(5, "Sync Endpoint:         ".to_string(), self.settings.sync_endpoint.clone().unwrap_or_else(|| "Not configured".to_string())),
            field_line(6, "Auto-sync Interval:    ".to_string(), format!("{} minutes (0 = disabled)", self.settings.auto_sync_interval_minutes)),
            Line::from(""),
            Line::from(vec![
                Span::styled("Security Settings", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from(""),
            field_line(7, "Remember Password:     ".to_string(), if self.settings.remember_password {
                "Yes (press Enter to disable)".to_string()
            } else {
                "No (press Enter to enable)".to_string()
            }),
            Line::from(""),
            Line::from(""),
            Line::from(vec![
                Span::styled("Instructions: ", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  • Use ↑/↓ or j/k to navigate between fields"),
            Line::from("  • Press Enter, i, or Space to edit a field"),
            Line::from("  • For text fields: type and press Enter to save, Esc to cancel"),
            Line::from("  • For toggles and cycles: press Enter to change value immediately"),
            Line::from("  • Remember Password: Enter prompts for password (if enabling) or disables (if enabled)"),
            Line::from("  • Press 'y' to trigger manual sync"),
            Line::from("  • Press 'Y' (Shift+y) to force full resync (pull all notes from server)"),
            Line::from(""),
            Line::from(vec![
                Span::styled("Sync Credentials (for multi-device setup): ", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  • Press 'p' to paste sync credentials from another device"),
            Line::from("  • Press 'c' to copy sync credentials to share with another device"),
        ];

        // Add status and error messages if present
        let mut all_lines = settings_text;
        if let Some(status) = &self.sync_status {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(vec![
                Span::styled("Status: ", Style::default().fg(self.color_scheme.success).add_modifier(Modifier::BOLD)),
                Span::styled(status.clone(), Style::default().fg(self.color_scheme.success)),
            ]));
        }
        if let Some(err) = &self.error {
            all_lines.push(Line::from(""));
            all_lines.push(Line::from(vec![
                Span::styled("Error: ", Style::default().fg(self.color_scheme.error).add_modifier(Modifier::BOLD)),
                Span::styled(err.clone(), Style::default().fg(self.color_scheme.error)),
            ]));
        }

        let paragraph = Paragraph::new(all_lines)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, size);

        // Show cursor when editing text fields
        if matches!(self.input_mode, InputMode::SettingsEdit) && (self.selected_setting == 0 || self.selected_setting == 3 || self.selected_setting == 5 || self.selected_setting == 6) {
            // Calculate cursor position based on selected field
            let line_offset = match self.selected_setting {
                0 => 2,  // Language is on line 2
                3 => 5,  // Auto-lock timeout is on line 5
                5 => 10, // Sync endpoint is on line 10
                6 => 11, // Auto-sync interval is on line 11
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
            .style(Style::default().fg(self.color_scheme.accent));

        let help_text = vec![
            Line::from(vec![
                Span::styled("UNLOCK SCREEN", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Enter password"),
            Line::from("  Enter                 Unlock database"),
            Line::from("  Tab                   Switch password/confirm (new DB)"),
            Line::from("  Backspace             Delete character"),
            Line::from("  Ctrl+q / Esc          Quit application"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE LIST", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  /                     Enter search mode"),
            Line::from("  y                     Sync notes (if configured)"),
            Line::from("  s                     Show settings"),
            Line::from("  n                     Create new note"),
            Line::from("  i / Enter             Open note in $EDITOR"),
            Line::from("  d                     Delete selected note"),
            Line::from("  j / ↓                 Move down"),
            Line::from("  k / ↑                 Move up"),
            Line::from("  ?                     Show this help"),
            Line::from("  Ctrl+q                Quit application"),
            Line::from(""),
            Line::from(vec![
                Span::styled("SEARCH MODE", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Enter search query"),
            Line::from("  #tag                  Search by tag"),
            Line::from("  -word                 Exclude word (negation)"),
            Line::from("  word1 word2           Match all words (AND)"),
            Line::from("  Enter                 Open note in $EDITOR"),
            Line::from("  Esc                   Exit search mode"),
            Line::from("  ↑ / ↓                 Navigate results"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE PREVIEW - VIEW MODE", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Enter / e             Edit with external $EDITOR"),
            Line::from("  t                     Enter tag mode"),
            Line::from("  ?                     Show this help"),
            Line::from("  q / Esc               Save and return to list"),
            Line::from(""),
            Line::from(vec![
                Span::styled("NOTE PREVIEW - TAG MODE", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  Type                  Enter tag name"),
            Line::from("  Enter                 Add tag"),
            Line::from("  Backspace (empty)     Remove last tag"),
            Line::from("  Backspace             Delete character from input"),
            Line::from("  Esc                   Exit to normal mode"),
            Line::from(""),
            Line::from(vec![
                Span::styled("SETTINGS", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  j/k or ↑/↓            Navigate between fields"),
            Line::from("  Enter / i / Space     Edit selected field"),
            Line::from("  Enter                 Save text/number fields, cycle/toggle other fields"),
            Line::from("  Esc                   Cancel editing (text/number fields)"),
            Line::from("  p                     Paste sync credentials (shows text input)"),
            Line::from("  c                     Copy sync credentials (shows text to copy)"),
            Line::from("  s / q                 Close settings panel"),
            Line::from(""),
            Line::from(vec![
                Span::styled("ATTACHMENTS", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  A                     Add attachment (enter file path)"),
            Line::from("  a + number            View attachment (a1, a2, ...)"),
            Line::from("  Shift+j / Shift+k     Navigate attachments"),
            Line::from("  X                     Remove selected attachment"),
            Line::from("  Note: Images use chafa if available, otherwise system default app"),
            Line::from(""),
            Line::from(vec![
                Span::styled("GLOBAL", Style::default().fg(self.color_scheme.title).add_modifier(Modifier::BOLD)),
            ]),
            Line::from("  ?                     Show this help screen"),
        ];

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .wrap(Wrap { trim: false });

        frame.render_widget(paragraph, size);
    }

    /// Render sync credentials display modal
    fn render_show_credentials(&self, frame: &mut Frame, credentials: &str) {
        let size = frame.area();

        // Create centered modal (60% width, 60% height)
        let modal_width = (size.width as f32 * 0.6) as u16;
        let modal_height = (size.height as f32 * 0.6) as u16;
        let modal_x = (size.width - modal_width) / 2;
        let modal_y = (size.height - modal_height) / 2;

        let modal_area = ratatui::layout::Rect {
            x: modal_x,
            y: modal_y,
            width: modal_width,
            height: modal_height,
        };

        // Create background
        frame.render_widget(
            Block::default().style(Style::default().bg(self.color_scheme.background)),
            modal_area,
        );

        // Create border block
        let block = Block::default()
            .title(" Sync Credentials (Copy This Text) ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.color_scheme.title));

        // Split into content area and help area
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(3)])
            .split(block.inner(modal_area));

        // Render border
        frame.render_widget(block, modal_area);

        // Render credentials text (wrapped)
        let text = vec![
            Line::from(""),
            Line::from(Span::styled(
                "Copy the text below and paste it into another Jottery client:",
                Style::default().fg(self.color_scheme.accent),
            )),
            Line::from(""),
            Line::from(Span::raw(credentials)),
        ];

        let paragraph = Paragraph::new(text)
            .wrap(Wrap { trim: false })
            .style(Style::default().fg(self.color_scheme.foreground));

        frame.render_widget(paragraph, chunks[0]);

        // Render help text
        let help = Paragraph::new(Line::from(vec![
            Span::styled("Press ", Style::default().fg(self.color_scheme.muted)),
            Span::styled("Esc", Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(" or ", Style::default().fg(self.color_scheme.muted)),
            Span::styled("Enter", Style::default().fg(self.color_scheme.accent).add_modifier(Modifier::BOLD)),
            Span::styled(" to close", Style::default().fg(self.color_scheme.muted)),
        ]))
        .alignment(Alignment::Center);

        frame.render_widget(help, chunks[1]);
    }

    /// Render sync credentials input modal
    fn render_input_credentials(&self, frame: &mut Frame) {
        let size = frame.area();

        // Create centered modal (60% width, 40% height)
        let modal_width = (size.width as f32 * 0.6) as u16;
        let modal_height = (size.height as f32 * 0.4) as u16;
        let modal_x = (size.width - modal_width) / 2;
        let modal_y = (size.height - modal_height) / 2;

        let modal_area = ratatui::layout::Rect {
            x: modal_x,
            y: modal_y,
            width: modal_width,
            height: modal_height,
        };

        // Create background
        frame.render_widget(
            Block::default().style(Style::default().bg(self.color_scheme.background)),
            modal_area,
        );

        // Create border block
        let block = Block::default()
            .title(" Paste Sync Credentials ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(self.color_scheme.title));

        // Split into content area and help area
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(2),
                Constraint::Min(1),
                Constraint::Length(3),
            ])
            .split(block.inner(modal_area));

        // Render border
        frame.render_widget(block, modal_area);

        // Render instruction text
        let instruction = Paragraph::new(Line::from(Span::styled(
            "Paste the base64 credentials text from another Jottery client:",
            Style::default().fg(self.color_scheme.accent),
        )));
        frame.render_widget(instruction, chunks[0]);

        // Render input field
        let input = Paragraph::new(Line::from(vec![
            Span::raw(&self.credential_input),
            Span::styled("█", Style::default().fg(self.color_scheme.title)), // Cursor
        ]))
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(self.color_scheme.foreground));

        frame.render_widget(input, chunks[1]);

        // Render help text
        let help = Paragraph::new(Line::from(vec![
            Span::styled("Press ", Style::default().fg(self.color_scheme.muted)),
            Span::styled("Enter", Style::default().fg(self.color_scheme.success).add_modifier(Modifier::BOLD)),
            Span::styled(" to paste | ", Style::default().fg(self.color_scheme.muted)),
            Span::styled("Esc", Style::default().fg(self.color_scheme.error).add_modifier(Modifier::BOLD)),
            Span::styled(" to cancel", Style::default().fg(self.color_scheme.muted)),
        ]))
        .alignment(Alignment::Center);

        frame.render_widget(help, chunks[2]);
    }

    /// Check if app should quit
    pub fn should_quit(&self) -> bool {
        matches!(self.state, AppState::Quit)
    }
}
