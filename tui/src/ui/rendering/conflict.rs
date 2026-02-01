//! Rendering for the conflict resolution modal
//!
//! Shows a side-by-side comparison of local vs server versions with resolution options.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::ColorScheme;

/// Render conflict resolution modal
pub fn render_conflict_modal(app: &App, frame: &mut Frame, size: Rect) {
    // Calculate modal size (large for side-by-side comparison)
    let modal_width = 120.min(size.width.saturating_sub(4));
    let modal_height = 35.min(size.height.saturating_sub(4));
    let modal_x = (size.width.saturating_sub(modal_width)) / 2;
    let modal_y = (size.height.saturating_sub(modal_height)) / 2;

    let modal_area = Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    // Clear the background area
    frame.render_widget(Clear, modal_area);

    // Check if we have conflict data
    if app.conflict_note_id.is_none() || app.conflict_data.is_none() {
        render_no_conflict_state(frame, modal_area, &app.color_scheme);
        return;
    }

    let conflict_data = app.conflict_data.as_ref().unwrap();

    // Create main modal block
    let modal_block = Block::default()
        .title(format!(" {} ", t!("conflict.title")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.accent))
        .style(Style::default().bg(app.color_scheme.background));

    let inner_area = modal_block.inner(modal_area);
    frame.render_widget(modal_block, modal_area);

    // Determine if we have ancestor data for 3-way merge
    let has_ancestor = app.conflict_ancestor_content.is_some();

    // Split into: header, ancestor (optional), content panes, and footer (help/actions)
    let main_layout = if has_ancestor {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header with note info and timestamps
                Constraint::Length(6),  // Ancestor section (3-way merge)
                Constraint::Min(0),     // Content panes
                Constraint::Length(2),  // Footer with actions
            ])
            .split(inner_area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(3),  // Header with note info and timestamps
                Constraint::Min(0),     // Content panes
                Constraint::Length(2),  // Footer with actions
            ])
            .split(inner_area)
    };

    let header_area = main_layout[0];
    let (ancestor_area, content_area, footer_area) = if has_ancestor {
        (Some(main_layout[1]), main_layout[2], main_layout[3])
    } else {
        (None, main_layout[1], main_layout[2])
    };

    // Render header
    render_conflict_header(
        frame,
        header_area,
        &app.conflict_local_content,
        conflict_data,
        &app.color_scheme,
    );

    // Render ancestor section if available (3-way merge)
    if let (Some(ancestor_area), Some(ancestor_content)) = (ancestor_area, &app.conflict_ancestor_content) {
        render_ancestor_section(
            frame,
            ancestor_area,
            ancestor_content,
            app.conflict_ancestor_tags.as_deref(),
            &app.color_scheme,
        );
    }

    // Split content into two panes (local | server)
    let panes = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(content_area);

    // Render local (left) pane
    render_content_pane(
        frame,
        panes[0],
        &t!("conflict.your_version"),
        &app.conflict_local_content,
        &app.conflict_local_tags,
        app.conflict_local_scroll,
        !app.conflict_focus_server,
        &app.color_scheme,
    );

    // Render server (right) pane
    render_content_pane(
        frame,
        panes[1],
        &t!("conflict.server_version"),
        &app.conflict_server_content,
        &app.conflict_server_tags,
        app.conflict_server_scroll,
        app.conflict_focus_server,
        &app.color_scheme,
    );

    // Render footer with actions
    render_conflict_footer(frame, footer_area, &app.color_scheme);
}

/// Render the ancestor section for 3-way merge
fn render_ancestor_section(
    frame: &mut Frame,
    area: Rect,
    content: &str,
    tags: Option<&[String]>,
    color_scheme: &ColorScheme,
) {
    let ancestor_block = Block::default()
        .title(format!(" {} ", t!("conflict.common_ancestor")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color_scheme.muted))
        .style(Style::default().bg(color_scheme.background));

    let inner = ancestor_block.inner(area);
    frame.render_widget(ancestor_block, area);

    let mut lines = vec![];

    // Add description
    lines.push(Line::styled(
        t!("conflict.ancestor_description").to_string(),
        Style::default().fg(color_scheme.muted),
    ));

    // Show tags if present
    if let Some(tags) = tags {
        if !tags.is_empty() {
            let tags_str = tags.iter()
                .map(|t| format!("#{}", t))
                .collect::<Vec<_>>()
                .join(" ");
            lines.push(Line::styled(
                format!("Tags: {}", tags_str),
                Style::default().fg(color_scheme.muted),
            ));
        }
    }

    // Show truncated content preview
    let preview: String = content.lines()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ");
    let truncated = if preview.chars().count() > 80 {
        format!("{}...", preview.chars().take(80).collect::<String>())
    } else {
        preview
    };
    lines.push(Line::styled(truncated, Style::default().fg(color_scheme.muted)));

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, inner);
}

