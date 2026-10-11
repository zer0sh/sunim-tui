use crate::{
    app::App,
    model::format_timestamp,
    ui::{chrome, highlight_style},
};
use ratatui::{
    Frame,
    layout::{Constraint, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Row, Table, TableState},
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let site = app.current_site.as_deref().unwrap_or("—");
    let env = app.current_env.as_deref().unwrap_or("—");
    let body = chrome(f, app, area, &format!("Backups  —  {site}.{env}"));

    let head = Row::new(["  Date", "Element", "Size", "Initiator", "Expires"]).style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let date = |d: Option<f64>| d.map(format_timestamp).unwrap_or_else(|| "—".into());
    let rows: Vec<Row> = app
        .backups
        .iter()
        .map(|b| {
            Row::new([
                Cell::from(format!("  {}", date(b.date))),
                Cell::from(b.element().to_string()).style(Style::default().fg(Color::Cyan)),
                Cell::from(b.size.clone().unwrap_or_else(|| "—".into())),
                Cell::from(b.initiator.clone().unwrap_or_else(|| "—".into()))
                    .style(Style::default().fg(Color::DarkGray)),
                Cell::from(date(b.expiry)).style(Style::default().fg(Color::DarkGray)),
            ])
        })
        .collect();

    let hint = match (rows.is_empty(), app.is_loading()) {
        (true, true) => " (loading…)",
        (true, false) => " (no backups — press c to create one)",
        _ => "",
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(24),
            Constraint::Length(10),
            Constraint::Length(10),
            Constraint::Length(12),
            Constraint::Min(20),
        ],
    )
    .header(head)
    .row_highlight_style(highlight_style())
    .block(
        Block::default()
            .borders(Borders::ALL)
            .title(format!(" Backups{hint} ")),
    );

    let mut state = TableState::default().with_selected(app.backups_state.selected());
    f.render_stateful_widget(table, body, &mut state);
}
