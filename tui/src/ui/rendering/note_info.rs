//! Note info modal rendering
//!
//! Renders a modal showing detailed metadata about the currently selected note.

use ratatui::{
    layout::{Alignment, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use rust_i18n::t;

use crate::models::Note;
use crate::ui::app::App;
use crate::ui::helpers::format_datetime_metadata;
use crate::ui::rendering::modal::centered_rect;

/// Render the note info modal
///
/// Shows detailed metadata about the currently selected note including
/// timestamps, word count, tags, attachments, and status flags.
pub fn render_note_info(app: &App, frame: &mut Frame, size: Rect) {
    if !app.show_note_info {
        return;
    }

    let filtered = app.filtered_notes();
    if filtered.is_empty() || app.selected_note >= filtered.len() {
        return;
    }

    let note = filtered[app.selected_note];

    // Calculate modal size
    let modal_width = 70.min(size.width.saturating_sub(4));
    let modal_height = 20.min(size.height.saturating_sub(4));
    let modal_area = centered_rect(modal_width, modal_height, size);

    // Clear the background area
    frame.render_widget(Clear, modal_area);

    let modal_block = Block::default()
        .title(format!(" {} ", t!("note.info_title")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.accent))
        .style(Style::default().bg(app.color_scheme.background));

    let info_lines = build_info_lines(note, app);

    let modal_paragraph = Paragraph::new(info_lines)
        .block(modal_block)
        .alignment(Alignment::Left);

    frame.render_widget(modal_paragraph, modal_area);
}

/// Build the info lines for the note metadata display
fn build_info_lines<'a>(note: &Note, app: &App) -> Vec<Line<'a>> {
    let mut info_lines = vec![Line::from("")];

    // Note ID (truncated)
    let id_display = if note.id.len() > 36 {
        note.id[..36].to_string()
    } else {
        note.id.clone()
    };
    info_lines.push(Line::from(vec![
        Span::styled("ID:          ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(id_display, Style::default().fg(app.color_scheme.foreground)),
    ]));

    // Created date (locale-aware)
    info_lines.push(Line::from(vec![
        Span::styled("Created:     ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(
            format_datetime_metadata(&note.created_at),
            Style::default().fg(app.color_scheme.foreground)
        ),
    ]));

    // Modified date (locale-aware)
    info_lines.push(Line::from(vec![
        Span::styled("Modified:    ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(
            format_datetime_metadata(&note.modified_at),
            Style::default().fg(app.color_scheme.foreground)
        ),
    ]));

    // Synced date (locale-aware)
    let synced_str = match &note.synced_at {
        Some(dt) => format_datetime_metadata(dt),
        None => "Never".to_string(),
    };
    info_lines.push(Line::from(vec![
        Span::styled("Synced:      ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(synced_str, Style::default().fg(app.color_scheme.foreground)),
    ]));

    info_lines.push(Line::from(""));

    // Word count / character count
    let word_count = note.content.split_whitespace().count();
    let char_count = note.content.chars().count();
    info_lines.push(Line::from(vec![
        Span::styled("Words:       ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(format!("{}", word_count), Style::default().fg(app.color_scheme.foreground)),
        Span::styled("  Characters: ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(format!("{}", char_count), Style::default().fg(app.color_scheme.foreground)),
    ]));

    // Syntax language
    info_lines.push(Line::from(vec![
        Span::styled("Syntax:      ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(format!("{}", note.syntax_language), Style::default().fg(app.color_scheme.accent_secondary)),
    ]));

    // Colour
    let color_str = note.color.as_deref().unwrap_or("None").to_string();
    info_lines.push(Line::from(vec![
        Span::styled("Colour:      ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(color_str, Style::default().fg(app.color_scheme.foreground)),
    ]));

    info_lines.push(Line::from(""));

    // Tags count
    info_lines.push(Line::from(vec![
        Span::styled("Tags:        ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(format!("{}", note.tags.len()), Style::default().fg(app.color_scheme.foreground)),
    ]));

    // Attachments count
    info_lines.push(Line::from(vec![
        Span::styled("Attachments: ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(format!("{}", note.attachments.len()), Style::default().fg(app.color_scheme.foreground)),
    ]));

    // Status flags
    let mut status_parts = Vec::new();
    if note.pinned { status_parts.push("📌 Pinned"); }
    if note.archived { status_parts.push("📦 Archived"); }
    if note.locked { status_parts.push("🔒 Locked"); }
    let status_str = if status_parts.is_empty() {
        "None".to_string()
    } else {
        status_parts.join("  ")
    };
    info_lines.push(Line::from(vec![
        Span::styled("Status:      ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(status_str, Style::default().fg(app.color_scheme.foreground)),
    ]));

    // Locked date (if locked, locale-aware)
    if note.locked {
        if let Some(locked_at) = &note.locked_at {
            info_lines.push(Line::from(vec![
                Span::styled("Locked at:   ", Style::default().fg(app.color_scheme.muted)),
                Span::styled(
                    format_datetime_metadata(locked_at),
                    Style::default().fg(app.color_scheme.foreground)
                ),
            ]));
        }
    }

    // Version
    info_lines.push(Line::from(vec![
        Span::styled("Version:     ", Style::default().fg(app.color_scheme.muted)),
        Span::styled(format!("{}", note.version), Style::default().fg(app.color_scheme.foreground)),
    ]));

    info_lines.push(Line::from(""));
    info_lines.push(Line::styled(
        "Press Esc or I to close",
        Style::default().fg(app.color_scheme.muted)
    ));

    info_lines
}
