use crate::{
    app::App,
    ui::{chrome, filter_label, highlight_style},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let body = chrome(f, app, area, "Sites");

    let listed = app.listed_sites();
    let items: Vec<ListItem> = listed
        .iter()
        .map(|s| {
            let plan = s.plan_name.as_deref().unwrap_or("—");
            let fw = s.framework.as_deref().unwrap_or("—");
            let mut spans = vec![Span::raw(format!("  {:<40} [{plan}]  {fw}", s.name))];
            if s.frozen.unwrap_or(false) {
                spans.push(Span::styled("  [frozen]", Style::default().fg(Color::Blue)));
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let hint = match () {
        _ if !items.is_empty() => "",
        _ if !app.sites_loaded => " (loading…)",
        _ if !app.sites_filter.text.is_empty() => " (no site matches the filter — Esc clears it)",
        _ if app.sites.is_empty() => " (no sites on this account — press r to refresh)",
        _ => " (none of your selected sites exist anymore — press S)",
    };

    let title = format!(
        " Sites{}{hint} ",
        filter_label(&app.sites_filter, listed.len())
    );

    let mut state = app.sites_state;
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, body, &mut state);
}
