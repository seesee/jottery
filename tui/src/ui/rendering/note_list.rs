//! Rendering for the note list view (split pane with preview)

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::state::{FocusedPanel, InputMode, ViewMode};
use crate::ui::helpers::{strip_markdown, render_markdown_for_terminal};
use crate::ui::rendering::modal::render_confirmation_modal;
use crate::ui::note_colors::{get_note_color, get_tag_color, is_dark_theme};
use crate::models::SyntaxLanguage;

/// Render note list (split pane view)
pub fn render_note_list(app: &mut App, frame: &mut Frame) {
    app.debug_log("render_note_list() started");
    let size = frame.area();

    // Main layout: content + help at bottom
    app.debug_log(&format!("Frame size: {}x{}", size.width, size.height));
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
    app.debug_log("Calling filtered_notes()");
    let filtered = app.filtered_notes();
    app.debug_log(&format!("filtered_notes() returned {} notes", filtered.len()));
    let title = match app.view_mode {
        ViewMode::RecycleBin => "Recycle Bin".to_string(),
        ViewMode::AttachmentViewer => "Attachment Viewer".to_string(),
        ViewMode::VersionHistory => "Version History".to_string(),
        ViewMode::ConflictResolution => "Conflict Resolution".to_string(),
        ViewMode::NoteList => {
            if app.archive_mode {
                if app.search_active && !app.search_input.is_empty() {
                    format!("Jottery v{} - Archive ({}/{})", env!("CARGO_PKG_VERSION"), filtered.len(), app.notes.len())
                } else {
                    format!("Jottery v{} - Archive", env!("CARGO_PKG_VERSION"))
                }
            } else if app.search_active && !app.search_input.is_empty() {
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
        let search_title = if app.archive_mode {
            "Search archived notes:"
        } else {
            "Search:"
        };
        let search_bar = Paragraph::new(app.search_input.as_str())
            .style(Style::default().fg(app.color_scheme.accent))
            .block(Block::default().title(search_title).borders(Borders::ALL));
        frame.render_widget(search_bar, left_chunks[0]);
        left_chunks[1]
    } else {
        left_chunks[0]
    };

    // Render note list
    let list_block = Block::default()
        .title(title)
        .borders(Borders::ALL);

    app.debug_log("Building list items");
    let items: Vec<ListItem> = filtered
        .iter()
        .enumerate()
        .map(|(i, note)| {
            // Generate multi-line preview (like web UI)
            // Line 1: Title (first non-empty line)
            // Line 2-3: Preview (next 2 lines of content)
            // Line 4: Date + Tags
            // Line 5: Blank separator

            let content_lines: Vec<&str> = note.content.lines()
                .filter(|line| !line.trim().is_empty())
                .collect();

            // Extract title (first line)
            let title_raw = content_lines.get(0).unwrap_or(&"Untitled");
            let title_content = if note.syntax_language == SyntaxLanguage::Markdown {
                strip_markdown(title_raw)
            } else {
                title_raw.to_string()
            };

            // Truncate title to 38 chars to leave room for indicators
            let title_truncated = if title_content.chars().count() > 38 {
                let truncated: String = title_content.chars().take(35).collect();
                format!("{}...", truncated)
            } else {
                title_content
            };

            // Add indicators for pinned, attachments, and multi-select
            let mut indicators = String::new();

            // Show checkbox for multi-select mode
            if app.is_multi_select_mode {
                if app.selected_note_ids.contains(&note.id) {
                    indicators.push_str("[✓] ");
                } else {
                    indicators.push_str("[ ] ");
                }
            }

            if note.pinned {
                indicators.push_str("📌 ");
            }
            if note.locked {
                indicators.push_str("🔒 ");
            }
            if !note.attachments.is_empty() {
                indicators.push_str(&format!("📎{} ", note.attachments.len()));
            }
            // Conflict indicator
            if app.conflict_note_ids.contains(&note.id) {
                indicators.push_str("⚠ ");
            }

            let title_line = if !indicators.is_empty() {
                format!("{}{}", indicators, title_truncated)
            } else {
                title_truncated
            };

            // Extract preview lines (lines 2-3 from content)
            let preview_line1 = if content_lines.len() > 1 {
                let line = content_lines[1];
                let stripped = if note.syntax_language == SyntaxLanguage::Markdown {
                    strip_markdown(line)
                } else {
                    line.to_string()
                };
                if stripped.chars().count() > 38 {
                    let truncated: String = stripped.chars().take(35).collect();
                    format!("{}...", truncated)
                } else {
                    stripped
                }
            } else {
                String::new()
            };

            let preview_line2 = if content_lines.len() > 2 {
                let line = content_lines[2];
                let stripped = if note.syntax_language == SyntaxLanguage::Markdown {
                    strip_markdown(line)
                } else {
                    line.to_string()
                };
                if stripped.chars().count() > 38 {
                    let truncated: String = stripped.chars().take(35).collect();
                    format!("{}...", truncated)
                } else {
                    stripped
                }
            } else {
                String::new()
            };

            // Format date + tags line
            let date_str = note.modified_at.format("%d/%m/%y").to_string();
            let tags_str = if !note.tags.is_empty() {
                let tag_preview: String = note.tags.iter()
                    .take(3)
                    .map(|t| format!("#{}", t))
                    .collect::<Vec<_>>()
                    .join(" ");
                if note.tags.len() > 3 {
                    format!("{} +{}", tag_preview, note.tags.len() - 3)
                } else {
                    tag_preview
                }
            } else {
                String::new()
            };

            let metadata_line = if !tags_str.is_empty() {
                format!("{} • {}", date_str, tags_str)
            } else {
                date_str
            };

            // Get note background color if set (same as web UI)
            let theme_name = app.settings.theme.to_string();
            let is_dark = is_dark_theme(&theme_name);

            // Get palette info for debugging
            let palette = app.settings.get_color_palette();
            app.debug_log(&format!("Note {} - id={}, theme_name={}, is_dark={}, color={:?}, palette_size={}",
                i, &note.id[..8], theme_name, is_dark, note.color, palette.len()));

            // Log detailed color resolution
            if let Some(ref color_key) = note.color {
                app.debug_log(&format!("Note {} - Looking up color_key='{}' in palette", i, color_key));
                if let Some(color_def) = palette.get(color_key) {
                    app.debug_log(&format!("Note {} - Found in palette: light={}, dark={}",
                        i, color_def.light, color_def.dark));
                    let hex = if is_dark { &color_def.dark } else { &color_def.light };
                    app.debug_log(&format!("Note {} - Using hex={}", i, hex));
                } else {
                    app.debug_log(&format!("Note {} - Color '{}' NOT found in palette, will use ANSI fallback",
                        i, color_key));
                }
            }

            let note_bg_color = get_note_color(note.color.as_ref(), &app.settings, is_dark);
            app.debug_log(&format!("Note {} - get_note_color returned: {:?}", i, note_bg_color));

            // Style selected notes differently in multi-select mode
            let is_selected = app.selected_note_ids.contains(&note.id);
            app.debug_log(&format!("Note {} - is_selected={}", i, is_selected));

            let mut style = if is_selected {
                app.debug_log(&format!("Note {} - Using SELECTED style (accent color)", i));
                // Selected notes use accent color
                Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
            } else {
                app.debug_log(&format!("Note {} - Using DEFAULT style", i));
                // Unselected notes use default foreground
                Style::default()
            };

            // Apply background color if note has a color (for both selected and unselected)
            if let Some(bg_color) = note_bg_color {
                app.debug_log(&format!("Note {} - Applying BACKGROUND color: {:?}", i, bg_color));
                style = style.bg(bg_color);
            }

            app.debug_log(&format!("Note {} - Final style: {:?}", i, style));

            // Create multi-line text for this note
            let mut lines = vec![
                // Title: Gray + Bold for emphasis
                Line::from(Span::styled(
                    title_line,
                    Style::default().fg(Color::Gray).add_modifier(Modifier::BOLD)
                )),
            ];

            // Add preview lines if they exist (use gray for readability)
            if !preview_line1.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", preview_line1),
                    Style::default().fg(Color::Gray)
                )));
            }
            if !preview_line2.is_empty() {
                lines.push(Line::from(Span::styled(
                    format!("  {}", preview_line2),
                    Style::default().fg(Color::Gray)
                )));
            }

            // Add metadata line (brighter for better readability)
            lines.push(Line::from(Span::styled(
                format!("  {}", metadata_line),
                Style::default().fg(Color::White)
            )));

            // Add blank separator line
            lines.push(Line::from(""));

            let text = Text::from(lines);
            ListItem::new(text).style(style)
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

    // Store note list inner area for mouse click detection (will assign to app at end)
    // Inner area is list_chunk minus the border (1 pixel on each side)
    let note_list_inner_area = Rect {
        x: list_chunk.x + 1,
        y: list_chunk.y + 1,
        width: list_chunk.width.saturating_sub(2),
        height: list_chunk.height.saturating_sub(2),
    };

    // Help text (full width at bottom)
    let status_text = if let Some(ref status) = app.sync_status {
        status.clone()
    } else if app.search_active {
        // Show count of matching notes in opposite mode (if > 0)
        let opposite_count = app.count_opposite_mode_matches();
        if opposite_count > 0 {
            let mode_name = if app.archive_mode { "active" } else { "archive" };
            format!("Type: search | Esc: exit | ↑/↓: navigate | {} {} in {}",
                opposite_count,
                if opposite_count == 1 { "match" } else { "matches" },
                mode_name)
        } else {
            "Type: search | Esc: exit | ↑/↓: navigate".to_string()
        }
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
            ViewMode::ConflictResolution => {
                "1: Keep Mine | 2: Keep Server | 3: Keep Both | Tab: switch | j/k: scroll | Esc: cancel".to_string()
            }
            ViewMode::NoteList => {
                if app.archive_mode {
                    "A: unarchive | Shift+A: toggle mode | ↑/↓: navigate | /: search | Esc: exit archive".to_string()
                } else if app.is_multi_select_mode {
                    format!("Space: toggle | Esc: clear | t: add tags | d: delete | c: combine | e: export ({} selected)", app.selected_note_ids.len())
                } else {
                    "?: help | /: search | Space: select | p: pin | t: tags | []: type | r: bin | Shift+A: archive | v: ver | n: new".to_string()
                }
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

    // Variables for mouse click detection (assigned inside if block, stored at end)
    let mut tags_line_area_local: Option<Rect> = None;
    let mut tag_positions_local: Vec<(String, u16, u16)> = Vec::new();
    let mut note_link_positions_local: Vec<(String, u16, u16, u16)> = Vec::new();
    let mut preview_area_local: Option<Rect> = None;

    if !filtered.is_empty() && app.selected_note < filtered.len() {
        let note = filtered[app.selected_note];

        // Show tags (or n/a if none)
        // Also calculate tag positions for click detection
        let tags_prefix = "Tags: ";
        let preview_inner_x = preview_area.x + 1; // Account for border
        let preview_inner_y = preview_area.y + 1; // Account for title bar
        let scroll_offset = app.preview_scroll_offset;

        // Determine theme for tag colors
        let theme_name = app.settings.theme.to_string();
        let is_dark = is_dark_theme(&theme_name);

        let tags_spans = if !note.tags.is_empty() {
            let mut x_offset = preview_inner_x + tags_prefix.len() as u16;
            let mut spans = vec![Span::raw(tags_prefix)];

            for (i, tag) in note.tags.iter().enumerate() {
                let tag_str = format!("#{}", tag);
                let tag_len = tag_str.len() as u16;

                // Store position only if visible (scroll offset is 0 for tags line)
                if scroll_offset == 0 {
                    tag_positions_local.push((tag.clone(), x_offset, x_offset + tag_len));
                }
                x_offset += tag_len + 1; // +1 for space separator

                // Get tag color and create styled span
                app.debug_log(&format!("Tag '{}' - looking up color", tag));
                let tag_color = get_tag_color(tag, &app.settings, is_dark);
                app.debug_log(&format!("Tag '{}' - color resolved: {:?}", tag, tag_color.is_some()));
                let tag_style = if let Some(color) = tag_color {
                    Style::default().bg(color).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                spans.push(Span::styled(tag_str, tag_style));

                // Add space separator if not the last tag
                if i < note.tags.len() - 1 {
                    spans.push(Span::raw(" "));
                }
            }

            Some(spans)
        } else {
            None
        };

        // Store tags line area for click detection (only visible when scroll offset is 0)
        tags_line_area_local = if scroll_offset == 0 {
            Some(Rect {
                x: preview_inner_x,
                y: preview_inner_y,
                width: preview_area.width.saturating_sub(2),
                height: 1,
            })
        } else {
            None
        };

        // Build metadata line with spans
        let mut metadata_spans = Vec::new();

        // Add tags spans (or "n/a")
        if let Some(mut tag_spans) = tags_spans {
            metadata_spans.append(&mut tag_spans);
        } else {
            metadata_spans.push(Span::raw("Tags: "));
            metadata_spans.push(Span::raw("n/a"));
        }

        // Add separator
        metadata_spans.push(Span::raw(" | "));

        // Add syntax language
        metadata_spans.push(Span::styled(
            format!("Type: {}", note.syntax_language),
            Style::default().fg(app.color_scheme.accent_secondary)
        ));

        let metadata_line = Line::from(metadata_spans);

        // Render content based on type
        let mut lines = vec![
            metadata_line,
            Line::raw(""),  // Blank line
        ];

        // Store the preview area for link click detection
        preview_area_local = Some(preview_area);

        // Calculate the starting line index for content (after metadata + blank line)
        let content_start_line_index = lines.len();

        // For markdown, render it cleanly; for other types, use syntax highlighting
        if note.syntax_language == SyntaxLanguage::Markdown {
            // Create a lookup function for note titles (used for empty link text)
            let note_title_lookup = |note_id: &str| -> Option<String> {
                app.notes.iter()
                    .find(|n| n.id == note_id)
                    .map(|n| {
                        // Get first non-empty line as title
                        n.content.lines()
                            .find(|line| !line.trim().is_empty())
                            .map(|line| strip_markdown(line))
                            .unwrap_or_else(|| "Untitled".to_string())
                    })
            };

            // Render markdown with inline formatting and code block highlighting
            let render_result = render_markdown_for_terminal(&note.content, &app.syntax_highlighter, &app.debug_log, Some(note_title_lookup));

            // Convert link positions from render result to screen coordinates
            // The y coordinate is: preview_inner_y + content_start_line_index + link_line_index - scroll_offset
            let preview_inner_x = preview_area.x + 1; // Account for border
            let preview_inner_y = preview_area.y + 1; // Account for title bar
            for (note_id, line_idx, char_start, char_end) in render_result.note_links {
                // Calculate screen y position
                let screen_y = preview_inner_y as i32 + content_start_line_index as i32 + line_idx as i32 - scroll_offset as i32;
                // Only add if the line is visible (y >= preview_inner_y)
                if screen_y >= preview_inner_y as i32 && screen_y < (preview_area.y + preview_area.height - 1) as i32 {
                    note_link_positions_local.push((
                        note_id,
                        screen_y as u16,
                        preview_inner_x + char_start as u16,
                        preview_inner_x + char_end as u16,
                    ));
                }
            }

            lines.extend(render_result.lines);
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

        // Render footer with left-aligned Version + sync indicator and right-aligned Modified
        let modified_str = note.modified_at.format("%d/%m/%Y, %H:%M:%S").to_string();

        // Determine sync status: synced if synced_at exists and is >= modified_at
        let is_synced = match &note.synced_at {
            Some(synced) => *synced >= note.modified_at,
            None => false,
        };

        // Sync indicator: filled green circle for synced, empty red circle for unsynced
        let (sync_indicator, sync_color) = if is_synced {
            ("●", Color::Green)
        } else {
            ("○", Color::Red)
        };

        let version_label = format!("Version: {}", note.version);
        let modified_label = format!("Modified: {}", modified_str);

        // Calculate padding between left and right sections
        // -2 for borders, -2 for 1-space padding on each side
        let inner_width = timestamp_area.width.saturating_sub(4) as usize;
        // version_label + space + sync_indicator (1 char) + modified_label
        let total_text_len = version_label.len() + 1 + 1 + modified_label.len();
        let padding = if inner_width > total_text_len {
            inner_width - total_text_len
        } else {
            1
        };

        let timestamp_style = Style::default().fg(app.color_scheme.accent_secondary);

        let timestamp_line = Line::from(vec![
            Span::raw("│"),
            Span::raw(" "),  // Left padding
            Span::styled(version_label, timestamp_style),
            Span::raw(" "),
            Span::styled(sync_indicator, Style::default().fg(sync_color)),
            Span::raw(" ".repeat(padding)),
            Span::styled(modified_label, timestamp_style),
            Span::raw(" "),  // Right padding
            Span::raw("│"),
        ]);

        // Add bottom border line
        let border_width = timestamp_area.width.saturating_sub(2) as usize;
        let border_line = Line::from(vec![
            Span::raw("└"),
            Span::raw("─".repeat(border_width)),
            Span::raw("┘"),
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
        render_confirmation_modal(
            frame,
            size,
            "Force Full Resync",
            vec![
                Line::from("Pull ALL notes and attachments from server?")
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from("This will overwrite local changes if server is newer.")
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.accent,
            &app.color_scheme,
            60,
            7,
        );
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
        render_confirmation_modal(
            frame,
            size,
            " Delete Notes ",
            vec![
                Line::from(format!("Delete {} notes?", count))
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from("Notes will be moved to recycle bin.")
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.error,
            &app.color_scheme,
            50,
            7,
        );
    }

    // Render bulk combine confirmation modal if showing
    if app.show_bulk_combine_confirm {
        let count = app.selected_note_ids.len();
        render_confirmation_modal(
            frame,
            size,
            " Combine Notes ",
            vec![
                Line::from(format!("Combine {} notes into one?", count))
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from("Notes will be merged by creation date.")
                    .style(Style::default().fg(app.color_scheme.muted)),
                Line::from("Original notes will be moved to recycle bin.")
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.accent,
            &app.color_scheme,
            55,
            8,
        );
    }

    // Render bulk add tags input modal if showing
    if matches!(app.input_mode, InputMode::BulkAddTags) {
        let count = app.selected_note_ids.len();
        let modal_width = 60;
        let modal_height = 6;
        let x = (size.width.saturating_sub(modal_width)) / 2;
        let y = (size.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(x, y, modal_width, modal_height);

        // Clear the background area
        frame.render_widget(Clear, modal_area);

        let modal_block = Block::default()
            .title(format!(" {} ", t!("bulk.enter_tags")))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.color_scheme.accent))
            .style(Style::default().bg(app.color_scheme.background));

        let lines = vec![
            Line::styled(
                format!("{} {} {}", t!("bulk.add_tags_to"), count, if count == 1 { t!("sync.note") } else { t!("sync.notes") }),
                Style::default().fg(app.color_scheme.foreground)
            ),
            Line::styled(
                format!("{}█", app.bulk_tags_input),
                Style::default().fg(app.color_scheme.accent)
            ),
            Line::styled(
                "Enter: confirm | Esc: cancel",
                Style::default().fg(app.color_scheme.muted)
            ),
        ];

        let modal_paragraph = Paragraph::new(Text::from(lines))
            .block(modal_block)
            .style(Style::default().fg(app.color_scheme.foreground));

        frame.render_widget(modal_paragraph, modal_area);
    }

    // Render bulk export path input modal if showing
    if matches!(app.input_mode, InputMode::BulkExportPath) {
        let count = app.selected_note_ids.len();
        let modal_width = 70;
        let modal_height = 6;
        let x = (size.width.saturating_sub(modal_width)) / 2;
        let y = (size.height.saturating_sub(modal_height)) / 2;

        let modal_area = Rect::new(x, y, modal_width, modal_height);

        // Clear the background area
        frame.render_widget(Clear, modal_area);

        let modal_block = Block::default()
            .title(format!(" {} ", t!("bulk.enter_export_path")))
            .borders(Borders::ALL)
            .border_style(Style::default().fg(app.color_scheme.accent))
            .style(Style::default().bg(app.color_scheme.background));

        let lines = vec![
            Line::styled(
                format!("{} {} {}", t!("bulk.export_notes_to"), count, if count == 1 { t!("sync.note") } else { t!("sync.notes") }),
                Style::default().fg(app.color_scheme.foreground)
            ),
            Line::styled(
                format!("{}█", app.bulk_export_path_input),
                Style::default().fg(app.color_scheme.accent)
            ),
            Line::styled(
                "Enter: confirm | Esc: cancel",
                Style::default().fg(app.color_scheme.muted)
            ),
        ];

        let modal_paragraph = Paragraph::new(Text::from(lines))
            .block(modal_block)
            .style(Style::default().fg(app.color_scheme.foreground));

        frame.render_widget(modal_paragraph, modal_area);
    }

    // Render conflict resolution modal if showing
    if matches!(app.view_mode, ViewMode::ConflictResolution) {
        super::conflict::render_conflict_modal(app, frame, size);
    }

    // Render note info modal if showing
    if app.show_note_info {
        let filtered = app.filtered_notes();
        if !filtered.is_empty() && app.selected_note < filtered.len() {
            let note = filtered[app.selected_note];

            // Calculate modal size
            let modal_width = 70.min(size.width.saturating_sub(4));
            let modal_height = 20.min(size.height.saturating_sub(4));
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

            let modal_block = Block::default()
                .title(format!(" {} ", t!("note.info_title")))
                .borders(Borders::ALL)
                .border_style(Style::default().fg(app.color_scheme.accent))
                .style(Style::default().bg(app.color_scheme.background));

            // Build info lines
            let mut info_lines = vec![Line::from("")];

            // Note ID (truncated)
            let id_display = if note.id.len() > 36 { &note.id[..36] } else { &note.id };
            info_lines.push(Line::from(vec![
                Span::styled("ID:          ", Style::default().fg(app.color_scheme.muted)),
                Span::styled(id_display, Style::default().fg(app.color_scheme.foreground)),
            ]));

            // Created date
            info_lines.push(Line::from(vec![
                Span::styled("Created:     ", Style::default().fg(app.color_scheme.muted)),
                Span::styled(
                    note.created_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Style::default().fg(app.color_scheme.foreground)
                ),
            ]));

            // Modified date
            info_lines.push(Line::from(vec![
                Span::styled("Modified:    ", Style::default().fg(app.color_scheme.muted)),
                Span::styled(
                    note.modified_at.format("%Y-%m-%d %H:%M:%S").to_string(),
                    Style::default().fg(app.color_scheme.foreground)
                ),
            ]));

            // Synced date
            let synced_str = match &note.synced_at {
                Some(dt) => dt.format("%Y-%m-%d %H:%M:%S").to_string(),
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
            let color_str = note.color.as_ref().map(|c| c.as_str()).unwrap_or("None");
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
            let status_str = if status_parts.is_empty() { "None".to_string() } else { status_parts.join("  ") };
            info_lines.push(Line::from(vec![
                Span::styled("Status:      ", Style::default().fg(app.color_scheme.muted)),
                Span::styled(status_str, Style::default().fg(app.color_scheme.foreground)),
            ]));

            // Locked date (if locked)
            if note.locked {
                if let Some(locked_at) = &note.locked_at {
                    info_lines.push(Line::from(vec![
                        Span::styled("Locked at:   ", Style::default().fg(app.color_scheme.muted)),
                        Span::styled(
                            locked_at.format("%Y-%m-%d %H:%M:%S").to_string(),
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

            let modal_paragraph = Paragraph::new(info_lines)
                .block(modal_block)
                .alignment(Alignment::Left);

            frame.render_widget(modal_paragraph, modal_area);
        }
    }

    // Store click detection areas in app (after all borrows of filtered are done)
    app.note_list_area = Some(note_list_inner_area);
    app.tags_line_area = tags_line_area_local;
    app.tag_positions = tag_positions_local;
    app.note_link_positions = note_link_positions_local;
    app.preview_area = preview_area_local;
}
