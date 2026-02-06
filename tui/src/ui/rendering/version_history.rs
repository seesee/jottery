//! Version history modal rendering
//!
//! Renders a two-pane modal showing version history for a note,
//! with a version list on the left and content preview on the right.

use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    text::Line,
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::ui::app::App;
use crate::ui::rendering::modal::centered_rect;
use crate::ui::state::ViewMode;

/// Render the version history modal
///
/// Shows a two-pane layout with version list on the left and
/// content preview on the right. Supports scrolling through versions
/// and viewing their full content.
pub fn render_version_history(app: &App, frame: &mut Frame, size: Rect) {
    if !matches!(app.view_mode, ViewMode::VersionHistory) {
        return;
    }

    // Calculate modal size (larger than attachment viewer for content preview)
    let modal_width = 100.min(size.width.saturating_sub(4));
    let modal_height = 30.min(size.height.saturating_sub(4));
    let modal_area = centered_rect(modal_width, modal_height, size);

    // Clear the background area
    frame.render_widget(Clear, modal_area);

    if !app.loaded_versions.is_empty() {
        render_version_list_with_preview(app, frame, modal_area);
    } else {
        render_empty_state(app, frame, modal_area);
    }
}

/// Render the two-pane layout with version list and preview
fn render_version_list_with_preview(app: &App, frame: &mut Frame, modal_area: Rect) {
    // Create two-pane layout (version list | preview)
    let modal_block = Block::default()
        .title(format!(" Version History ({}) ", app.loaded_versions.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.accent))
        .style(Style::default().bg(app.color_scheme.background));

    // Split into left pane (version list) and right pane (preview)
    let inner_area = modal_block.inner(modal_area);
    frame.render_widget(modal_block, modal_area);

    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(30),  // Version list
            Constraint::Min(0),      // Preview pane
        ])
        .split(inner_area);

    // Left pane: Version list
    render_version_list(app, frame, panes[0]);

    // Right pane: Preview of selected version
    if app.selected_version < app.loaded_versions.len() {
        render_version_preview(app, frame, panes[1]);
    }
}

/// Render the version list in the left pane
fn render_version_list(app: &App, frame: &mut Frame, area: Rect) {
    let mut version_lines = vec![];
    for (i, version) in app.loaded_versions.iter().enumerate() {
        let created_str = version.created_at.format("%Y-%m-%d %H:%M").to_string();
        let reason_str = match version.reason {
            crate::repository::VersionReason::Sync => "auto",
            crate::repository::VersionReason::ManualSync => "manual",
        };

        let line_text = format!(" v{:<4} │ {} │ {}", version.version, created_str, reason_str);

        let style = if i == app.selected_version {
            Style::default()
                .fg(app.color_scheme.background)
                .bg(app.color_scheme.accent)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.color_scheme.foreground)
        };

        version_lines.push(Line::styled(line_text, style));
    }

    let version_list_block = Block::default()
        .title(" Versions ")
        .borders(Borders::RIGHT);

    let version_list = Paragraph::new(version_lines)
        .block(version_list_block);

    frame.render_widget(version_list, area);
}

/// Render the version preview in the right pane
fn render_version_preview(app: &App, frame: &mut Frame, area: Rect) {
    let version = &app.loaded_versions[app.selected_version];

    // Split right pane into content area and help text area
    let preview_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),      // Content area (scrollable)
            Constraint::Length(1),   // Help text (fixed)
        ])
        .split(area);

    // Content area with scrollable preview
    let preview_block = Block::default()
        .title(" Preview ")
        .borders(Borders::NONE);

    let mut preview_lines = vec![];

    // Metadata section
    preview_lines.push(Line::styled(
        format!("Version: {}", version.version),
        Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
    ));
    preview_lines.push(Line::styled(
        format!("Created: {}", version.created_at.format("%Y-%m-%d %H:%M:%S")),
        Style::default().fg(app.color_scheme.foreground)
    ));
    preview_lines.push(Line::styled(
        format!("Synced:  {}", version.synced_at.format("%Y-%m-%d %H:%M:%S")),
        Style::default().fg(app.color_scheme.foreground)
    ));
    preview_lines.push(Line::styled(
        format!("Characters: {}", version.content.len()),
        Style::default().fg(app.color_scheme.foreground)
    ));

    // Tags section
    if !version.tags.is_empty() {
        let tags_str = version.tags.iter()
            .map(|t| format!("#{}", t))
            .collect::<Vec<_>>()
            .join(" ");
        preview_lines.push(Line::styled(
            format!("Tags: {}", tags_str),
            Style::default().fg(app.color_scheme.accent_secondary)
        ));
    }

    preview_lines.push(Line::from(""));
    preview_lines.push(Line::styled(
        "─".repeat(60),
        Style::default().fg(app.color_scheme.muted)
    ));
    preview_lines.push(Line::from(""));

    // Full content (no truncation)
    for line in version.content.lines() {
        preview_lines.push(Line::from(line));
    }

    let preview = Paragraph::new(preview_lines)
        .block(preview_block)
        .wrap(Wrap { trim: false })
        .scroll((app.version_preview_scroll_offset as u16, 0));

    frame.render_widget(preview, preview_chunks[0]);

    // Fixed help text at bottom
    let help_text = Line::styled(
        "↑/↓: versions │ Shift+J/K: scroll │ Enter: restore │ Esc: close",
        Style::default().fg(app.color_scheme.muted)
    );
    let help_paragraph = Paragraph::new(help_text)
        .alignment(Alignment::Center);

    frame.render_widget(help_paragraph, preview_chunks[1]);
}

/// Render empty state when no versions are available
fn render_empty_state(app: &App, frame: &mut Frame, modal_area: Rect) {
    let modal_block = Block::default()
        .title(" Version History ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.error))
        .style(Style::default().bg(app.color_scheme.background));

    let modal_lines = vec![
        Line::from(""),
        Line::styled("No version history", Style::default().fg(app.color_scheme.error)),
        Line::from(""),
        Line::styled("Press Esc to close", Style::default().fg(app.color_scheme.muted)),
    ];

    let modal_paragraph = Paragraph::new(modal_lines)
        .block(modal_block)
        .alignment(Alignment::Center);

    frame.render_widget(modal_paragraph, modal_area);
}
