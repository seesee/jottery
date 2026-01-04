//! Rendering for the help screen

use ratatui::Frame;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;

/// Helper struct for help screen entries with separate key bindings and descriptions
struct HelpEntry {
    key_binding: String,
    description: String,
}

impl HelpEntry {
    fn new(key_i18n: &str, desc_i18n: &str) -> Self {
        Self {
            key_binding: t!(key_i18n).to_string(),
            description: t!(desc_i18n).to_string(),
        }
    }
}

/// Render the help screen showing all keyboard shortcuts
pub fn render_help(app: &App, frame: &mut Frame) {
    let size = frame.area();

    let block = Block::default()
        .title(format!("{} - {}", t!("help.title"), t!("help.subtitle")))
        .borders(Borders::ALL)
        .style(Style::default().fg(app.color_scheme.accent));

    // Build sections with HelpEntry structs
    let sections: Vec<(&str, Vec<HelpEntry>)> = vec![
        (
            "help.unlock_screen",
            vec![
                HelpEntry::new("help.type_password_key", "help.type_password_desc"),
                HelpEntry::new("help.enter_to_unlock_key", "help.enter_to_unlock_desc"),
                HelpEntry::new("help.tab_to_switch_key", "help.tab_to_switch_desc"),
                HelpEntry::new("help.backspace_key", "help.backspace_desc"),
                HelpEntry::new("help.ctrl_q_esc_key", "help.ctrl_q_esc_desc"),
            ],
        ),
        (
            "help.note_list",
            vec![
                HelpEntry::new("help.slash_search_key", "help.slash_search_desc"),
                HelpEntry::new("help.y_sync_key", "help.y_sync_desc"),
                HelpEntry::new("help.s_settings_key", "help.s_settings_desc"),
                HelpEntry::new("help.n_new_key", "help.n_new_desc"),
                HelpEntry::new("help.i_enter_open_key", "help.i_enter_open_desc"),
                HelpEntry::new("help.d_delete_key", "help.d_delete_desc"),
                HelpEntry::new("help.j_down_key", "help.j_down_desc"),
                HelpEntry::new("help.k_up_key", "help.k_up_desc"),
                HelpEntry::new("help.question_help_key", "help.question_help_desc"),
                HelpEntry::new("help.ctrl_q_quit_key", "help.ctrl_q_quit_desc"),
            ],
        ),
        (
            "help.search_mode",
            vec![
                HelpEntry::new("help.type_search_key", "help.type_search_desc"),
                HelpEntry::new("help.search_tag_key", "help.search_tag_desc"),
                HelpEntry::new("help.search_exclude_key", "help.search_exclude_desc"),
                HelpEntry::new("help.search_and_key", "help.search_and_desc"),
                HelpEntry::new("help.enter_open_editor_key", "help.enter_open_editor_desc"),
                HelpEntry::new("help.esc_exit_search_key", "help.esc_exit_search_desc"),
                HelpEntry::new("help.arrows_navigate_key", "help.arrows_navigate_desc"),
            ],
        ),
        (
            "help.note_preview_view",
            vec![
                HelpEntry::new("help.enter_e_edit_key", "help.enter_e_edit_desc"),
                HelpEntry::new("help.t_tag_mode_key", "help.t_tag_mode_desc"),
                HelpEntry::new("help.question_help_key", "help.question_help_desc"),
                HelpEntry::new("help.q_esc_save_key", "help.q_esc_save_desc"),
            ],
        ),
        (
            "help.note_preview_tag",
            vec![
                HelpEntry::new("help.type_tag_key", "help.type_tag_desc"),
                HelpEntry::new("help.enter_add_tag_key", "help.enter_add_tag_desc"),
                HelpEntry::new("help.backspace_remove_key", "help.backspace_remove_desc"),
                HelpEntry::new("help.backspace_delete_key", "help.backspace_delete_desc"),
                HelpEntry::new("help.esc_normal_key", "help.esc_normal_desc"),
            ],
        ),
        (
            "help.settings",
            vec![
                HelpEntry::new("help.jk_navigate_key", "help.jk_navigate_desc"),
                HelpEntry::new("help.enter_edit_key", "help.enter_edit_desc"),
                HelpEntry::new("help.enter_save_key", "help.enter_save_desc"),
                HelpEntry::new("help.esc_cancel_key", "help.esc_cancel_desc"),
                HelpEntry::new("help.p_paste_key", "help.p_paste_desc"),
                HelpEntry::new("help.c_copy_key", "help.c_copy_desc"),
                HelpEntry::new("help.s_q_close_key", "help.s_q_close_desc"),
            ],
        ),
        (
            "help.attachments",
            vec![
                HelpEntry::new("help.A_add_attachment_key", "help.A_add_attachment_desc"),
                HelpEntry::new("help.a_view_attachment_key", "help.a_view_attachment_desc"),
                HelpEntry::new("help.shift_jk_attachment_key", "help.shift_jk_attachment_desc"),
                HelpEntry::new("help.X_remove_attachment_key", "help.X_remove_attachment_desc"),
                HelpEntry::new("help.note_images_key", "help.note_images_desc"),
            ],
        ),
        (
            "help.global",
            vec![
                HelpEntry::new("help.question_global_key", "help.question_global_desc"),
            ],
        ),
    ];

    // Calculate maximum key binding width across all sections
    let max_key_width = sections
        .iter()
        .flat_map(|(_, entries)| entries)
        .map(|entry| entry.key_binding.len())
        .max()
        .unwrap_or(0);

    // Build help text with dynamic spacing
    let mut help_text = Vec::new();

    for (section_title, entries) in sections {
        // Add section header
        help_text.push(Line::from(vec![
            Span::styled(
                t!(section_title),
                Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)
            ),
        ]));

        // Add entries with dynamic spacing
        for entry in entries {
            let spacing = " ".repeat(max_key_width - entry.key_binding.len() + 2);
            help_text.push(Line::from(format!(
                "  {}{}{}",
                entry.key_binding,
                spacing,
                entry.description
            )));
        }

        // Add blank line after section
        help_text.push(Line::from(""));
    }

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(paragraph, size);
}