/// Render the header showing note info and timestamps
fn render_conflict_header(
    frame: &mut Frame,
    area: Rect,
    local_content: &str,
    conflict_data: &crate::models::sync::ConflictData,
    color_scheme: &ColorScheme,
) {
    // Extract first line as title
    let title = local_content.lines().next().unwrap_or("Untitled");
    // Use character-aware truncation to avoid panicking on multi-byte UTF-8
    let title_truncated = if title.chars().count() > 50 {
        let truncated: String = title.chars().take(50).collect();
        format!("{}...", truncated)
    } else {
        title.to_string()
    };

    let local_date = conflict_data.detected_at.format("%Y-%m-%d %H:%M").to_string();
    let server_date = conflict_data.server_modified_at.format("%Y-%m-%d %H:%M").to_string();

    let lines = vec![
        Line::from(vec![
            Span::styled("Note: ", Style::default().fg(color_scheme.muted)),
            Span::styled(title_truncated, Style::default().fg(color_scheme.foreground).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(vec![
            Span::styled("Local: ", Style::default().fg(color_scheme.accent)),
            Span::styled(&local_date, Style::default().fg(color_scheme.foreground)),
            Span::raw("  │  "),
            Span::styled("Server: ", Style::default().fg(color_scheme.accent_secondary)),
            Span::styled(&server_date, Style::default().fg(color_scheme.foreground)),
        ]),
    ];

    let header = Paragraph::new(lines);
    frame.render_widget(header, area);
}

/// Render a content pane (local or server version)
fn render_content_pane(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    content: &str,
    tags: &[String],
    scroll_offset: usize,
    is_focused: bool,
    color_scheme: &ColorScheme,
) {
    let border_style = if is_focused {
        Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(color_scheme.muted)
    };

    let pane_block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(border_style)
        .style(Style::default().bg(color_scheme.background));

    let inner = pane_block.inner(area);
    frame.render_widget(pane_block, area);

    // Build content lines
    let mut lines = vec![];

    // Show tags
    if !tags.is_empty() {
        let tags_str = tags.iter()
            .map(|t| format!("#{}", t))
            .collect::<Vec<_>>()
            .join(" ");
        lines.push(Line::styled(
            format!("Tags: {}", tags_str),
            Style::default().fg(color_scheme.accent_secondary),
        ));
        lines.push(Line::raw(""));
    }

    // Content lines
    for line in content.lines() {
        lines.push(Line::raw(line));
    }

    let content_paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll_offset as u16, 0));

    frame.render_widget(content_paragraph, inner);
}

/// Render the footer with action keys
fn render_conflict_footer(frame: &mut Frame, area: Rect, color_scheme: &ColorScheme) {
    let help_spans = vec![
        Span::styled("1", Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)),
        Span::raw(": Keep Mine  "),
        Span::styled("2", Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)),
        Span::raw(": Keep Server  "),
        Span::styled("3", Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)),
        Span::raw(": Keep Both  "),
        Span::styled("Tab", Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)),
        Span::raw(": Switch pane  "),
        Span::styled("j/k", Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)),
        Span::raw(": Scroll  "),
        Span::styled("Esc", Style::default().fg(color_scheme.accent).add_modifier(Modifier::BOLD)),
        Span::raw(": Cancel"),
    ];

    let help = Paragraph::new(Line::from(help_spans))
        .alignment(Alignment::Center)
        .style(Style::default().fg(color_scheme.muted));

    frame.render_widget(help, area);
}

/// Render state when no conflict data is available
fn render_no_conflict_state(frame: &mut Frame, area: Rect, color_scheme: &ColorScheme) {
    let modal_block = Block::default()
        .title(" Conflict Resolution ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(color_scheme.error))
        .style(Style::default().bg(color_scheme.background));

    let lines = vec![
        Line::from(""),
        Line::styled("No conflict data available", Style::default().fg(color_scheme.error)),
        Line::from(""),
        Line::styled("Press Esc to close", Style::default().fg(color_scheme.muted)),
    ];

    let paragraph = Paragraph::new(lines)
        .block(modal_block)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}
