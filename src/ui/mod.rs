use crate::{
    app::{App, ListFilter, Overlay, Screen},
    model,
};
use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

mod backups;
mod command_bar;
mod dashboard;
mod dialogs;
mod environments;
mod help;
pub mod keymap;
mod log_panel;
mod login;
mod selector;
mod sites;

const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧"];
/// Longest task label shown in the top-right corner.
const ACTIVITY_LABEL_MAX: usize = 40;

pub fn draw(f: &mut Frame, app: &App) {
    let full = f.area();

    // Split vertically when log panel is open (only if enough height)
    let main_area = if app.show_log && full.height > 12 {
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(full);
        log_panel::draw(f, app, chunks[1]);
        chunks[0]
    } else {
        full
    };

    match app.screen {
        Screen::Login => login::draw(f, app, main_area),
        Screen::Dashboard => dashboard::draw(f, app, main_area),
        Screen::Sites => sites::draw(f, app, main_area),
        Screen::Environments => environments::draw(f, app, main_area),
        Screen::Backups => backups::draw(f, app, main_area),
        Screen::SiteSelector => selector::draw(f, app, main_area),
    }

    draw_activity(f, app);

    // Overlays always use the full terminal area
    match &app.overlay {
        Overlay::Help => help::draw(f, app),
        Overlay::Command => command_bar::draw(f, app),
        Overlay::Confirm(msg, _) => draw_confirm(f, msg),
        Overlay::NewMultidev(form) => dialogs::new_multidev(f, form),
        Overlay::DeleteMultidev(form) => dialogs::delete_multidev(f, form),
        Overlay::None => {}
    }
}

// ── screen frame ──────────────────────────────────────────────────────────────

/// Draws the header and the footer of a screen and returns the body area.
pub fn chrome(f: &mut Frame, app: &App, area: Rect, title: &str) -> Rect {
    let body = with_footer(f, app, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(body);
    f.render_widget(
        Paragraph::new(format!("  {title}"))
            .style(
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            )
            .block(Block::default().borders(Borders::BOTTOM)),
        chunks[0],
    );
    chunks[1]
}

/// Draws the footer (message line + key hints, wrapped over as many lines as
/// needed) at the bottom of `area` and returns what is left above it.
pub fn with_footer(f: &mut Frame, app: &App, area: Rect) -> Rect {
    let hints = keymap::footer_hints(app);
    let hint_lines = hint_lines(&hints, area.width);
    let height = u16::try_from(hint_lines.len() + 1)
        .unwrap_or(u16::MAX)
        .min(area.height / 2);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(height)])
        .split(area);

    let message = if let Some(err) = &app.error {
        Line::styled(
            format!(" ✗ {err}"),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )
    } else if let Some(ok) = &app.status {
        Line::styled(format!(" ✓ {ok}"), Style::default().fg(Color::Green))
    } else {
        Line::default()
    };
    let mut lines = vec![message];
    lines.extend(hint_lines);
    f.render_widget(Paragraph::new(lines), chunks[1]);
    chunks[0]
}

/// Packs `key label` pairs into as few lines as fit in `width`.
fn hint_lines(hints: &[(&'static str, &'static str)], width: u16) -> Vec<Line<'static>> {
    const GAP: &str = "   ";
    let width = usize::from(width).max(1);
    let mut lines = vec![];
    let mut spans: Vec<Span<'static>> = vec![];
    let mut used = 0;
    for (keys, label) in hints {
        let item = keys.chars().count() + 1 + label.chars().count();
        if !spans.is_empty() && used + GAP.len() + item > width {
            lines.push(Line::from(std::mem::take(&mut spans)));
        }
        let lead = if spans.is_empty() { " " } else { GAP };
        used = if spans.is_empty() { 0 } else { used } + lead.len() + item;
        spans.push(Span::raw(lead));
        spans.push(Span::styled(
            *keys,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::styled(
            format!(" {label}"),
            Style::default().fg(Color::DarkGray),
        ));
    }
    if !spans.is_empty() {
        lines.push(Line::from(spans));
    }
    lines
}

/// Top-right corner: what is loading, or how old the data on screen is.
fn draw_activity(f: &mut Frame, app: &App) {
    let area = f.area();
    let (text, style) = if let Some((label, others)) = app.activity() {
        let frame = SPINNER[usize::from(app.spinner_tick) % SPINNER.len()];
        let label = truncate(label, ACTIVITY_LABEL_MAX);
        let more = if others > 0 {
            format!(" (+{others})")
        } else {
            String::new()
        };
        (
            format!(" {frame} {label}{more} "),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else if let Some(fetched) = app.data_updated() {
        (
            format!(
                " updated {} · r refresh ",
                model::format_age(fetched, model::now_unix())
            ),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        return;
    };
    let width = u16::try_from(text.chars().count())
        .unwrap_or(u16::MAX)
        .min(area.width);
    let badge = Rect {
        x: area.width - width,
        y: 0,
        width,
        height: 1.min(area.height),
    };
    f.render_widget(Clear, badge);
    f.render_widget(Paragraph::new(text).style(style), badge);
}

fn truncate(text: &str, max: usize) -> String {
    if text.chars().count() <= max {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

// ── shared widgets ────────────────────────────────────────────────────────────

pub fn highlight_style() -> Style {
    Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan)
        .add_modifier(Modifier::BOLD)
}

/// Title suffix describing a `/` filter (empty when there is none).
pub fn filter_label(filter: &ListFilter, matches: usize) -> String {
    match (filter.editing, filter.text.is_empty()) {
        (true, _) => format!(" — filter: {}_ ", filter.text),
        (false, false) => format!(" — filter: {} ({matches}) ", filter.text),
        (false, true) => String::new(),
    }
}

pub fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    }
}

fn draw_confirm(f: &mut Frame, msg: &str) {
    let area = f.area();
    let width = 64.min(area.width.saturating_sub(4)).max(10);
    let inner = usize::from(width.saturating_sub(2)).max(1);
    // Rough wrapped height; +1 absorbs word-wrapping slack.
    let text_lines = msg.chars().count().div_ceil(inner) + 1;
    let height = u16::try_from(text_lines + 4).unwrap_or(u16::MAX);
    let popup = centered(area, width, height);

    let text = vec![
        Line::from(msg.to_string()),
        Line::from(""),
        Line::from(vec![
            Span::styled(
                "y",
                Style::default()
                    .fg(Color::Green)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(": confirm   any other key: cancel"),
        ]),
    ];

    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(text)
            .wrap(Wrap { trim: true })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Red))
                    .title(" Confirm "),
            )
            .style(
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        popup,
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    fn text(line: &Line) -> String {
        line.spans.iter().map(|s| s.content.as_ref()).collect()
    }

    #[test]
    fn hints_wrap_to_the_available_width() {
        let hints = [("a", "one"), ("b", "two"), ("c", "three")];
        let wide = hint_lines(&hints, 80);
        assert_eq!(wide.len(), 1);
        assert_eq!(text(&wide[0]), " a one   b two   c three");

        let narrow = hint_lines(&hints, 14);
        assert_eq!(narrow.len(), 2);
        assert!(narrow.iter().all(|l| text(l).chars().count() <= 14));
        assert!(hint_lines(&[], 80).is_empty());
    }

    #[test]
    fn long_labels_are_truncated() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefghij", 5), "abcd…");
    }
}
