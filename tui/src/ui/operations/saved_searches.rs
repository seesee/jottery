//! Saved searches: naming the current query, and going back to one.

use anyhow::{Context, Result};
use rust_i18n::t;
use zeroize::Zeroize;

use crate::repository::SavedSearchRepository;

use super::super::app::App;
use super::super::state::{InputMode, ViewMode};

/// Read the saved searches and show the list of them.
pub fn open_picker(app: &mut App) -> Result<()> {
    load(app)?;
    app.selected_saved_search = 0;
    app.show_saved_search_delete_confirm = false;
    app.view_mode = ViewMode::SavedSearches;
    Ok(())
}

fn load(app: &mut App) -> Result<()> {
    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    app.saved_searches = SavedSearchRepository::new(db.connection()).list(key)?;
    Ok(())
}

/// Ask what to call the query in the search bar.
pub fn begin_saving(app: &mut App) {
    if app.search_input.trim().is_empty() {
        app.notify(t!("saved_search.nothing_to_save").to_string());
        return;
    }
    app.saved_search_name_input.clear();
    app.input_mode = InputMode::SaveSearchName;
}

/// Save the query in the search bar under the name typed in.
pub fn save_current(app: &mut App) -> Result<()> {
    let name = app.saved_search_name_input.trim().to_string();
    app.saved_search_name_input.zeroize();
    app.input_mode = InputMode::Normal;
    if name.is_empty() {
        return Ok(());
    }

    let db = app.db.as_ref().context("Database not available")?;
    let key = app.key.as_ref().context("Key not available")?;
    SavedSearchRepository::new(db.connection()).create(&name, app.search_input.trim(), key)?;

    app.notify(t!("saved_search.saved", name = name).to_string());
    if app.settings.sync_enabled {
        app.trigger_sync();
    }
    Ok(())
}

/// Run the search highlighted in the list.
pub fn apply_selected(app: &mut App) {
    let Some(search) = app.saved_searches.get(app.selected_saved_search) else {
        return;
    };

    app.search_input = search.query.clone();
    app.search_active = true;
    app.view_mode = ViewMode::NoteList;
    app.invalidate_filter_cache();
    app.select_note_at(0);
    app.selected_note_id = app.filtered_notes().first().map(|note| note.id.clone());
    app.preview_scroll_offset = 0;
}

/// Delete the search highlighted in the list, on every device.
pub fn delete_selected(app: &mut App) -> Result<()> {
    let Some(search) = app.saved_searches.get(app.selected_saved_search) else {
        return Ok(());
    };

    let db = app.db.as_ref().context("Database not available")?;
    SavedSearchRepository::new(db.connection()).delete(&search.id)?;

    load(app)?;
    app.selected_saved_search = app.selected_saved_search.min(app.saved_searches.len().saturating_sub(1));
    app.notify(t!("saved_search.deleted").to_string());

    if app.settings.sync_enabled {
        app.trigger_sync();
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Database;
    use crate::models::Note;
    use crate::repository::NoteRepository;
    use crate::ui::state::AppState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

    fn app_with_notes() -> App {
        let dir = std::env::temp_dir().join(format!("jottery-saved-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let mut app = App::new(dir.join("test.db"), None).unwrap();
        let db = Database::in_memory(&uuid::Uuid::new_v4().to_string()).unwrap();
        let key = app.crypto.generate_master_key();

        let mut tagged = Note::new("Tagged".to_string());
        tagged.tags = vec!["work".to_string()];
        let repo = NoteRepository::new(db.connection());
        repo.create(&tagged, &key).unwrap();
        repo.create(&Note::new("Other".to_string()), &key).unwrap();

        app.db = Some(db);
        app.key = Some(key);
        app.state = AppState::NoteList;
        crate::ui::operations::notes::load_notes(&mut app).unwrap();
        app
    }

    fn press(app: &mut App, code: KeyCode) {
        app.handle_key(KeyEvent::new(code, KeyModifiers::NONE)).unwrap();
    }

    fn type_text(app: &mut App, text: &str) {
        for letter in text.chars() {
            press(app, KeyCode::Char(letter));
        }
    }

    #[test]
    fn a_query_can_be_saved_and_picked_again() {
        let mut app = app_with_notes();

        press(&mut app, KeyCode::Char('/'));
        type_text(&mut app, "#work");
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)).unwrap();
        assert!(matches!(app.input_mode, InputMode::SaveSearchName));

        type_text(&mut app, "Work");
        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.input_mode, InputMode::Normal));

        press(&mut app, KeyCode::Esc);
        assert!(app.search_input.is_empty());

        press(&mut app, KeyCode::Char('g'));
        assert!(matches!(app.view_mode, ViewMode::SavedSearches));
        assert_eq!(app.saved_searches[0].name, "Work");

        press(&mut app, KeyCode::Enter);
        assert!(matches!(app.view_mode, ViewMode::NoteList));
        assert_eq!(app.search_input, "#work");
        assert_eq!(app.filtered_notes().len(), 1);
    }

    #[test]
    fn an_empty_search_bar_has_nothing_to_save() {
        let mut app = app_with_notes();

        press(&mut app, KeyCode::Char('/'));
        app.handle_key(KeyEvent::new(KeyCode::Char('s'), KeyModifiers::CONTROL)).unwrap();

        assert!(matches!(app.input_mode, InputMode::Normal));
    }

    #[test]
    fn deleting_asks_first() {
        let mut app = app_with_notes();
        let db = app.db.as_ref().unwrap();
        SavedSearchRepository::new(db.connection()).create("Doomed", "x", app.key.as_ref().unwrap()).unwrap();
        press(&mut app, KeyCode::Char('g'));

        press(&mut app, KeyCode::Char('d'));
        assert!(app.show_saved_search_delete_confirm);
        press(&mut app, KeyCode::Char('n'));
        assert_eq!(app.saved_searches.len(), 1);

        press(&mut app, KeyCode::Char('d'));
        press(&mut app, KeyCode::Char('y'));
        assert!(app.saved_searches.is_empty());
    }

    #[test]
    fn locking_forgets_what_the_list_held() {
        let mut app = app_with_notes();
        let db = app.db.as_ref().unwrap();
        SavedSearchRepository::new(db.connection()).create("Private", "x", app.key.as_ref().unwrap()).unwrap();
        press(&mut app, KeyCode::Char('g'));

        app.lock();

        assert!(app.saved_searches.is_empty());
        assert!(matches!(app.view_mode, ViewMode::NoteList));
    }
}
