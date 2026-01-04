//! Rendering for sync credentials screens

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;

/// Render sync credentials display modal
pub fn render_show_credentials(app: &App, frame: &mut Frame, credentials: &str) {
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
        Block::default().style(Style::default().bg(app.color_scheme.background)),
        modal_area,
    );

    // Create border block
    let block = Block::default()
        .title(format!(" {} ", t!("sync.credentials_title")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.title));

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
            t!("sync.credentials_instruction"),
            Style::default().fg(app.color_scheme.accent),
        )),
        Line::from(""),
        Line::from(Span::raw(credentials)),
    ];

    let paragraph = Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .style(Style::default().fg(app.color_scheme.foreground));

    frame.render_widget(paragraph, chunks[0]);

    // Render help text
    let help = Paragraph::new(t!("help.press_esc_or_enter"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.muted));

    frame.render_widget(help, chunks[1]);
}

/// Render sync credentials input modal
pub fn render_input_credentials(app: &App, frame: &mut Frame) {
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
        Block::default().style(Style::default().bg(app.color_scheme.background)),
        modal_area,
    );

    // Create border block
    let block = Block::default()
        .title(format!(" {} ", t!("sync.paste_title")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.title));

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
        t!("sync.paste_instruction"),
        Style::default().fg(app.color_scheme.accent),
    )));
    frame.render_widget(instruction, chunks[0]);

    // Render input field
    let input = Paragraph::new(Line::from(vec![
        Span::raw(&app.credential_input),
        Span::styled("█", Style::default().fg(app.color_scheme.title)), // Cursor
    ]))
    .wrap(Wrap { trim: false })
    .style(Style::default().fg(app.color_scheme.foreground));

    frame.render_widget(input, chunks[1]);

    // Render help text
    let help = Paragraph::new(t!("help.press_enter_paste"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.muted));

    frame.render_widget(help, chunks[2]);
}
