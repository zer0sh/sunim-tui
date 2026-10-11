use crate::{app::App, ui::chrome};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Cell, Paragraph, Row, Table},
};

const PREVIEW_ROWS: usize = 5;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let sites = app.visible_sites();
    let total = sites.len();

    let user = app.whoami.as_deref().unwrap_or("—");
    let scope = if app.has_site_selection() {
        format!("   ·   showing {total} of {} sites", app.sites.len())
    } else {
        String::new()
    };
    let body = chrome(f, app, area, &format!("sunim   {user}{scope}"));
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // stat cards
            Constraint::Min(0),    // sites table
        ])
        .split(body);

    // ── stat cards ────────────────────────────────────────────────────────────
    let framework_is = |s: &&&crate::model::Site, names: &[&str]| {
        s.framework
            .as_deref()
            .map(str::to_lowercase)
            .is_some_and(|f| names.iter().any(|n| f.contains(n)))
    };
    let wp = sites
        .iter()
        .filter(|s| framework_is(s, &["wordpress"]))
        .count();
    let drupal = sites
        .iter()
        .filter(|s| framework_is(s, &["drupal", "backdrop"]))
        .count();
    let other = total.saturating_sub(wp + drupal);

    let cards = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Ratio(1, 4); 4])
        .split(chunks[0]);

    let loading_suffix = if app.is_loading() && !app.sites_loaded {
        " …"
    } else {
        ""
    };
    render_card(
        f,
        cards[0],
        " Sites ",
        &format!("{total}{loading_suffix}"),
        Color::Cyan,
    );
    render_card(f, cards[1], " WordPress ", &format!("{wp}"), Color::Green);
    render_card(f, cards[2], " Drupal ", &format!("{drupal}"), Color::Yellow);
    render_card(f, cards[3], " Other ", &format!("{other}"), Color::DarkGray);

    // ── preview table ─────────────────────────────────────────────────────────
    let more = total.saturating_sub(PREVIEW_ROWS);
    let title = match (total, more) {
        (0, _) => " Sites ".to_string(),
        (_, 0) => format!(" Sites A→Z — {total} total "),
        (_, m) => format!(" Sites A→Z — first {PREVIEW_ROWS} of {total}   +{m} more, press s "),
    };

    let header = Row::new(["  Name", "Framework", "Plan"]).style(
        Style::default()
            .fg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );

    let rows: Vec<Row> = if sites.is_empty() {
        let hint = match () {
            _ if !app.sites_loaded => "  loading…",
            _ if app.sites.is_empty() => "  no sites loaded — press r to refresh",
            _ => "  none of your selected sites exist anymore — press S",
        };
        vec![Row::new([hint, "", ""])]
    } else {
        sites
            .iter()
            .take(PREVIEW_ROWS)
            .map(|s| {
                Row::new([
                    Cell::from(format!("  {}", s.name)),
                    Cell::from(s.framework.as_deref().unwrap_or("—").to_string())
                        .style(Style::default().fg(Color::Cyan)),
                    Cell::from(s.plan_name.as_deref().unwrap_or("—").to_string())
                        .style(Style::default().fg(Color::DarkGray)),
                ])
            })
            .collect()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Percentage(45),
            Constraint::Percentage(25),
            Constraint::Percentage(30),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(title),
    );

    f.render_widget(table, chunks[1]);
}

fn render_card(f: &mut Frame, area: Rect, title: &str, value: &str, color: Color) {
    f.render_widget(
        Paragraph::new(format!("\n  {value}"))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(color))
                    .title(title),
            )
            .style(Style::default().fg(color).add_modifier(Modifier::BOLD)),
        area,
    );
}
