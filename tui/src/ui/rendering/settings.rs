//! Rendering for the settings screen

use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::state::InputMode;

/// Render settings screen
pub fn render_settings(app: &App, frame: &mut Frame) {
    let size = frame.area();

    let mode_text = match app.input_mode {
        InputMode::SettingsEdit => " [EDIT]",
        InputMode::PasswordVerify => " [ENTER PASSWORD]",
        _ => "",
    };

    let block = Block::default()
        .title(format!("Settings{} - ↑/↓: navigate | Enter/i: edit | s/q: close", mode_text))
        .borders(Borders::ALL)
        .style(Style::default().fg(app.color_scheme.success));

    // Helper to create field line with selection indicator
    let field_line = |index: usize, label: String, value: String| -> Line {
        let selected = index == app.selected_setting;
        let editing = selected && matches!(app.input_mode, InputMode::SettingsEdit);
        let password_verify = selected && matches!(app.input_mode, InputMode::PasswordVerify);

        let display_value = if password_verify && index == 7 {
            // Show masked password input for remember password verification
            format!("{}_{}", "*".repeat(app.setting_input.len()), if app.setting_input.is_empty() { " (enter password)" } else { "" })
        } else if editing && (index == 0 || index == 3 || index == 5 || index == 6) {
            // Show input buffer for editable fields (language, auto-lock, sync endpoint, auto-sync interval)
            format!("{}_", app.setting_input)
        } else {
            value
        };

        let prefix = if selected { "→ " } else { "  " };
        let label_style = if selected {
            Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        let value_style = if editing || password_verify {
            Style::default().fg(app.color_scheme.success).add_modifier(Modifier::BOLD)
        } else if selected {
            Style::default().fg(app.color_scheme.accent).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(app.color_scheme.accent)
        };

        Line::from(vec![
            Span::styled(prefix, label_style),
            Span::styled(label, label_style),
            Span::styled(display_value, value_style),
        ])
    };

    let settings_text = vec![
        Line::from(vec![
            Span::styled(t!("settings.title"), Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        field_line(0, format!("{}:              ", t!("settings.language")), app.settings.language.clone()),
        field_line(1, format!("{}:          ", t!("settings.color_scheme")), format!("{} ({})", app.settings.theme, t!("settings.press_enter_cycle"))),
        field_line(2, format!("{}:            ", t!("menu.search")), format!("{} ({})", app.settings.sort_order, t!("settings.press_enter_cycle"))),
        field_line(3, format!("{}:     ", t!("settings.auto_lock")), format!("{} {}", app.settings.auto_lock_timeout, t!("settings.minutes"))),
        Line::from(""),
        Line::from(vec![
            Span::styled(t!("settings.sync"), Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        field_line(4, format!("{}:          ", t!("settings.sync_enabled")), format!("{} ({})", if app.settings.sync_enabled { t!("common.yes") } else { t!("common.no") }, t!("settings.press_enter_toggle"))),
        field_line(5, format!("{}:         ", t!("settings.sync_endpoint")), app.settings.sync_endpoint.clone().unwrap_or_else(|| t!("sync.no_api_key").to_string())),
        field_line(6, format!("{}:    ", t!("settings.auto_sync_interval")), format!("{} {} (0 = {})", app.settings.auto_sync_interval_minutes, t!("settings.minutes"), t!("settings.disabled"))),
        Line::from(""),
        Line::from(vec![
            Span::styled(t!("help.general"), Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        field_line(7, format!("{}:     ", t!("password.remember")), if app.settings.remember_password {
            format!("{} ({})", t!("common.yes"), t!("settings.press_enter_toggle"))
        } else {
            format!("{} ({})", t!("common.no"), t!("settings.press_enter_toggle"))
        }),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{}: ", t!("settings.instructions")), Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!("  • {}", t!("settings.instructions_navigation"))),
        Line::from(format!("  • {}", t!("settings.instructions_edit"))),
        Line::from(format!("  • {}", t!("settings.instructions_text"))),
        Line::from(format!("  • {}", t!("settings.instructions_toggle"))),
        Line::from(format!("  • {}", t!("settings.instructions_remember"))),
        Line::from(format!("  • {}", t!("settings.instructions_sync"))),
        Line::from(format!("  • {}", t!("settings.instructions_force_sync"))),
        Line::from(""),
        Line::from(vec![
            Span::styled(format!("{}: ", t!("settings.sync")), Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!("  • {}", t!("settings.instructions_paste"))),
        Line::from(format!("  • {}", t!("settings.instructions_copy"))),
        Line::from(format!("  • {}", t!("settings.instructions_disconnect"))),
    ];

    // Add status and error messages if present
    let mut all_lines = settings_text;

    // Show user portal link if sync endpoint is configured
    if let Some(endpoint) = &app.settings.sync_endpoint {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", t!("settings.user_portal")), Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}/user", endpoint), Style::default().fg(app.color_scheme.accent)),
        ]));
    }
    if let Some(status) = &app.sync_status {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", t!("status.synced")), Style::default().fg(app.color_scheme.success).add_modifier(Modifier::BOLD)),
            Span::styled(status.clone(), Style::default().fg(app.color_scheme.success)),
        ]));
    }
    if let Some(err) = &app.error {
        all_lines.push(Line::from(""));
        all_lines.push(Line::from(vec![
            Span::styled(format!("{}: ", t!("error.generic")), Style::default().fg(app.color_scheme.error).add_modifier(Modifier::BOLD)),
            Span::styled(err.clone(), Style::default().fg(app.color_scheme.error)),
        ]));
    }

    let paragraph = Paragraph::new(all_lines)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, size);

    // Show cursor when editing text fields
    if matches!(app.input_mode, InputMode::SettingsEdit) && (app.selected_setting == 0 || app.selected_setting == 3 || app.selected_setting == 5 || app.selected_setting == 6) {
        // Calculate cursor position based on selected field
        let line_offset = match app.selected_setting {
            0 => 2,  // Language is on line 2
            3 => 5,  // Auto-lock timeout is on line 5
            5 => 10, // Sync endpoint is on line 10
            6 => 11, // Auto-sync interval is on line 11
            _ => 0,
        };

        let cursor_x = 26 + app.setting_input.len() as u16; // After label
        let cursor_y = line_offset + 1; // +1 for border

        frame.set_cursor_position((cursor_x, cursor_y));
    }
}
