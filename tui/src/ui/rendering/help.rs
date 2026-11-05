//! Rendering for the help screen

use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::state::AppState;

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
pub fn render_help(app: &mut App, frame: &mut Frame) {
    let size = frame.area();

    // Get current scroll offset
    let scroll_offset = if let AppState::Help { scroll_offset, .. } = &app.state {
        *scroll_offset
    } else {
        0
    };

    let block = Block::default()
        .title(format!("{} - {} (↑↓/j/k to scroll, PgUp/PgDown, q/? to close)", t!("help.title"), t!("help.subtitle")))
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
                HelpEntry::new("help.p_pin_key", "help.p_pin_desc"),
                HelpEntry::new("help.r_recycle_key", "help.r_recycle_desc"),
                HelpEntry::new("help.z_archive_key", "help.z_archive_desc"),
                HelpEntry::new("help.shift_a_archive_key", "help.shift_a_archive_desc"),
                HelpEntry::new("help.L_lock_key", "help.L_lock_desc"),
                HelpEntry::new("help.ctrl_l_lock_key", "help.ctrl_l_lock_desc"),
                HelpEntry::new("help.I_info_key", "help.I_info_desc"),
                HelpEntry::new("help.v_versions_key", "help.v_versions_desc"),
                HelpEntry::new("help.b_inbox_key", "help.b_inbox_desc"),
                HelpEntry::new("help.g_saved_key", "help.g_saved_desc"),
                HelpEntry::new("help.D_duplicate_key", "help.D_duplicate_desc"),
                HelpEntry::new("help.o_copy_key", "help.o_copy_desc"),
                HelpEntry::new("help.w_wrap_key", "help.w_wrap_desc"),
                HelpEntry::new("help.l_link_key", "help.l_link_desc"),
                HelpEntry::new("help.bracket_cycle_key", "help.bracket_cycle_desc"),
                HelpEntry::new("help.color_cycle_key", "help.color_cycle_desc"),
                HelpEntry::new("help.j_down_key", "help.j_down_desc"),
                HelpEntry::new("help.k_up_key", "help.k_up_desc"),
                HelpEntry::new("help.pageup_pagedown_key", "help.pageup_pagedown_desc"),
                HelpEntry::new("help.ctrl_scroll_key", "help.ctrl_scroll_desc"),
                HelpEntry::new("help.mouse_scroll_key", "help.mouse_scroll_desc"),
                HelpEntry::new("help.question_help_key", "help.question_help_desc"),
                HelpEntry::new("help.q_quit_key", "help.q_quit_desc"),
            ],
        ),
        (
            "help.search_mode",
            vec![
                HelpEntry::new("help.type_search_key", "help.type_search_desc"),
                HelpEntry::new("help.search_tag_key", "help.search_tag_desc"),
                HelpEntry::new("help.search_exclude_key", "help.search_exclude_desc"),
                HelpEntry::new("help.search_and_key", "help.search_and_desc"),
                HelpEntry::new("help.search_or_key", "help.search_or_desc"),
                HelpEntry::new("help.search_phrase_key", "help.search_phrase_desc"),
                HelpEntry::new("help.search_wildcard_key", "help.search_wildcard_desc"),
                HelpEntry::new("help.search_exclude_tag_key", "help.search_exclude_tag_desc"),
                HelpEntry::new("help.search_attachment_key", "help.search_attachment_desc"),
                HelpEntry::new("help.search_archived_key", "help.search_archived_desc"),
                HelpEntry::new("help.search_created_key", "help.search_created_desc"),
                HelpEntry::new("help.search_modified_key", "help.search_modified_desc"),
                HelpEntry::new("help.search_words_key", "help.search_words_desc"),
                HelpEntry::new("help.search_color_key", "help.search_color_desc"),
                HelpEntry::new("help.search_color_note_key", "help.search_color_note_desc"),
                HelpEntry::new("help.search_color_tag_key", "help.search_color_tag_desc"),
                HelpEntry::new("help.search_category_key", "help.search_category_desc"),
                HelpEntry::new("help.enter_open_editor_key", "help.enter_open_editor_desc"),
                HelpEntry::new("help.esc_exit_search_key", "help.esc_exit_search_desc"),
                HelpEntry::new("help.arrows_navigate_key", "help.arrows_navigate_desc"),
                HelpEntry::new("help.tab_search_key", "help.tab_search_desc"),
                HelpEntry::new("help.ctrl_s_save_search_key", "help.ctrl_s_save_search_desc"),
                HelpEntry::new("help.ctrl_p_history_key", "help.ctrl_p_history_desc"),
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
                HelpEntry::new("help.s_q_close_key", "help.s_q_close_desc"),
            ],
        ),
        (
            "help.attachments",
            vec![
                HelpEntry::new("help.tab_focus_attachment_key", "help.tab_focus_attachment_desc"),
                HelpEntry::new("help.plus_add_attachment_key", "help.plus_add_attachment_desc"),
                HelpEntry::new("help.S_save_attachment_key", "help.S_save_attachment_desc"),
                HelpEntry::new("help.a_view_attachment_key", "help.a_view_attachment_desc"),
                HelpEntry::new("help.shift_jk_attachment_key", "help.shift_jk_attachment_desc"),
                HelpEntry::new("help.X_remove_attachment_key", "help.X_remove_attachment_desc"),
                HelpEntry::new("help.note_images_key", "help.note_images_desc"),
            ],
        ),
        (
            "help.multi_select",
            vec![
                HelpEntry::new("help.space_select_key", "help.space_select_desc"),
                HelpEntry::new("help.shift_v_range_key", "help.shift_v_range_desc"),
                HelpEntry::new("help.ctrl_a_all_key", "help.ctrl_a_all_desc"),
                HelpEntry::new("help.multi_t_tags_key", "help.multi_t_tags_desc"),
                HelpEntry::new("help.multi_remove_tags_key", "help.multi_remove_tags_desc"),
                HelpEntry::new("help.multi_colour_key", "help.multi_colour_desc"),
                HelpEntry::new("help.multi_d_delete_key", "help.multi_d_delete_desc"),
                HelpEntry::new("help.multi_z_archive_key", "help.multi_z_archive_desc"),
                HelpEntry::new("help.multi_c_combine_key", "help.multi_c_combine_desc"),
                HelpEntry::new("help.multi_e_export_key", "help.multi_e_export_desc"),
                HelpEntry::new("help.esc_clear_key", "help.esc_clear_desc"),
            ],
        ),
        (
            "help.mouse",
            vec![
                HelpEntry::new("help.click_note_key", "help.click_note_desc"),
                HelpEntry::new("help.click_tag_key", "help.click_tag_desc"),
                HelpEntry::new("help.click_tags_key", "help.click_tags_desc"),
                HelpEntry::new("help.scroll_key", "help.scroll_desc"),
            ],
        ),
        (
            "help.global",
            vec![
                HelpEntry::new("help.question_global_key", "help.question_global_desc"),
            ],
        ),
    ];

    // Determine if we should use 2-column layout (terminal width > 100)
    let use_two_columns = size.width > 100;

    // Calculate maximum key binding width across all sections
    let max_key_width = sections
        .iter()
        .flat_map(|(_, entries)| entries)
        .map(|entry| entry.key_binding.len())
        .max()
        .unwrap_or(0);

    if use_two_columns {
        // Split sections into two columns
        let mid = sections.len().div_ceil(2);
        let (left_sections, right_sections) = sections.split_at(mid);

        // Create horizontal layout
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
            .split(size);

        // Build left column text
        let left_text = build_column_text(left_sections, max_key_width, app);

        // Build right column text
        let right_text = build_column_text(right_sections, max_key_width, app);

        // Render both columns
        let left_block = Block::default()
            .title(format!("{} - {}", t!("help.title"), t!("help.subtitle")))
            .borders(Borders::ALL)
            .style(Style::default().fg(app.color_scheme.accent));

        let right_block = Block::default()
            .borders(Borders::ALL)
            .style(Style::default().fg(app.color_scheme.accent));

        let left_paragraph = Paragraph::new(left_text)
            .block(left_block)
            .wrap(Wrap { trim: false });

        let right_paragraph = Paragraph::new(right_text)
            .block(right_block)
            .wrap(Wrap { trim: false });

        // They scroll together, as far as the longer one needs
        let furthest = super::furthest_scroll(&left_paragraph, chunks[0]).max(super::furthest_scroll(&right_paragraph, chunks[1]));
        let offset = scroll_offset.min(furthest);
        keep_help_scroll(app, offset);

        frame.render_widget(left_paragraph.scroll((super::scroll_rows(offset), 0)), chunks[0]);
        frame.render_widget(right_paragraph.scroll((super::scroll_rows(offset), 0)), chunks[1]);
    } else {
        // Single column layout with scrolling
        let help_text = build_column_text(&sections, max_key_width, app);

        let paragraph = Paragraph::new(help_text)
            .block(block)
            .wrap(Wrap { trim: false });

        let offset = scroll_offset.min(super::furthest_scroll(&paragraph, size));
        keep_help_scroll(app, offset);

        frame.render_widget(paragraph.scroll((super::scroll_rows(offset), 0)), size);
    }
}

