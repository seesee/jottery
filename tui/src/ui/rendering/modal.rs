//! Modal rendering helpers
//!
//! Provides common functionality for rendering centered modals and confirmation dialogs.

use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};

use crate::ui::ColorScheme;

/// Create a centered rectangle within the given area
pub fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = (area.width.saturating_sub(width)) / 2;
    let y = (area.height.saturating_sub(height)) / 2;
    Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    }
}

/// Render a confirmation modal with y/n/Esc prompt
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area (modal will be centered within it)
/// * `title` - The modal title
/// * `message_lines` - Lines of text to display (excluding the y/n prompt)
/// * `confirm_color` - The color for the confirm key highlight (e.g., accent or error)
/// * `color_scheme` - The app's color scheme
/// * `width` - Modal width
/// * `height` - Modal height (should include room for message lines + prompt)
#[allow(clippy::too_many_arguments)]
pub fn render_confirmation_modal(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    message_lines: Vec<Line<'_>>,
    confirm_color: ratatui::style::Color,
    color_scheme: &ColorScheme,
    width: u16,
    height: u16,
) {
    let modal_area = centered_rect(width, height, area);

    // Clear background
    frame.render_widget(Clear, modal_area);

    // Create modal block
    let modal_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(confirm_color))
        .style(Style::default().bg(color_scheme.background));

    // Build content with y/n/Esc prompt
    let mut lines = vec![Line::from("")];
    lines.extend(message_lines);
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::raw("Press "),
        Span::styled(
            "y",
            Style::default()
                .fg(confirm_color)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to confirm, "),
        Span::styled(
            "n",
            Style::default()
                .fg(color_scheme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" or "),
        Span::styled(
            "Esc",
            Style::default()
                .fg(color_scheme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(" to cancel"),
    ]));

    let modal_paragraph = Paragraph::new(lines)
        .block(modal_block)
        .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(modal_paragraph, modal_area);
}

/// Render a basic modal (just clears and provides a block)
///
/// Returns the modal area and inner area for custom content rendering.
///
/// # Arguments
/// * `frame` - The frame to render to
/// * `area` - The full screen area (modal will be centered within it)
/// * `title` - The modal title
/// * `border_color` - The border color
/// * `color_scheme` - The app's color scheme
/// * `width` - Modal width
/// * `height` - Modal height
#[allow(dead_code)]
pub fn render_modal_frame(
    frame: &mut Frame,
    area: Rect,
    title: &str,
    border_color: ratatui::style::Color,
    color_scheme: &ColorScheme,
    width: u16,
    height: u16,
) -> Rect {
    let modal_area = centered_rect(width, height, area);

    // Clear background
    frame.render_widget(Clear, modal_area);

    // Create and render modal block
    let modal_block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .style(Style::default().bg(color_scheme.background));

    let inner = modal_block.inner(modal_area);
    frame.render_widget(modal_block, modal_area);

    inner
}
