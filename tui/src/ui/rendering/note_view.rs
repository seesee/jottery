//! Rendering for the note view (single note editing/preview)

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};

use crate::ui::app::App;
use crate::ui::state::InputMode;

/// Render note view
pub fn render_note_view(app: &App, frame: &mut Frame) {
    let size = frame.area();

    let mode_text = match app.input_mode {
        InputMode::Normal => "PREVIEW",
        InputMode::Tag => "TAG",
        InputMode::AttachmentPath => "ATTACHMENT",
        InputMode::Insert | InputMode::SettingsEdit | InputMode::PasswordVerify
            | InputMode::BulkAddTags | InputMode::BulkExportPath => "PREVIEW", // Should not happen in note view
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
    let tags_text = if app.current_tags.is_empty() {
        if matches!(app.input_mode, InputMode::Tag) {
            format!("Tags: {}_", app.tag_input)
        } else {
            "Tags: (none - press 't' to add)".to_string()
        }
    } else if matches!(app.input_mode, InputMode::Tag) {
        format!("Tags: {} {}_",
            app.current_tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" "),
            app.tag_input
        )
    } else {
        format!("Tags: {}",
            app.current_tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")
        )
    };

    let tags_style = if matches!(app.input_mode, InputMode::Tag) {
        Style::default().fg(app.color_scheme.accent)
    } else {
        Style::default().fg(app.color_scheme.accent_secondary)
    };

    let tags = Paragraph::new(tags_text)
        .style(tags_style);
    frame.render_widget(tags, chunks[0]);

    // Render note content with syntax highlighting
    let highlighted_text = app.syntax_highlighter.highlight(&app.note_input, app.note_syntax);
    let text = Paragraph::new(highlighted_text)
        .block(block)
        .wrap(Wrap { trim: false });
    frame.render_widget(text, chunks[1]);

    // Help text
    let help = match app.input_mode {
        InputMode::Normal | InputMode::Insert | InputMode::SettingsEdit | InputMode::PasswordVerify
            | InputMode::BulkAddTags | InputMode::BulkExportPath => {
            Paragraph::new("Enter/e: edit with $EDITOR | t: tags | q/Esc: save & quit")
                .style(Style::default().fg(app.color_scheme.muted))
                .alignment(Alignment::Center)
        }
        InputMode::Tag => {
            Paragraph::new("Type tag name | Enter: add | Backspace: remove last | Esc: exit")
                .style(Style::default().fg(app.color_scheme.muted))
                .alignment(Alignment::Center)
        }
        InputMode::AttachmentPath => {
            Paragraph::new("Type file path | Enter: attach | Esc: cancel")
                .style(Style::default().fg(app.color_scheme.muted))
                .alignment(Alignment::Center)
        }
    };
    frame.render_widget(help, chunks[2]);

    // Show cursor
    if let InputMode::Tag = app.input_mode {
        // Cursor after tag input
        let tag_prefix_len = if app.current_tags.is_empty() {
            "Tags: ".len()
        } else {
            format!("Tags: {} ",
                app.current_tags.iter().map(|t| format!("#{}", t)).collect::<Vec<_>>().join(" ")
            ).len()
        };

        frame.set_cursor_position((
            chunks[0].x + tag_prefix_len as u16 + app.tag_input.len() as u16,
            chunks[0].y,
        ));
    }
}