/// Remember where the text really stopped, so the way back up starts from there.
fn keep_help_scroll(app: &mut App, offset: usize) {
    if let AppState::Help { scroll_offset, .. } = &mut app.state {
        *scroll_offset = offset;
    }
}

/// Build text for a column of help sections
fn build_column_text(
    sections: &[(&str, Vec<HelpEntry>)],
    max_key_width: usize,
    app: &App,
) -> Vec<Line<'static>> {
    let mut all_lines = Vec::new();

    for (section_title, entries) in sections {
        // Add section header (convert to owned string)
        all_lines.push(Line::from(vec![
            Span::styled(
                t!(*section_title).to_string(),
                Style::default().fg(app.color_scheme.title).add_modifier(Modifier::BOLD)
            ),
        ]));

        // Add entries with dynamic spacing (already owned via format!)
        for entry in entries {
            let spacing = " ".repeat(max_key_width - entry.key_binding.len() + 2);
            all_lines.push(Line::from(format!(
                "  {}{}{}",
                entry.key_binding,
                spacing,
                entry.description
            )));
        }

        // Add blank line after section (convert to owned)
        all_lines.push(Line::from("".to_string()));
    }

    all_lines
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::{backend::TestBackend, Terminal};

    fn app_on_help(scroll_offset: usize) -> App {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        app.state = AppState::Help { previous: Box::new(AppState::NoteList), scroll_offset };
        app
    }

    fn help_screen(app: &mut App, width: u16) -> String {
        let mut terminal = Terminal::new(TestBackend::new(width, 24)).unwrap();
        terminal.draw(|frame| render_help(app, frame)).unwrap();
        let buffer = terminal.backend().buffer();
        (0..buffer.area.height)
            .map(|y| (0..buffer.area.width).map(|x| buffer[(x, y)].symbol().to_string()).collect::<String>())
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn offset(app: &App) -> usize {
        match app.state {
            AppState::Help { scroll_offset, .. } => scroll_offset,
            _ => panic!("not on the help screen"),
        }
    }

    /// End asks for the very bottom by asking for everything.
    #[test]
    fn scrolling_to_the_end_stops_at_the_end() {
        for width in [80, 200] {
            let mut app = app_on_help(usize::MAX);

            let screen = help_screen(&mut app, width);

            assert!(offset(&app) > 0 && offset(&app) < 1000, "{}", offset(&app));
            let text = screen.chars().filter(|c| c.is_alphanumeric()).count();
            assert!(text > 100, "the screen is nearly blank:\n{screen}");
        }
    }

    #[test]
    fn the_way_back_up_starts_from_where_the_text_stopped() {
        let mut app = app_on_help(usize::MAX);
        help_screen(&mut app, 80);
        let bottom = offset(&app);

        let mut above = app_on_help(bottom - 1);
        help_screen(&mut above, 80);

        assert_eq!(offset(&above), bottom - 1);
    }
}
