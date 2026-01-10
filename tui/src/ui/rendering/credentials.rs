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

/// Render email input for registration status check
pub fn render_input_email_for_status(app: &App, frame: &mut Frame) {
    let size = frame.area();

    // Create centered modal (60% width, 30% height)
    let modal_width = (size.width as f32 * 0.6) as u16;
    let modal_height = (size.height as f32 * 0.3) as u16;
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
        .title(format!(" {} ", t!("sync.status.check_title")))
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
        t!("sync.status.enter_email"),
        Style::default().fg(app.color_scheme.accent),
    )));
    frame.render_widget(instruction, chunks[0]);

    // Render input field
    let input = Paragraph::new(Line::from(vec![
        Span::raw(&app.credential_input),
        Span::styled("█", Style::default().fg(app.color_scheme.title)), // Cursor
    ]))
    .style(Style::default().fg(app.color_scheme.foreground));

    frame.render_widget(input, chunks[1]);

    // Render help text
    let help = Paragraph::new(t!("help.press_enter_paste"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.muted));

    frame.render_widget(help, chunks[2]);
}

/// Render registration status result
pub fn render_registration_status(app: &App, frame: &mut Frame, status_message: &str) {
    let size = frame.area();

    // Create centered modal (50% width, 30% height)
    let modal_width = (size.width as f32 * 0.5) as u16;
    let modal_height = (size.height as f32 * 0.3) as u16;
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
        .title(format!(" {} ", t!("sync.status.result_title")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.title));

    // Split into content area and help area
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(block.inner(modal_area));

    // Render border
    frame.render_widget(block, modal_area);

    // Render status message
    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            status_message,
            Style::default().fg(app.color_scheme.accent),
        )),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: false })
    .style(Style::default().fg(app.color_scheme.foreground));

    frame.render_widget(text, chunks[0]);

    // Render help text
    let help = Paragraph::new(t!("help.press_esc_or_enter"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.muted));

    frame.render_widget(help, chunks[1]);
}

/// Render registration endpoint input
pub fn render_register_input_endpoint(app: &App, frame: &mut Frame) {
    render_registration_input_modal(
        app,
        frame,
        &t!("register.title"),
        &t!("register.enter_endpoint"),
        &app.credential_input,
        false,
    );
}

/// Render registration email input
pub fn render_register_input_email(app: &App, frame: &mut Frame) {
    render_registration_input_modal(
        app,
        frame,
        &t!("register.title"),
        &t!("register.enter_email"),
        &app.credential_input,
        false,
    );
}

/// Render registration password input
pub fn render_register_input_password(app: &App, frame: &mut Frame) {
    // Check if this is a re-entry after approval (pending registration exists)
    let is_reentry = app.db.as_ref().and_then(|db| {
        let sync_repo = crate::repository::sync::SyncRepository::new(db.connection());
        sync_repo.get_metadata().ok().flatten()
    }).map(|m| m.pending_registration_email.is_some()).unwrap_or(false);

    let prompt = if is_reentry {
        t!("register.reenter_password")
    } else {
        t!("register.enter_password")
    };

    render_registration_input_modal(
        app,
        frame,
        &t!("register.title"),
        &prompt,
        &app.credential_input,
        true,
    );
}

/// Render registration pending approval screen
pub fn render_register_pending_approval(app: &App, frame: &mut Frame, email: &str) {
    let size = frame.area();

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

    frame.render_widget(
        Block::default().style(Style::default().bg(app.color_scheme.background)),
        modal_area,
    );

    let block = Block::default()
        .title(format!(" {} ", t!("register.pending_title")))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.title));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(block.inner(modal_area));

    frame.render_widget(block, modal_area);

    let text = Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            t!("register.pending_message"),
            Style::default().fg(app.color_scheme.accent),
        )),
        Line::from(""),
        Line::from(Span::styled(
            format!("{}: {}", t!("common.email"), email),
            Style::default().fg(app.color_scheme.foreground),
        )),
        Line::from(""),
        Line::from(Span::styled(
            t!("register.pending_instructions"),
            Style::default().fg(app.color_scheme.muted),
        )),
    ])
    .alignment(Alignment::Center)
    .wrap(Wrap { trim: false });

    frame.render_widget(text, chunks[0]);

    let help = Paragraph::new(t!("register.pending_help"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.muted));

    frame.render_widget(help, chunks[1]);
}

/// Render registration device name input
pub fn render_register_input_device_name(app: &App, frame: &mut Frame) {
    render_registration_input_modal(
        app,
        frame,
        &t!("register.device_title"),
        &t!("register.enter_device_name"),
        &app.credential_input,
        false,
    );
}

/// Helper to render a registration input modal
fn render_registration_input_modal(
    app: &App,
    frame: &mut Frame,
    title: &str,
    instruction: &str,
    input: &str,
    is_password: bool,
) {
    let size = frame.area();

    let modal_width = (size.width as f32 * 0.6) as u16;
    let modal_height = (size.height as f32 * 0.35) as u16;
    let modal_x = (size.width - modal_width) / 2;
    let modal_y = (size.height - modal_height) / 2;

    let modal_area = ratatui::layout::Rect {
        x: modal_x,
        y: modal_y,
        width: modal_width,
        height: modal_height,
    };

    frame.render_widget(
        Block::default().style(Style::default().bg(app.color_scheme.background)),
        modal_area,
    );

    let block = Block::default()
        .title(format!(" {} ", title))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(app.color_scheme.title));

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(block.inner(modal_area));

    frame.render_widget(block, modal_area);

    let instruction_widget = Paragraph::new(Line::from(Span::styled(
        instruction,
        Style::default().fg(app.color_scheme.accent),
    )));
    frame.render_widget(instruction_widget, chunks[0]);

    let display_value = if is_password {
        format!("{}█", "*".repeat(input.len()))
    } else {
        format!("{}█", input)
    };

    let input_widget = Paragraph::new(Line::from(vec![
        Span::styled(display_value, Style::default().fg(app.color_scheme.foreground)),
    ]))
    .style(Style::default().fg(app.color_scheme.foreground));

    frame.render_widget(input_widget, chunks[1]);

    let help = Paragraph::new(t!("help.press_enter_paste"))
        .alignment(Alignment::Center)
        .style(Style::default().fg(app.color_scheme.muted));

    frame.render_widget(help, chunks[2]);
}
