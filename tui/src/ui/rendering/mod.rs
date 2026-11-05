//! Rendering modules for the TUI application

pub mod conflict;
pub mod credentials;
pub mod help;
pub mod inbox;
pub mod locked;
pub mod modal;
pub mod note_info;
pub mod note_list;
pub mod note_view;
pub mod saved_searches;
pub mod settings;
pub mod version_history;

/// The furthest a paragraph can scroll in `area` and still fill it, counting the rows it
/// really takes up once wrapped. Going further only shows blank space.
pub(crate) fn furthest_scroll(paragraph: &ratatui::widgets::Paragraph<'_>, area: ratatui::layout::Rect) -> usize {
    paragraph.line_count(area.width).saturating_sub(area.height as usize)
}

/// A scroll position in the form a paragraph takes it.
pub(crate) fn scroll_rows(offset: usize) -> u16 {
    u16::try_from(offset).unwrap_or(u16::MAX)
}
