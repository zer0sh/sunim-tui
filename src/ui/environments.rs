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
    let site = app.current_site.as_deref().unwrap_or("—");
    let body = chrome(f, app, area, &format!("Environments  —  {site}"));

    let envs = app.listed_environments();
    let items: Vec<ListItem> = envs
        .iter()
        .map(|e| {
            let domain = e.domain.as_deref().unwrap_or("—");
            let protected = if e.is_protected() { " 🔒" } else { "" };
            let mut spans = vec![Span::raw(format!(
                "  {:<12}{protected:<3} {:<45}",
                e.id, domain
            ))];
            let mut tag = |text: &str, color: Color| {
                spans.push(Span::styled(
                    format!(" [{text}]"),
                    Style::default().fg(color),
                ));
            };
            match e.connection_mode.as_deref() {
                Some("sftp") => tag("SFTP", Color::Yellow),
                Some("git") => tag("Git", Color::DarkGray),
                _ => {}
            }
            if e.locked.unwrap_or(false) {
                tag("locked", Color::Magenta);
            }
            if !e.initialized.unwrap_or(true) {
                tag("uninit", Color::Red);
            }
            ListItem::new(Line::from(spans))
        })
        .collect();

    let hint = match () {
        _ if !items.is_empty() => "",
        _ if app.is_loading() && app.environments.is_empty() => " (loading…)",
        _ if !app.envs_filter.text.is_empty() => {
            " (no environment matches the filter — Esc clears it)"
        }
        _ if app.environments.is_empty() => " (no environments — press r to refresh)",
        _ => "",
    };

    let title = format!(
        " Environments{}{hint} ",
        filter_label(&app.envs_filter, envs.len())
    );

    let mut state = app.envs_state;
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(highlight_style())
        .highlight_symbol("▶ ");
    f.render_stateful_widget(list, body, &mut state);
}
