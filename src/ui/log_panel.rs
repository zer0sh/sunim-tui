use crate::app::{App, LogLevel};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let visible_rows = usize::from(area.height.saturating_sub(2)); // minus borders
    let total = app.log_entries.len();
    let end = total.saturating_sub(app.log_scroll);
    let start = end.saturating_sub(visible_rows);

    let items: Vec<ListItem> = app.log_entries[start..end]
        .iter()
        .map(|entry| {
            let (tag, color) = match entry.level {
                LogLevel::Command => ("→", Color::DarkGray),
                LogLevel::Info => ("i", Color::Cyan),
                LogLevel::Success => ("✓", Color::Green),
                LogLevel::Error => ("✗", Color::Red),
            };
            let text_color = match entry.level {
                LogLevel::Info => Color::White,
                _ => color,
            };
            ListItem::new(Line::from(vec![
                Span::styled(
                    format!(" {tag} "),
                    Style::default().fg(color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(entry.message.clone(), Style::default().fg(text_color)),
            ]))
        })
        .collect();

    let position = match (total, app.log_scroll) {
        (0, _) => " no entries yet ".to_string(),
        (_, 0) => String::new(),
        (_, n) => format!(" — {n} newer below "),
    };

    let list = List::new(items).block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(format!(" Output{position}  PgUp/PgDn: scroll  l: hide ")),
    );

    f.render_widget(list, area);
}
