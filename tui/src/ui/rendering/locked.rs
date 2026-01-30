//! Rendering for the locked (password entry) screen

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Direction, Layout};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use rust_i18n::t;

use crate::password_storage::StorageBackendType;
use crate::ui::app::App;

/// Render the locked screen with password input
pub fn render_locked(app: &App, frame: &mut Frame) {
    let size = frame.area();

    let title = if app.is_new_database {
        format!("Jottery TUI v{} - {}", env!("CARGO_PKG_VERSION"), t!("password.create_title"))
    } else {
        format!("Jottery TUI v{} - {}", env!("CARGO_PKG_VERSION"), t!("password.unlock_title"))
    };

    let block = Block::default()
        .title(title)
        .borders(Borders::ALL);

    let constraints = if app.is_new_database {
        vec![
            Constraint::Length(3),  // Password field
            Constraint::Length(3),  // Confirm field
            Constraint::Length(2),  // Help text
            Constraint::Length(3),  // Error (if any)
            Constraint::Min(0),     // Remaining space
        ]
    } else {
        vec![
            Constraint::Length(3),  // Password field
            Constraint::Length(2),  // Remember password checkbox
            Constraint::Length(3),  // Help text
            Constraint::Length(3),  // Error (if any)
            Constraint::Min(0),     // Remaining space
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(2)
        .constraints(constraints)
        .split(size);

    frame.render_widget(block, size);

    // Password field
    let password_style = if app.is_new_database && !app.password_confirm_focused {
        Style::default().fg(app.color_scheme.accent)
    } else if !app.is_new_database {
        Style::default().fg(app.color_scheme.accent)
    } else {
        Style::default()
    };

    let password_text = "*".repeat(app.password_input.len());
    let password = Paragraph::new(password_text)
        .style(password_style)
        .block(Block::default().title(t!("password.enter")).borders(Borders::ALL));
    frame.render_widget(password, chunks[0]);

    if app.is_new_database {
        // Confirm field
        let confirm_style = if app.password_confirm_focused {
            Style::default().fg(app.color_scheme.accent)
        } else {
            Style::default()
        };

        let confirm_text = "*".repeat(app.password_confirm.len());
        let confirm = Paragraph::new(confirm_text)
            .style(confirm_style)
            .block(Block::default().title(t!("password.confirm")).borders(Borders::ALL));
        frame.render_widget(confirm, chunks[1]);

        // Help text
        let help = Paragraph::new(t!("help.tab_switch"))
            .style(Style::default().fg(app.color_scheme.muted))
            .alignment(Alignment::Center);
        frame.render_widget(help, chunks[2]);

        // Cursor position
        if app.password_confirm_focused {
            frame.set_cursor_position((
                chunks[1].x + app.password_confirm.len() as u16 + 1,
                chunks[1].y + 1,
            ));
        } else {
            frame.set_cursor_position((
                chunks[0].x + app.password_input.len() as u16 + 1,
                chunks[0].y + 1,
            ));
        }

        // Error (if any)
        if let Some(err) = &app.error {
            let error = Paragraph::new(err.clone())
                .style(Style::default().fg(app.color_scheme.error))
                .block(Block::default().title(t!("error.generic")).borders(Borders::ALL));
            frame.render_widget(error, chunks[3]);
        }
    } else {
        // Show cursor at end of password input
        frame.set_cursor_position((
            chunks[0].x + app.password_input.len() as u16 + 1,
            chunks[0].y + 1,
        ));

        // Remember password checkbox
        let checkbox_text = if app.remember_password_checkbox {
            format!("[X] {}", t!("password.remember_toggle"))
        } else {
            format!("[ ] {}", t!("password.remember_toggle"))
        };
        let checkbox = Paragraph::new(checkbox_text)
            .style(if app.remember_password_checkbox {
                Style::default().fg(app.color_scheme.accent)
            } else {
                Style::default().fg(app.color_scheme.muted)
            })
            .alignment(Alignment::Center);
        frame.render_widget(checkbox, chunks[1]);

        // Help text with security warning based on storage backend
        let (help_text, help_style) = if app.remember_password_checkbox {
            // Show different warning based on storage backend
            match app.password_storage.backend_type() {
                StorageBackendType::Keychain => (
                    t!("password.remember_keychain_warning"),
                    Style::default().fg(app.color_scheme.success),
                ),
                StorageBackendType::File => (
                    t!("password.remember_warning"),
                    Style::default().fg(app.color_scheme.error),
                ),
            }
        } else {
            (
                t!("help.enter_unlock"),
                Style::default().fg(app.color_scheme.muted),
            )
        };
        let help = Paragraph::new(help_text)
            .style(help_style)
            .alignment(Alignment::Center);
        frame.render_widget(help, chunks[2]);

        // Error (if any)
        if let Some(err) = &app.error {
            let error = Paragraph::new(err.clone())
                .style(Style::default().fg(app.color_scheme.error))
                .block(Block::default().title(t!("error.generic")).borders(Borders::ALL));
            frame.render_widget(error, chunks[3]);
        }
    }
}
