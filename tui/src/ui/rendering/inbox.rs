//! Inbox rendering — displays inbox items in a modal overlay

use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
};
use rust_i18n::t;

use crate::ui::app::App;
use crate::models::get_note_title;
use super::modal::centered_rect_percent;

/// Render the inbox modal overlay
pub fn render_inbox(app: &mut App, frame: &mut Frame, area: Rect) {
    // Centre the modal — use 80% of the area
    let modal_area = centered_rect_percent(80, 90, area);

    // Clear the background
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(format!(" {} ({}) ", t!("inbox.title"), app.inbox_items.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if app.inbox_items.is_empty() {
        let empty = Paragraph::new(t!("inbox.empty").to_string())
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, inner);
        return;
    }

    // Split inner area: list on left, preview on right
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(inner);

    // Render item list
    let items: Vec<ListItem> = app.inbox_items.iter().enumerate().map(|(i, item)| {
        // Extract title using custom t: tag or first non-empty line
        let title_full = get_note_title(&item.content, &item.tags);
        let title: String = title_full.chars().take(40).collect();
        let tags_str = if item.tags.is_empty() {
            String::new()
        } else {
            format!(" [{}]", item.tags.join(", "))
        };
        let source_str = item.source.as_deref().map(|s| format!(" ({})", s)).unwrap_or_default();

        let style = if i == app.selected_inbox_item {
            Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        ListItem::new(vec![
            Line::from(vec![
                Span::styled(title, style),
            ]),
            Line::from(vec![
                Span::styled(
                    format!("  {}{}{}", &item.created_at[..10], tags_str, source_str),
                    Style::default().fg(Color::DarkGray),
                ),
            ]),
        ])
    }).collect();

    let list = List::new(items)
        .block(Block::default().borders(Borders::RIGHT));
    frame.render_widget(list, chunks[0]);

    // Render preview of selected item
    if let Some(item) = app.inbox_items.get(app.selected_inbox_item) {
        let mut lines = Vec::new();

        // Tags
        if !item.tags.is_empty() {
            let tag_spans: Vec<Span> = item.tags.iter().map(|tag| {
                Span::styled(format!(" #{} ", tag), Style::default().fg(Color::Yellow))
            }).collect();
            lines.push(Line::from(tag_spans));
            lines.push(Line::from(""));
        }

        // Source
        if let Some(ref source) = item.source {
            lines.push(Line::from(vec![
                Span::styled("Source: ", Style::default().fg(Color::DarkGray)),
                Span::styled(source.clone(), Style::default().fg(Color::Cyan)),
            ]));
            lines.push(Line::from(""));
        }

        // Content (plain text only — never rendered as HTML/Markdown for security)
        for content_line in item.content.lines() {
            lines.push(Line::from(content_line.to_string()));
        }

        let preview = Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::default()
                .title(" Preview ")
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::DarkGray)));
        frame.render_widget(preview, chunks[1]);
    }

    // Render confirmation modals
    if app.show_inbox_delete_confirm {
        render_confirm_modal(
            frame,
            area,
            &t!("inbox.confirm_delete_title"),
            &t!("inbox.confirm_delete_message"),
        );
    }

    if app.show_inbox_delete_all_confirm {
        render_confirm_modal(
            frame,
            area,
            &t!("inbox.confirm_delete_all_title"),
            &t!("inbox.confirm_delete_all_message", count = app.inbox_items.len()),
        );
    }
}

fn render_confirm_modal(frame: &mut Frame, area: Rect, title: &str, message: &str) {
    let modal = centered_rect_percent(50, 30, area);
    frame.render_widget(Clear, modal);

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red));
    let inner = block.inner(modal);
    frame.render_widget(block, modal);

    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(message.to_string()),
        Line::from(""),
        Line::from(vec![
            Span::styled("y", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(": confirm | "),
            Span::styled("n/Esc", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(": cancel"),
        ]),
    ])
    .wrap(Wrap { trim: true });
    frame.render_widget(text, inner);
}
