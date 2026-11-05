//! Input handling for the mouse

use crossterm::event::{MouseButton, MouseEvent, MouseEventKind};
use rust_i18n::t;

use crate::ui::app::App;
use crate::ui::state::{AppState, FocusedPanel, InputMode, ViewMode};

/// Handle a mouse event
pub fn handle_mouse(app: &mut App, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            // Scroll preview up 3 lines
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_sub(3);
        }
        MouseEventKind::ScrollDown => {
            // Scroll preview down 3 lines
            app.preview_scroll_offset = app.preview_scroll_offset.saturating_add(3);
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // Only handle clicks in NoteList state
            if !matches!(app.state, AppState::NoteList) {
                return;
            }
            if !matches!(app.view_mode, ViewMode::NoteList) {
                return;
            }
            if app.search_active {
                // Clicking outside search exits search mode
                app.search_active = false;
            }

            let x = mouse.column;
            let y = mouse.row;

            // Check if click is in note list area
            if let Some(area) = app.note_list_area {
                if x >= area.x && x < area.x + area.width
                    && y >= area.y && y < area.y + area.height
                {
                    // Notes are several rows tall and the list scrolls, so
                    // the row alone doesn't say which note this is
                    if let Some(clicked_index) = app.note_at_row(y - area.y) {
                        if app.is_multi_select_mode {
                            // In multi-select mode, toggle selection
                            app.toggle_note_selection(clicked_index);
                        } else {
                            // Normal mode: select the note
                            app.selected_note = clicked_index;
                            app.selected_note_id = app.filtered_notes().get(clicked_index).map(|n| n.id.clone());
                            app.preview_scroll_offset = 0;
                            app.focused_panel = FocusedPanel::NoteContent;
                        }
                    }
                    return;
                }
            }

            // Check if click is on tags line (for entering tag mode)
            if let Some(area) = app.tags_line_area {
                if y == area.y && x >= area.x && x < area.x + area.width {
                    // Check if click is on a specific tag (for search)
                    let mut found_tag = None;
                    for (tag_name, tag_start, tag_end) in &app.tag_positions {
                        if x >= *tag_start && x < *tag_end {
                            found_tag = Some(tag_name.clone());
                            break;
                        }
                    }

                    if let Some(tag) = found_tag {
                        // Click on a tag: search for it
                        app.search_input = format!("#{}", tag);
                        app.invalidate_filter_cache();
                        app.search_active = true;
                        app.select_note_at(0);
                    } else {
                        // Click on "Tags:" label or empty area: enter tag mode
                        let filtered = app.filtered_notes();
                        if !filtered.is_empty() && app.selected_note < filtered.len() {
                            let note = &filtered[app.selected_note];

                            // The note view can edit the text too, so a locked note
                            // goes no further than the 't' key lets it
                            if note.locked {
                                app.notify(t!("note.locked_cannot_tag").to_string());
                                return;
                            }

                            let note_id = note.id.clone();
                            let content = note.content.clone();
                            let syntax_lang = note.syntax_language;
                            let tags = note.tags.clone();

                            app.note_input = content;
                            app.note_syntax = syntax_lang;
                            app.current_tags = tags;
                            app.editing_note_id = Some(note_id);
                            app.state = AppState::NoteView;
                            app.input_mode = InputMode::Tag;
                            app.tag_input.clear();
                        }
                    }
                    return;
                }
            }

            // Check if click is on a note link in the preview area
            if let Some(_preview_area) = app.preview_area {
                let mut found_link = None;
                for (note_id, link_y, link_x_start, link_x_end) in &app.note_link_positions {
                    if y == *link_y && x >= *link_x_start && x < *link_x_end {
                        found_link = Some(note_id.clone());
                        break;
                    }
                }

                if let Some(target_note_id) = found_link {
                    crate::ui::operations::notes::follow_link(app, target_note_id);
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::Note;
    use crate::repository::NoteRepository;
    use crossterm::event::KeyModifiers;
    use ratatui::layout::Rect;

    fn app_with_a_note(locked: bool) -> App {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::new(dir.path().join("test.db"), None).unwrap();
        std::mem::forget(dir);
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let key = app.crypto.generate_master_key();

        let mut note = Note::new("Hold still".to_string());
        note.locked = locked;
        NoteRepository::new(db.connection()).create(&note, &key).unwrap();

        app.db = Some(db);
        app.key = Some(key);
        app.state = AppState::NoteList;
        crate::ui::operations::notes::load_notes(&mut app).unwrap();
        app.tags_line_area = Some(Rect::new(10, 5, 40, 1));
        app
    }

    fn click_on_the_tags_line(app: &mut App) {
        handle_mouse(app, MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 12,
            row: 5,
            modifiers: KeyModifiers::NONE,
        });
    }

    #[test]
    fn a_click_on_the_tags_line_opens_the_tag_editor() {
        let mut app = app_with_a_note(false);

        click_on_the_tags_line(&mut app);

        assert!(matches!(app.state, AppState::NoteView));
        assert!(matches!(app.input_mode, InputMode::Tag));
    }

    #[test]
    fn a_click_on_the_tags_line_of_a_locked_note_goes_nowhere() {
        let mut app = app_with_a_note(true);

        click_on_the_tags_line(&mut app);

        assert!(matches!(app.state, AppState::NoteList));
        assert!(app.editing_note_id.is_none());
    }
}
