//! The list of saved searches, in a modal overlay

use ratatui::{
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use rust_i18n::t;

use super::modal::centered_rect_percent;
use crate::ui::app::App;

/// Render the list of saved searches
pub fn render_saved_searches(app: &App, frame: &mut Frame, area: Rect) {
    let modal_area = centered_rect_percent(70, 60, area);
    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .title(format!(" {} ({}) ", t!("saved_search.title"), app.saved_searches.len()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if app.saved_searches.is_empty() {
        let empty = Paragraph::new(t!("saved_search.empty").to_string())
            .style(Style::default().fg(Color::DarkGray))
            .wrap(Wrap { trim: true });
        frame.render_widget(empty, inner);
        return;
    }

    let items: Vec<ListItem> = app.saved_searches.iter().enumerate().map(|(index, search)| {
        let style = if index == app.selected_saved_search {
            Style::default().bg(Color::Yellow).fg(Color::Black).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };

        ListItem::new(vec![
            Line::from(Span::styled(search.name.clone(), style)),
            Line::from(Span::styled(format!("  {}", search.query), Style::default().fg(Color::DarkGray))),
        ])
    }).collect();

    frame.render_widget(List::new(items), inner);

    if app.show_saved_search_delete_confirm {
        if let Some(search) = app.saved_searches.get(app.selected_saved_search) {
            let question = centered_rect_percent(60, 20, area);
            frame.render_widget(Clear, question);
            let text = t!("saved_search.delete_confirm", name = search.name).to_string();
            frame.render_widget(
                Paragraph::new(text)
                    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Red)))
                    .wrap(Wrap { trim: true }),
                question,
            );
        }
    }
}
