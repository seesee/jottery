//! Rendering for the note list view (split pane with preview)

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::state::{FocusedPanel, InputMode, ViewMode};
use crate::ui::helpers::{strip_markdown, render_markdown_for_terminal, truncate_to_width, display_width, format_date_short, format_datetime_full};
use crate::ui::rendering::modal::{centered_rect, render_confirmation_modal, render_input_modal};
use crate::ui::note_colors::{get_note_color, get_tag_color, is_dark_theme};
use crate::models::{Note, SyntaxLanguage, get_note_title, get_regular_tags};

/// Where a note landed in the list area: rows `top..bottom`, counted from the top of
/// the area, and its index in the visible notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NoteRow {
    pub top: u16,
    pub bottom: u16,
    pub index: usize,
}

/// Build the list item for one note.
fn note_list_item(app: &App, note: &Note) -> ListItem<'static> {
    // Generate multi-line preview (like web UI)
    // Line 1: Title (first non-empty line)
    // Line 2-3: Preview (next 2 lines of content)
    // Line 4: Date + Tags
    // Line 5: Blank separator

    // Extract title using custom t: tag or first non-empty line
    let title_raw = get_note_title(&note.content, &note.tags);
    let title_content = if note.syntax_language == SyntaxLanguage::Markdown {
        strip_markdown(&title_raw)
    } else {
        title_raw
    };

    // Get content lines for preview (excluding first line if no custom title)
    let content_lines: Vec<&str> = note.content.lines()
        .filter(|line| !line.trim().is_empty())
        .take(3)
        .collect();

    // Build indicators for pinned, attachments, and multi-select FIRST
    // so we can calculate remaining space for title
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

    // Calculate available width for title (40 columns minus borders, minus indicator width)
    // Left pane is 42 cols, borders take 2, leaving 40 for content
    let max_line_width: usize = 40;
    let indicator_width = display_width(&indicators);
    let available_title_width = max_line_width.saturating_sub(indicator_width);

    // Truncate title to fit in remaining space
    // Use Unicode-aware width calculation to handle emojis and wide characters
    let title_truncated = truncate_to_width(&title_content, available_title_width);

    let title_line = if !indicators.is_empty() {
        format!("{}{}", indicators, title_truncated)
    } else {
        title_truncated
    };

    // Extract preview lines (lines 2-3 from content)
    // Use Unicode-aware width calculation for proper truncation
    // Preview lines are indented by 2 spaces, so reduce available width
    let preview_max_width = max_line_width.saturating_sub(2);
    let preview_of = |line: Option<&&str>| -> String {
        let Some(line) = line else {
            return String::new();
        };
        let stripped = if note.syntax_language == SyntaxLanguage::Markdown {
            strip_markdown(line)
        } else {
            line.to_string()
        };
        truncate_to_width(&stripped, preview_max_width)
    };
    let preview_line1 = preview_of(content_lines.get(1));
    let preview_line2 = preview_of(content_lines.get(2));

    // Format date + tags line (locale-aware)
    // Filter out virtual tags (e.g., t: title tags)
    let date_str = format_date_short(&note.modified_at);
    let regular_tags = get_regular_tags(&note.tags);
    let tags_str = if !regular_tags.is_empty() {
        let tag_preview: String = regular_tags.iter()
            .take(3)
            .map(|t| format!("#{}", t))
            .collect::<Vec<_>>()
            .join(" ");
        if regular_tags.len() > 3 {
            format!("{} +{}", tag_preview, regular_tags.len() - 3)
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
    let is_dark = is_dark_theme(&app.settings.theme.to_string());
    let note_bg_color = get_note_color(note.color.as_ref(), &app.settings, is_dark);

    // Style selected notes differently in multi-select mode
    let mut style = if app.selected_note_ids.contains(&note.id) {
        // Selected notes use accent color
        Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
    } else {
        // Unselected notes use default foreground
        Style::default()
    };

    // Apply background color if note has a color (for both selected and unselected)
    if let Some(bg_color) = note_bg_color {
        style = style.bg(bg_color);
    }

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

    ListItem::new(Text::from(lines)).style(style)
}

/// Work out which notes fit in `height` rows, keeping the selected note on screen.
///
/// Returns the index of the first one, their list items, and where each landed.
fn visible_list_items(
    app: &App,
    filtered: &[&Note],
    height: usize,
) -> (usize, Vec<ListItem<'static>>, Vec<NoteRow>) {
    if filtered.is_empty() || height == 0 {
        return (0, Vec::new(), Vec::new());
    }

    let selected = app.selected_note.min(filtered.len() - 1);

    // The earliest first note that still leaves the selection fully on screen
    let mut earliest = selected;
    let mut used = note_list_item(app, filtered[selected]).height();
    while earliest > 0 {
        let above = note_list_item(app, filtered[earliest - 1]).height();
        if used + above > height {
            break;
        }
        used += above;
        earliest -= 1;
    }

    // Stay where the list was unless the selection has moved out of view
    let offset = app.note_list_offset.clamp(earliest, selected);

    let mut items = Vec::new();
    let mut rows = Vec::new();
    let mut top = 0;
    for (index, note) in filtered.iter().enumerate().skip(offset) {
        if top >= height {
            break;
        }
        let item = note_list_item(app, note);
        let bottom = top + item.height();

        // The list leaves out an item that doesn't fit, so nothing's clickable there
        if bottom <= height {
            rows.push(NoteRow { top: top as u16, bottom: bottom as u16, index });
        }
        items.push(item);
        top = bottom;
    }

    (offset, items, rows)
}

/// Render note list (split pane view)
pub fn render_note_list(app: &mut App, frame: &mut Frame) {
    let size = frame.area();

    // Main layout: content + optional inbox indicator + help at bottom
    let show_inbox_bar = !app.inbox_items.is_empty() && matches!(app.view_mode, ViewMode::NoteList);
    let main_layout = if show_inbox_bar {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1), Constraint::Length(1)])
            .split(size)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(size)
    };

    let content_area = main_layout[0];
    let inbox_area = if show_inbox_bar { Some(main_layout[1]) } else { None };
    let help_area = if show_inbox_bar { main_layout[2] } else { main_layout[1] };

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
    let notes_in_this_view = app.notes.iter().filter(|n| n.archived == app.archive_mode).count();
    let title = match app.view_mode {
        ViewMode::RecycleBin => t!("ui.recycle_bin").to_string(),
        ViewMode::AttachmentViewer => t!("ui.attachment_viewer").to_string(),
        ViewMode::VersionHistory => t!("ui.version_history").to_string(),
        ViewMode::ConflictResolution => t!("ui.conflict_resolution").to_string(),
        ViewMode::Inbox => format!("{} ({})", t!("inbox.title"), app.inbox_items.len()),
        ViewMode::SavedSearches => t!("saved_search.title").to_string(),
        ViewMode::NoteList => {
            if app.archive_mode {
                if app.search_active && !app.search_input.is_empty() {
                    format!("Jottery v{} - {} ({}/{})", env!("CARGO_PKG_VERSION"), t!("ui.archive"), filtered.len(), notes_in_this_view)
                } else {
                    format!("Jottery v{} - {}", env!("CARGO_PKG_VERSION"), t!("ui.archive"))
                }
            } else if app.search_active && !app.search_input.is_empty() {
                format!("Jottery v{} - {} ({}/{})", env!("CARGO_PKG_VERSION"), t!("ui.notes"), filtered.len(), notes_in_this_view)
            } else {
                format!("Jottery v{} - {}", env!("CARGO_PKG_VERSION"), t!("ui.notes"))
            }
        }
    };

    // Notes the vault can't read would otherwise just be missing from the list
    let title = if app.unreadable_notes > 0 {
        format!("{} ⚠ {}", title, app.unreadable_notes)
    } else {
        title
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
            t!("ui.search_archive").to_string()
        } else {
            t!("ui.search").to_string()
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

    // Only the notes on screen are built, so a large vault costs no more than a
    // small one. List items are several rows tall, which is why this is worked out
    // here and not left to the List widget: mouse clicks need the same answer.
    let list_height = list_chunk.height.saturating_sub(2) as usize;
    let (list_offset, items, rows) = visible_list_items(app, &filtered, list_height);

    let list = List::new(items)
        .block(list_block)
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .fg(app.color_scheme.accent)
        );

    // The list holds just the visible window, so selection is relative to it
    let mut list_state = ratatui::widgets::ListState::default();
    list_state.select(app.selected_note.checked_sub(list_offset));

    frame.render_stateful_widget(list, list_chunk, &mut list_state);

    // Store note list inner area for mouse click detection (will assign to app at end)
    // Inner area is list_chunk minus the border (1 pixel on each side)
    let note_list_inner_area = Rect {
        x: list_chunk.x + 1,
        y: list_chunk.y + 1,
        width: list_chunk.width.saturating_sub(2),
        height: list_chunk.height.saturating_sub(2),
    };

    // Help text (full width at bottom). An error comes first: handlers report
    // failures through app.error, and this is the only place they'd be seen.
    let status_text = if let Some(ref error) = app.error {
        error.clone()
    } else if let Some(ref status) = app.sync_status {
        status.clone()
    } else if app.search_active {
        // Show count of matching notes in opposite mode (if > 0)
        let opposite_count = app.count_opposite_mode_matches();
        if opposite_count > 0 {
            let mode_name = if app.archive_mode { t!("status_bar.in_active") } else { t!("status_bar.in_archive") };
            format!("{} | Esc: {} | ↑/↓: {} | {} {} {}",
                t!("status_bar.type_search"), t!("status_bar.exit"), t!("status_bar.navigate"),
                opposite_count,
                if opposite_count == 1 { t!("status_bar.match") } else { t!("status_bar.matches") },
                mode_name)
        } else {
            format!("{} | Esc: {} | ↑/↓: {}",
                t!("status_bar.type_search"), t!("status_bar.exit"), t!("status_bar.navigate"))
        }
    } else {
        match app.view_mode {
            ViewMode::RecycleBin => {
                format!("r: {} | E: {} | Esc: {} | ↑/↓: {}",
                    t!("status_bar.restore"), t!("status_bar.empty_bin"),
                    t!("status_bar.back_to_notes"), t!("status_bar.navigate"))
            }
            ViewMode::AttachmentViewer => {
                format!("↑/↓: {} | 1-9: {} | Enter: {} | S: {} | +: {} | d: {} | Esc: {}",
                    t!("status_bar.navigate"), t!("status_bar.quick_select"),
                    t!("status_bar.view"), t!("status_bar.save"), t!("status_bar.add"),
                    t!("status_bar.delete"), t!("status_bar.close"))
            }
            ViewMode::VersionHistory => {
                format!("↑/↓: {} | Enter: {} | Esc: {}",
                    t!("status_bar.navigate"), t!("status_bar.restore_version"), t!("status_bar.close"))
            }
            ViewMode::ConflictResolution => {
                format!("1: {} | 2: {} | 3: {} | Tab: {} | j/k: {} | Esc: {}",
                    t!("status_bar.keep_mine"), t!("status_bar.keep_server"), t!("status_bar.keep_both"),
                    t!("status_bar.switch"), t!("status_bar.scroll"), t!("status_bar.cancel"))
            }
            ViewMode::Inbox => {
                format!("Enter: {} | d: {} | A: {} | D: {} | ↑/↓: {} | Esc: {}",
                    t!("status_bar.accept"), t!("status_bar.delete"), t!("status_bar.accept_all"),
                    t!("status_bar.delete_all"), t!("status_bar.navigate"), t!("status_bar.close"))
            }
            ViewMode::SavedSearches => {
                format!("Enter: {} | d: {} | ↑/↓: {} | Esc: {}",
                    t!("status_bar.search"), t!("status_bar.delete"), t!("status_bar.navigate"), t!("status_bar.close"))
            }
            ViewMode::NoteList => {
                if app.archive_mode {
                    format!("a: {} | Shift+A: {} | ↑/↓: {} | /: {} | Esc: {}",
                        t!("status_bar.unarchive"), t!("status_bar.toggle_mode"),
                        t!("status_bar.navigate"), t!("status_bar.search"), t!("status_bar.exit_archive"))
                } else if app.is_multi_select_mode {
                    format!("Space: {} | Esc: {} | t: {} | d: {} | z: {} | c: {} | e: {} ({} {})",
                        t!("status_bar.toggle"), t!("status_bar.clear"), t!("status_bar.add_tags"),
                        t!("status_bar.delete"), t!("status_bar.archive"), t!("status_bar.combine"),
                        t!("status_bar.export"), app.selected_note_ids.len(), t!("status_bar.selected"))
                } else {
                    format!("?: {} | /: {} | Space: {} | p: {} | t: {} | []: {} | r: {} | z: {} | v: {} | n: {}",
                        t!("status_bar.help"), t!("status_bar.search"), t!("status_bar.select"),
                        t!("status_bar.pin"), t!("status_bar.tags"), t!("status_bar.type"),
                        t!("status_bar.bin"), t!("status_bar.archive"), t!("status_bar.ver"), t!("status_bar.new"))
                }
            }
        }
    };
    let help = Paragraph::new(status_text)
        .style(if app.error.is_some() {
            Style::default().fg(app.color_scheme.error)
        } else if let Some(ref status) = app.sync_status {
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

    // Render inbox indicator bar if inbox has items (only in NoteList mode)
    if let Some(inbox_area) = inbox_area {
        let inbox_text = format!("▶ {} — {}",
            t!("inbox.indicator", count = app.inbox_items.len()),
            t!("status_bar.press_b_open"));
        let inbox_bar = Paragraph::new(inbox_text)
            .style(Style::default()
                .fg(Color::Yellow)
                .add_modifier(Modifier::BOLD))
            .alignment(Alignment::Center);
        frame.render_widget(inbox_bar, inbox_area);
    }

    // Right pane: note preview with timestamp footer
    // Split into main preview area and timestamp footer (2 lines: timestamp + border)
    let preview_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(2)])
        .split(right_pane);

    let preview_area = preview_layout[0];
    let timestamp_area = preview_layout[1];

    let preview_block = Block::default()
        .title(t!("ui.preview").to_string())
        .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT);

    // Variables for mouse click detection (assigned inside if block, stored at end)
    let mut tags_line_area_local: Option<Rect> = None;
    let mut tag_positions_local: Vec<(String, u16, u16)> = Vec::new();
    let mut note_link_positions_local: Vec<(String, u16, u16, u16)> = Vec::new();
    let mut preview_scroll = None;
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

        // Filter out virtual tags for display
        let preview_regular_tags = get_regular_tags(&note.tags);
        let tags_spans = if !preview_regular_tags.is_empty() {
            let mut x_offset = preview_inner_x + display_width(tags_prefix) as u16;
            let mut spans = vec![Span::raw(tags_prefix)];

            for (i, tag) in preview_regular_tags.iter().enumerate() {
                let tag_str = format!("#{}", tag);
                let tag_display_width = display_width(&tag_str) as u16;

                // Store position only if visible (scroll offset is 0 for tags line)
                if scroll_offset == 0 {
                    tag_positions_local.push(((*tag).clone(), x_offset, x_offset + tag_display_width));
                }
                x_offset += tag_display_width + 1; // +1 for space separator

                // Get tag color and create styled span
                let tag_color = get_tag_color(tag, &app.settings, is_dark);
                let tag_style = if let Some(color) = tag_color {
                    Style::default().bg(color).add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                };

                spans.push(Span::styled(tag_str, tag_style));

                // Add space separator if not the last tag
                if i < preview_regular_tags.len() - 1 {
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
            format!("{} {}", t!("ui.type"), note.syntax_language),
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

        // The terminal cell a tab lands on depends on where it is, and a paragraph drops them
        let content = crate::ui::helpers::expand_tabs(&note.content);
        let content = crate::ui::helpers::resolve_wiki_links(&content, &|reference: &str| {
            app.notes.iter()
                .find(|other| other.id == reference
                    || crate::models::get_note_title(&other.content, &other.tags).eq_ignore_ascii_case(reference))
                .map(|other| (other.id.clone(), crate::models::get_note_title(&other.content, &other.tags)))
        }).into_owned();

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
                            .map(strip_markdown)
                            .unwrap_or_else(|| "Untitled".to_string())
                    })
            };

            // Render markdown with inline formatting and code block highlighting
            let render_result = render_markdown_for_terminal(&content, &app.syntax_highlighter, &app.debug_log, Some(note_title_lookup));

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
            let highlighted_content = app.syntax_highlighter.highlight(&content, note.syntax_language);
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
                format!("▶ {}", t!("ui.attachments_focused_hint"))
            } else {
                t!("ui.attachments_hint").to_string()
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
                    t!("ui.attachments_keys_hint").to_string(),
                    Style::default().fg(app.color_scheme.muted)
                ));
            }
        }

        // Clear the preview area first to remove any remnants from previous content
        // This prevents stray characters when switching notes or scrolling
        frame.render_widget(Clear, preview_area);

        let preview = Paragraph::new(Text::from(lines)).block(preview_block);
        let preview = if note.word_wrap { preview.wrap(Wrap { trim: false }) } else { preview };
        let scroll = app.preview_scroll_offset.min(super::furthest_scroll(&preview, preview_area));
        preview_scroll = Some(scroll);
        frame.render_widget(preview.scroll((super::scroll_rows(scroll), 0)), preview_area);

        // The link the keyboard is on
        if let Some((_, y, x_start, x_end)) = app.selected_link.and_then(|index| note_link_positions_local.get(index)) {
            let area = Rect::new(*x_start, *y, x_end.saturating_sub(*x_start), 1).intersection(frame.area());
            frame.buffer_mut().set_style(area, Style::default().add_modifier(Modifier::REVERSED));
        }

        // Render footer with left-aligned Version + sync indicator and right-aligned Modified
        let modified_str = format_datetime_full(&note.modified_at);

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

        let version_label = format!("{} {}", t!("ui.version"), note.version);
        let modified_label = format!("{} {}", t!("ui.modified"), modified_str);

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
            .title(t!("ui.preview").to_string())
            .borders(Borders::ALL);
        let preview = Paragraph::new(t!("note.no_notes").to_string())
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
        let modal_area = centered_rect(modal_width, modal_height, size);

        // Clear the background area
        frame.render_widget(Clear, modal_area);

        // Render modal background
        let modal_title = if app.attachment_to_save.is_some() {
            t!("ui.save_attachment").to_string()
        } else {
            t!("ui.add_attachment").to_string()
        };
        let modal_block = Block::default()
            .title(modal_title)
            .borders(Borders::ALL)
            .style(Style::default().bg(app.color_scheme.background).fg(app.color_scheme.accent));

        // Build content lines
        let mut lines = vec![
            Line::styled(
                format!("{}: {}█", t!("ui.path"), app.attachment_path_input),
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
            &t!("sync.force_title"),
            vec![
                Line::from(t!("sync.force_confirm").to_string())
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from(t!("sync.force_note").to_string())
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.accent,
            &app.color_scheme,
            60,
            7,
        );
    }

    // Render the question about an attachment that won't be opened
    if let Some(attachment) = &app.attachment_to_confirm {
        render_confirmation_modal(
            frame,
            size,
            &t!("attachment.unsafe_title"),
            vec![
                Line::from(format!("{} ({})", attachment.filename, attachment.mime_type))
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from(t!("attachment.unsafe_body").to_string())
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.accent,
            &app.color_scheme,
            96,
            7,
        );
    }

    // Render empty bin confirmation modal if showing
    if app.held_deletions > 0 {
        render_confirmation_modal(
            frame,
            size,
            &t!("sync.deletions_held_title"),
            vec![
                Line::from(t!("sync.deletions_held", count = app.held_deletions).to_string())
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from(t!("sync.deletions_held_warning").to_string())
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.error,
            &app.color_scheme,
            96,
            7,
        );
    }

    if app.show_empty_bin_confirm {
        render_confirmation_modal(
            frame,
            size,
            &t!("note.empty_bin_title"),
            vec![
                Line::from(t!("note.empty_bin_confirm", count = app.notes.len()).to_string())
                    .style(Style::default().fg(app.color_scheme.foreground)),
                Line::from(t!("note.empty_bin_warning").to_string())
                    .style(Style::default().fg(app.color_scheme.muted)),
            ],
            app.color_scheme.error,
            &app.color_scheme,
            96,
            7,
        );
    }

    // Render attachment viewer modal if showing
    if matches!(app.view_mode, ViewMode::AttachmentViewer) {
        let filtered = app.filtered_notes();

        // Calculate modal size
        let modal_width = 80.min(size.width.saturating_sub(4));
        let modal_height = 10.min(size.height.saturating_sub(4));
        let modal_area = centered_rect(modal_width, modal_height, size);

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
                .title(format!(" {} ({}) ", t!("ui.attachments"), note.attachments.len()))
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
                t!("ui.attachment_viewer_hint").to_string(),
                Style::default().fg(app.color_scheme.muted)
            ));

            let modal_paragraph = Paragraph::new(modal_lines)
                .block(modal_block)
                .alignment(ratatui::layout::Alignment::Left);

            frame.render_widget(modal_paragraph, modal_area);
        } else {
            // Show error state - no attachments or no note
            let modal_block = Block::default()
                .title(format!(" {} ", t!("ui.attachments")))
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
    super::version_history::render_version_history(app, frame, size);

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
    if matches!(app.input_mode, InputMode::BulkRemoveTags) {
        render_input_modal(
            frame,
            size,
            &t!("bulk.enter_tags"),
            &t!("bulk.remove_tags_from", count = app.selected_note_ids.len()),
            &app.bulk_tags_input,
            &t!("help.enter_confirm_esc_cancel"),
            &app.color_scheme,
            60,
            6,
        );
    }

    if matches!(app.input_mode, InputMode::BulkSetColour) {
        render_input_modal(
            frame,
            size,
            &t!("bulk.enter_colour"),
            &t!("bulk.colour_of", count = app.selected_note_ids.len()),
            &app.bulk_tags_input,
            &t!("help.enter_confirm_esc_cancel"),
            &app.color_scheme,
            60,
            6,
        );
    }

    if matches!(app.input_mode, InputMode::BulkAddTags) {
        let count = app.selected_note_ids.len();
        let description = format!(
            "{} {} {}",
            t!("bulk.add_tags_to"),
            count,
            if count == 1 { t!("sync.note") } else { t!("sync.notes") }
        );
        render_input_modal(
            frame,
            size,
            &t!("bulk.enter_tags"),
            &description,
            &app.bulk_tags_input,
            &t!("help.enter_confirm_esc_cancel"),
            &app.color_scheme,
            60,
            6,
        );
    }

    if matches!(app.input_mode, InputMode::SaveSearchName) {
        render_input_modal(
            frame,
            size,
            &t!("saved_search.name_prompt"),
            &app.search_input,
            &app.saved_search_name_input,
            &t!("help.enter_confirm_esc_cancel"),
            &app.color_scheme,
            60,
            6,
        );
    }

    if let Some(path) = &app.export_overwrite_path {
        super::inbox::render_confirm_modal(
            frame,
            size,
            &t!("bulk.overwrite_title"),
            &t!("bulk.overwrite_message", path = path.display().to_string()),
        );
    }

    // Render bulk export path input modal if showing
    if matches!(app.input_mode, InputMode::BulkExportPath) {
        let count = app.selected_note_ids.len();
        let description = format!(
            "{} {} {}",
            t!("bulk.export_notes_to"),
            count,
            if count == 1 { t!("sync.note") } else { t!("sync.notes") }
        );
        render_input_modal(
            frame,
            size,
            &t!("bulk.enter_export_path"),
            &description,
            &app.bulk_export_path_input,
            &t!("help.enter_confirm_esc_cancel"),
            &app.color_scheme,
            70,
            6,
        );
    }

    // Render conflict resolution modal if showing
    if matches!(app.view_mode, ViewMode::ConflictResolution) {
        super::conflict::render_conflict_modal(app, frame, size);
    }

    // Render note info modal if showing
    super::note_info::render_note_info(app, frame, size);

    if matches!(app.view_mode, ViewMode::SavedSearches) {
        super::saved_searches::render_saved_searches(app, frame, size);
    }

    // Render inbox modal if showing
    if matches!(app.view_mode, ViewMode::Inbox) {
        super::inbox::render_inbox(app, frame, size);
    }

    // Store click detection areas in app (after all borrows of filtered are done)
    app.note_list_area = Some(note_list_inner_area);
    app.note_list_offset = list_offset;
    app.note_list_rows = rows;
    app.tags_line_area = tags_line_area_local;
    app.tag_positions = tag_positions_local;
    app.note_link_positions = note_link_positions_local;

    // Where the text really stopped, so the way back up starts from there
    if let Some(scroll) = preview_scroll {
        app.preview_scroll_offset = scroll;
    }
    app.preview_area = preview_area_local;
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    pub(crate) fn test_app() -> App {
        let dir = tempfile::tempdir().unwrap();
        let app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        app
    }

    pub(crate) fn screen(app: &mut App) -> String {
        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| render_note_list(app, frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| {
                (0..buffer.area.width)
                    .map(|x| buffer[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn app_showing(content: &str) -> App {
        let mut app = test_app();
        app.notes.push(Note::new(content.to_string()));
        app.invalidate_filter_cache();
        app
    }

    #[test]
    fn a_preview_stops_scrolling_at_the_end_of_its_text() {
        let mut app = app_showing(&(0..100).map(|i| format!("Line {i}")).collect::<Vec<_>>().join("\n"));
        app.preview_scroll_offset = 100_000;

        let shown = screen(&mut app);

        assert!(app.preview_scroll_offset < 100, "{}", app.preview_scroll_offset);
        assert!(shown.contains("Line 99"), "{shown}");
    }

    #[test]
    fn a_short_preview_does_not_scroll_at_all() {
        let mut app = app_showing("Short");
        app.preview_scroll_offset = 50;

        let shown = screen(&mut app);

        assert_eq!(app.preview_scroll_offset, 0);
        assert!(shown.contains("Short"), "{shown}");
    }

    #[test]
    fn a_note_with_word_wrap_off_runs_off_the_edge_of_the_preview() {
        let long = format!("{}END", "word ".repeat(100));

        let mut app = app_showing(&long);
        app.notes[0].set_syntax(SyntaxLanguage::Plain);
        assert!(screen(&mut app).contains("END"));

        app.notes[0].word_wrap = false;
        assert!(!screen(&mut app).contains("END"));
    }

    #[test]
    fn the_link_the_keyboard_is_on_stands_out() {
        let target = Note::new("Target".to_string());
        let mut app = app_showing(&format!("[other note](link:{})", target.id));
        app.notes.push(target);
        app.invalidate_filter_cache();
        app.selected_note = app.filtered_notes().iter().position(|note| note.content.contains("link:")).unwrap();

        let mut terminal = Terminal::new(TestBackend::new(120, 30)).unwrap();
        terminal.draw(|frame| render_note_list(&mut app, frame)).unwrap();
        let (_, y, x, _) = app.note_link_positions[0].clone();
        let reversed = |terminal: &Terminal<TestBackend>| {
            terminal.backend().buffer()[(x, y)].modifier.contains(Modifier::REVERSED)
        };
        assert!(!reversed(&terminal));

        app.selected_link = Some(0);
        terminal.draw(|frame| render_note_list(&mut app, frame)).unwrap();
        assert!(reversed(&terminal));
    }

    #[test]
    fn a_wiki_link_shows_as_the_note_it_points_at() {
        let target = Note::new("Project Plan".to_string());
        let mut app = app_showing("See [[project plan]] for more");
        app.notes.push(target.clone());
        app.invalidate_filter_cache();
        app.selected_note = app.filtered_notes().iter().position(|note| note.content.contains("[[")).unwrap();

        let shown = screen(&mut app);

        assert!(!shown.contains("[[project plan]]"), "{shown}");
        assert_eq!(app.note_link_positions[0].0, target.id);
    }

    #[test]
    fn tab_indented_text_keeps_its_columns_in_a_preview() {
        let mut app = app_showing("First\n\t\tIndented");
        app.notes[0].set_syntax(SyntaxLanguage::Plain);

        let shown = screen(&mut app);

        assert!(shown.contains("        Indented"), "{shown}");
    }

    /// `count` notes with titles Note 0, Note 1 and so on, in display order, and
    /// heights that vary because every other note has preview lines.
    fn app_with_notes(count: usize) -> App {
        let mut app = test_app();
        let now = chrono::Utc::now();
        for i in 0..count {
            let text = if i % 2 == 0 {
                format!("Note {i}\nA preview line\nAnother one")
            } else {
                format!("Note {i}")
            };
            let mut note = Note::new(text);
            note.modified_at = now - chrono::Duration::minutes(i as i64);
            app.notes.push(note);
        }
        app
    }

    /// The text on a row of the list area.
    fn row_text(drawn: &str, app: &App, row: u16) -> String {
        let area = app.note_list_area.unwrap();
        drawn.lines().nth((area.y + row) as usize).unwrap().to_string()
    }

    #[test]
    fn a_click_lands_on_the_note_drawn_at_that_row() {
        let mut app = app_with_notes(5);
        let drawn = screen(&mut app);

        for drawn_note in app.note_list_rows.clone() {
            for row in drawn_note.top..drawn_note.bottom {
                assert_eq!(app.note_at_row(row), Some(drawn_note.index));
            }
            let title = format!("Note {}", drawn_note.index);
            assert!(row_text(&drawn, &app, drawn_note.top).contains(&title));
        }

        // Five rows down is the second note, not the sixth
        assert_eq!(app.note_at_row(5), Some(1));
    }

    #[test]
    fn a_click_still_lands_correctly_after_scrolling() {
        let mut app = app_with_notes(40);
        app.selected_note = 30;

        let drawn = screen(&mut app);

        assert!(app.note_list_offset > 0);
        let first = app.note_list_rows[0];
        assert_eq!(first.index, app.note_list_offset);
        assert_eq!(app.note_at_row(0), Some(app.note_list_offset));
        assert!(row_text(&drawn, &app, first.top).contains(&format!("Note {}", first.index)));
    }

    #[test]
    fn a_click_below_the_last_note_lands_nowhere() {
        let mut app = app_with_notes(1);
        screen(&mut app);

        assert_eq!(app.note_at_row(20), None);
    }

    #[test]
    fn the_selected_note_is_always_on_screen() {
        let mut app = app_with_notes(40);

        for selected in [0, 5, 39, 38, 20, 0] {
            app.selected_note = selected;
            screen(&mut app);
            assert!(app.note_list_rows.iter().any(|row| row.index == selected), "note {selected}");
        }
    }

    #[test]
    fn moving_within_the_window_does_not_scroll() {
        let mut app = app_with_notes(40);
        app.selected_note = 30;
        screen(&mut app);
        let offset = app.note_list_offset;

        app.selected_note = offset + 1;
        screen(&mut app);

        assert_eq!(app.note_list_offset, offset);
    }

    #[test]
    fn only_the_visible_notes_are_built() {
        let mut app = app_with_notes(5000);
        app.selected_note = 4000;

        screen(&mut app);

        assert!(app.note_list_rows.len() < 20);
    }

    #[test]
    fn an_error_is_shown_in_the_status_bar() {
        let mut app = test_app();
        app.notes.push(Note::new("A note".to_string()));
        app.error = Some("The disk is full".to_string());

        assert!(screen(&mut app).contains("The disk is full"));
    }

    #[test]
    fn an_error_outranks_a_sync_status() {
        let mut app = test_app();
        app.error = Some("The disk is full".to_string());
        app.sync_status = Some("All synced".to_string());

        let drawn = screen(&mut app);
        assert!(drawn.contains("The disk is full"));
        assert!(!drawn.contains("All synced"));
    }

    #[test]
    fn a_note_with_a_short_or_multi_byte_id_draws() {
        let mut app = test_app();
        let mut short = Note::new("Short id".to_string());
        short.id = "x".to_string();
        let mut wide = Note::new("Wide id".to_string());
        wide.id = "日本語のノート".to_string();
        app.notes.push(short);
        app.notes.push(wide);

        let drawn = screen(&mut app);
        assert!(drawn.contains("Short id"));
    }
}
