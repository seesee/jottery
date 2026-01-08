//! Rendering for the note list view (split pane with preview)

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};

use crate::ui::app::App;
use crate::ui::state::{FocusedPanel, InputMode, ViewMode};
use crate::ui::helpers::{strip_markdown, render_markdown_for_terminal};
use crate::models::SyntaxLanguage;

/// Render note list (split pane view)
pub fn render_note_list(app: &App, frame: &mut Frame) {
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
    let filtered = app.filtered_notes();
    let title = match app.view_mode {
        ViewMode::RecycleBin => "Recycle Bin".to_string(),
        ViewMode::AttachmentViewer => "Attachment Viewer".to_string(),
        ViewMode::VersionHistory => "Version History".to_string(),
        ViewMode::NoteList => {
            if app.search_active && !app.search_input.is_empty() {
                format!("Jottery v{} - Notes ({}/{})", env!("CARGO_PKG_VERSION"), filtered.len(), app.notes.len())
            } else {
                format!("Jottery v{} - Notes", env!("CARGO_PKG_VERSION"))
            }
        }
    };

    let left_constraints = if app.search_active {
        vec![Constraint::Length(3), Constraint::Min(0)]
    } else {
        vec![Constraint::Min(0)]
    };

    let left_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(left_constraints)
        .split(left_pane);

    // Render search bar if active
    let list_chunk = if app.search_active {
        let search_text = format!("Search: {}", app.search_input);
        let search_bar = Paragraph::new(search_text)
            .style(Style::default().fg(app.color_scheme.accent))
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

    let items: Vec<ListItem> = filtered
        .iter()
        .map(|note| {
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

            ListItem::new(preview)
        })
        .collect();

    let list = List::new(items)
        .block(list_block)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(app.color_scheme.accent)
        );

    // Create list state for scrolling
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(Some(app.selected_note));

    frame.render_stateful_widget(list, list_chunk, &mut list_state);

    // Help text (full width at bottom)
    let status_text = if let Some(ref status) = app.sync_status {
        status.clone()
    } else if app.search_active {
        "Type: search | Esc: exit | ↑/↓: navigate".to_string()
    } else {
        match app.view_mode {
            ViewMode::RecycleBin => {
                "r: restore | E: empty bin | Esc: back to notes | ↑/↓: navigate".to_string()
            }
            ViewMode::AttachmentViewer => {
                "↑/↓: navigate | 1-9: quick select | Enter: view | d: delete | Esc: close".to_string()
            }
            ViewMode::VersionHistory => {
                "↑/↓: navigate | Enter: restore version | Esc: close".to_string()
            }
            ViewMode::NoteList => {
                "/: search | p: pin | t: tags | l: type | r: recycle bin | v: versions | n: new | i: edit".to_string()
            }
        }
    };
    let help = Paragraph::new(status_text)
        .style(if let Some(ref status) = app.sync_status {
            // Show red for errors, yellow for other sync status, green for success
            if status.contains("failed") || status.contains("error") {
                Style::default().fg(app.color_scheme.error)
            } else if status.contains("complete") {
                Style::default().fg(app.color_scheme.success)
            } else {
                Style::default().fg(app.color_scheme.accent)
            }
        } else {
            Style::default().fg(app.color_scheme.muted)
        })
        .alignment(Alignment::Center);
    frame.render_widget(help, help_area);

    // Right pane: note preview with timestamp footer
    // Split into main preview area and timestamp footer (2 lines: timestamp + border)
    let preview_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(right_pane);

    let preview_area = preview_layout[0];
    let timestamp_area = preview_layout[1];

    let preview_block = Block::default()
        .title("Preview")
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT);

    if !filtered.is_empty() && app.selected_note < filtered.len() {
        let note = filtered[app.selected_note];

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
        let mut lines = vec![
            Line::styled(metadata_line, Style::default().fg(app.color_scheme.accent_secondary)),
            Line::raw(""),  // Blank line
        ];

        // For markdown, render it cleanly; for other types, use syntax highlighting
        if note.syntax_language == SyntaxLanguage::Markdown {
            // Render markdown with inline formatting and code block highlighting
            lines.extend(render_markdown_for_terminal(&note.content, &app.syntax_highlighter, &app.debug_log));
        } else {
            // Apply syntax highlighting to code
            let highlighted_content = app.syntax_highlighter.highlight(&note.content, note.syntax_language);
            lines.extend(highlighted_content.lines);
        }

        // Add attachments section if there are any attachments
        if !note.attachments.is_empty() {
            let is_focused = app.focused_panel == FocusedPanel::Attachments;
            lines.push(Line::raw("")); // Blank line before attachments

            // Show separator with focus indicator
            let separator_style = if is_focused {
                Style::default().fg(app.color_scheme.accent)
            } else {
                Style::default().fg(app.color_scheme.muted)
            };
            lines.push(Line::styled(
                "─".repeat(40),
                separator_style
            ));

            // Show header with focus indicator
            let header_text = if is_focused {
                "▶ Attachments: (Tab to switch, j/k to navigate, Enter to view, d to delete)"
            } else {
                "Attachments: (Tab to focus)"
            };
            let header_style = if is_focused {
                Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(app.color_scheme.accent)
            };
            lines.push(Line::styled(header_text, header_style));

            for (i, attachment) in note.attachments.iter().enumerate() {
                let size_kb = (attachment.size as f64) / 1024.0;
                let size_str = if size_kb < 1024.0 {
                    format!("{:.1} KB", size_kb)
                } else {
                    format!("{:.1} MB", size_kb / 1024.0)
                };

                let prefix = if is_focused && i == app.selected_attachment {
                    format!("▶ a{}. ", i + 1)
                } else {
                    format!("  a{}. ", i + 1)
                };
                let filename = &attachment.filename;
                let mime = &attachment.mime_type;
                let attachment_line = format!("{}{} ({}) [{}]", prefix, filename, size_str, mime);

                let style = if i == app.selected_attachment {
                    if is_focused {
                        Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD | Modifier::REVERSED)
                    } else {
                        Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
                    }
                } else {
                    Style::default().fg(app.color_scheme.foreground)
                };

                lines.push(Line::styled(attachment_line, style));
            }

            // Only show the shortcut hint if not focused (when focused, header shows the keys)
            if !is_focused {
                lines.push(Line::raw(""));
                lines.push(Line::styled(
                    "Press 'a' + number to view, 'A' to add, 'X' to remove selected",
                    Style::default().fg(app.color_scheme.muted)
                ));
            }
        }

        let preview = Paragraph::new(Text::from(lines))
            .block(preview_block)
            .wrap(Wrap { trim: false })
            .scroll((app.preview_scroll_offset as u16, 0));
        frame.render_widget(preview, preview_area);

        // Render timestamp footer with left-aligned Created and right-aligned Modified
        let created_str = note.created_at.format("%d/%m/%Y, %H:%M:%S").to_string();
        let modified_str = note.modified_at.format("%d/%m/%Y, %H:%M:%S").to_string();

        let created_label = format!("Created: {}", created_str);
        let modified_label = format!("Modified: {}", modified_str);

        // Calculate padding between left and right timestamps
        // -2 for borders, -2 for 1-space padding on each side
        let inner_width = timestamp_area.width.saturating_sub(4) as usize;
        let total_text_len = created_label.len() + modified_label.len();
        let padding = if inner_width > total_text_len {
            inner_width - total_text_len
        } else {
            1
        };

        let border_style = Style::default().fg(app.color_scheme.border);
        let timestamp_style = Style::default().fg(app.color_scheme.accent_secondary);

        let timestamp_line = Line::from(vec![
            Span::styled("│", border_style),
            Span::raw(" "),  // Left padding
            Span::styled(created_label, timestamp_style),
            Span::raw(" ".repeat(padding)),
            Span::styled(modified_label, timestamp_style),
            Span::raw(" "),  // Right padding
            Span::styled("│", border_style),
        ]);

        // Add bottom border line
        let border_width = timestamp_area.width.saturating_sub(2) as usize;
        let border_line = Line::from(vec![
            Span::styled("└", border_style),
            Span::styled("─".repeat(border_width), border_style),
            Span::styled("┘", border_style),
        ]);

        // Create a 2-line footer: timestamp + bottom border
        let timestamp_footer = Paragraph::new(vec![timestamp_line, border_line]);
        frame.render_widget(timestamp_footer, Rect {
            x: timestamp_area.x,
            y: timestamp_area.y,
            width: timestamp_area.width,
            height: 2,
        });
    } else {
        let preview_block_empty = Block::default()
            .title("Preview")
            .borders(Borders::ALL);
        let preview = Paragraph::new("No notes")
            .block(preview_block_empty)
            .alignment(Alignment::Center);
        frame.render_widget(preview, right_pane);
    }

    // Render attachment path input overlay if in AttachmentPath mode
    if matches!(app.input_mode, InputMode::AttachmentPath) {
        // Calculate modal size based on whether completions are showing
        let modal_width = 70;
        let completions_height = if app.path_completions.is_empty() {
            0
        } else {
            std::cmp::min(app.path_completions.len() + 1, 12) as u16  // Max 12 lines
        };
        let modal_height = 4 + completions_height;  // Input + completions
        let x = (size.width.saturating_sub(modal_width)) / 2;
        let y = (size.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(x, y, modal_width, modal_height);

        // Clear the background area
        frame.render_widget(Clear, modal_area);

        // Render modal background
        let modal_block = Block::default()
            .title("Add Attachment (Tab: complete, ↑↓: select, Enter: confirm)")
            .borders(Borders::ALL)
            .style(Style::default().bg(app.color_scheme.background).fg(app.color_scheme.accent));

        // Build content lines
        let mut lines = vec![
            Line::styled(
                format!("Path: {}█", app.attachment_path_input),
                Style::default().fg(app.color_scheme.foreground)
            ),
        ];

        // Add completions if available
        if !app.path_completions.is_empty() {
            lines.push(Line::styled(
                "─".repeat((modal_width - 2) as usize),
                Style::default().fg(app.color_scheme.muted)
            ));

            for (i, completion) in app.path_completions.iter().enumerate().take(10) {
                let is_selected = i == app.path_completion_index;
                let prefix = if is_selected { "▶ " } else { "  " };
                let style = if is_selected {
                    Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.color_scheme.foreground)
                };
                lines.push(Line::styled(format!("{}{}", prefix, completion), style));
            }

            if app.path_completions.len() > 10 {
                lines.push(Line::styled(
                    format!("  ... and {} more", app.path_completions.len() - 10),
                    Style::default().fg(app.color_scheme.muted)
                ));
            }
        }

        let modal_paragraph = Paragraph::new(Text::from(lines))
            .block(modal_block)
            .style(Style::default().fg(app.color_scheme.foreground));

        frame.render_widget(modal_paragraph, modal_area);
    }

    // Render force sync confirmation modal if showing
    if app.show_force_sync_confirm {
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

        // Clear the background area
        frame.render_widget(Clear, modal_area);

        // Render modal
        let modal_block = Block::default()
            .title("Force Full Resync")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.color_scheme.accent))
            .style(Style::default().bg(app.color_scheme.background));

        let modal_text = vec![
            Line::from(""),
            Line::from("Pull ALL notes and attachments from server?").style(Style::default().fg(app.color_scheme.foreground)),
            Line::from("This will overwrite local changes if server is newer.").style(Style::default().fg(app.color_scheme.muted)),
            Line::from(""),
            Line::from(vec![
                Span::raw("Press "),
                Span::styled("y", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" to confirm, "),
                Span::styled("n", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" or "),
                Span::styled("Esc", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" to cancel"),
            ]).style(Style::default().fg(app.color_scheme.foreground)),
        ];

        let modal_paragraph = Paragraph::new(modal_text)
            .block(modal_block)
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(modal_paragraph, modal_area);
    }

    // Render attachment viewer modal if showing
    if matches!(app.view_mode, ViewMode::AttachmentViewer) {
        let filtered = app.filtered_notes();

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

        // Clear the background area
        frame.render_widget(Clear, modal_area);

        // Check if we have valid data to show
        let has_note = !filtered.is_empty() && app.selected_note < filtered.len();
        let note = if has_note { Some(filtered[app.selected_note]) } else { None };
        let has_attachments = note.map(|n| !n.attachments.is_empty()).unwrap_or(false);

        if has_attachments {
            let note = note.unwrap();

            // Render modal with attachments
            let modal_block = Block::default()
                .title(format!(" Attachments ({}) ", note.attachments.len()))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.color_scheme.accent))
                .style(Style::default().bg(app.color_scheme.background));

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

                let style = if i == app.selected_attachment {
                    Style::default()
                        .fg(app.color_scheme.background)
                        .bg(app.color_scheme.accent)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(app.color_scheme.foreground)
                };

                modal_lines.push(Line::styled(line_text, style));
            }

            modal_lines.push(Line::from(""));
            modal_lines.push(Line::styled(
                "↑/↓: navigate │ 1-9: quick select │ Enter: view │ d: delete │ Esc: close",
                Style::default().fg(app.color_scheme.muted)
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
                .border_style(Style::default().fg(app.color_scheme.error))
                .style(Style::default().bg(app.color_scheme.background));

            let error_text = if !has_note {
                "No note selected"
            } else {
                "No attachments in this note"
            };

            let modal_lines = vec![
                Line::from(""),
                Line::styled(error_text, Style::default().fg(app.color_scheme.error)),
                Line::from(""),
                Line::styled("Press Esc to close", Style::default().fg(app.color_scheme.muted)),
            ];

            let modal_paragraph = Paragraph::new(modal_lines)
                .block(modal_block)
                .alignment(ratatui::layout::Alignment::Center);

            frame.render_widget(modal_paragraph, modal_area);
        }
    }

    // Render version history modal if showing
    if matches!(app.view_mode, ViewMode::VersionHistory) {
        // Calculate modal size (larger than attachment viewer for content preview)
        let modal_width = 100.min(size.width.saturating_sub(4));
        let modal_height = 30.min(size.height.saturating_sub(4));
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

        if !app.loaded_versions.is_empty() {
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

            frame.render_widget(version_list, panes[0]);

            // Right pane: Preview of selected version
            if app.selected_version < app.loaded_versions.len() {
                let version = &app.loaded_versions[app.selected_version];

                // Split right pane into content area and help text area
                let preview_chunks = Layout::default()
                    .direction(Direction::Vertical)
                    .constraints([
                        Constraint::Min(0),      // Content area (scrollable)
                        Constraint::Length(1),   // Help text (fixed)
                    ])
                    .split(panes[1]);

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
        } else {
            // Show error state - no versions
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
                .alignment(ratatui::layout::Alignment::Center);

            frame.render_widget(modal_paragraph, modal_area);
        }
    }

    // Render bulk delete confirmation modal if showing
    if app.show_bulk_delete_confirm {
        let count = app.selected_note_ids.len();

        // Create centered modal
        let modal_width = 50;
        let modal_height = 7;
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

        // Render modal
        let modal_block = Block::default()
            .title(" Delete Notes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.color_scheme.error))
            .style(Style::default().bg(app.color_scheme.background));

        let modal_text = vec![
            Line::from(""),
            Line::from(format!("Delete {} notes?", count)).style(Style::default().fg(app.color_scheme.foreground)),
            Line::from("Notes will be moved to recycle bin.").style(Style::default().fg(app.color_scheme.muted)),
            Line::from(""),
            Line::from(vec![
                Span::raw("Press "),
                Span::styled("y", Style::default().fg(app.color_scheme.error).add_modifier(Modifier::BOLD)),
                Span::raw(" to confirm, "),
                Span::styled("n", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" or "),
                Span::styled("Esc", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" to cancel"),
            ]).style(Style::default().fg(app.color_scheme.foreground)),
        ];

        let modal_paragraph = Paragraph::new(modal_text)
            .block(modal_block)
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(modal_paragraph, modal_area);
    }

    // Render bulk combine confirmation modal if showing
    if app.show_bulk_combine_confirm {
        let count = app.selected_note_ids.len();

        // Create centered modal
        let modal_width = 55;
        let modal_height = 8;
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

        // Render modal
        let modal_block = Block::default()
            .title(" Combine Notes ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.color_scheme.accent))
            .style(Style::default().bg(app.color_scheme.background));

        let modal_text = vec![
            Line::from(""),
            Line::from(format!("Combine {} notes into one?", count)).style(Style::default().fg(app.color_scheme.foreground)),
            Line::from("Notes will be merged by creation date.").style(Style::default().fg(app.color_scheme.muted)),
            Line::from("Original notes will be moved to recycle bin.").style(Style::default().fg(app.color_scheme.muted)),
            Line::from(""),
            Line::from(vec![
                Span::raw("Press "),
                Span::styled("y", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" to confirm, "),
                Span::styled("n", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" or "),
                Span::styled("Esc", Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)),
                Span::raw(" to cancel"),
            ]).style(Style::default().fg(app.color_scheme.foreground)),
        ];

        let modal_paragraph = Paragraph::new(modal_text)
            .block(modal_block)
            .alignment(ratatui::layout::Alignment::Center);

        frame.render_widget(modal_paragraph, modal_area);
    }
}
