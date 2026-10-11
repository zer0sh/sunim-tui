use crate::{
    app::App,
    ui::{chrome, filter_label, highlight_style},
};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem},
};

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let title = if app.selector_first_run {
        "Welcome! Choose the sites you want to manage (you can change this later with S)"
    } else {
        "Choose the sites you want to see"
    };
    let body = chrome(f, app, area, title);

    let sites = app.selector_sites();
    let items: Vec<ListItem> = sites
        .iter()
        .map(|s| {
            let checked = app.selector_checked.contains(&s.name);
            let (mark, style) = if checked {
                (
                    "[x]",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                ("[ ]", Style::default().fg(Color::DarkGray))
            };
            let fw = s.framework.as_deref().unwrap_or("—");
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {mark} "), style),
                Span::raw(format!("{:<40} ", s.name)),
                Span::styled(fw.to_string(), Style::default().fg(Color::Cyan)),
            ]))
        })
        .collect();

    let hint = match () {
        _ if !items.is_empty() => "",
        _ if !app.sites_loaded => " (loading…)",
        _ if !app.selector_filter.text.is_empty() => " (no site matches the filter)",
        _ => " (no sites on this account — press r to refresh)",
    };
    let block_title = format!(
        " Sites — {} of {} selected{}{hint} ",
        app.selector_checked.len(),
        app.sites.len(),
        filter_label(&app.selector_filter, sites.len())
    );

    let mut state = app.selector_state;
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(block_title))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, body, &mut state);
}
