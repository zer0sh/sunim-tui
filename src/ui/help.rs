//! Scrollable help overlay with *everything*: the current screen first, then
//! every other screen, dialog, command, tag and file. Generated from
//! `keymap`, so it always matches the footer hints.

use crate::{
    app::{App, Screen},
    ui::{
        centered,
        keymap::{self, Section},
    },
};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

const KEY_COLUMN: usize = 26;

const TAGS: &[(&str, &str)] = &[
    ("🔒", "dev, test and live: can't be deleted"),
    (
        "[Git] / [SFTP]",
        "Connection mode of the environment (m toggles it)",
    ),
    ("[locked]", "Environment protected with a username/password"),
    (
        "[uninit]",
        "Environment not initialized yet (e.g. test/live before the first deploy)",
    ),
    ("[frozen]", "Site frozen by Pantheon (inactive sandbox)"),
    ("[x] / [ ]", "Selected / not selected in the site selector"),
];

const INDICATORS: &[(&str, &str)] = &[
    (
        "⠋ env:list site (+1)",
        "Top right: what is loading now, and how many other tasks run",
    ),
    (
        "updated 5m ago",
        "Top right: age of the data on screen, shown from cache until refreshed",
    ),
    ("✗ message", "Footer: error, stays until Esc"),
    ("✓ message", "Footer: success, disappears after 5 seconds"),
];

const FILES: &[(&str, &str)] = &[
    (
        "config",
        "~/.config/sunim-tui/config.toml — token and selected sites (0600)",
    ),
    (
        "cache",
        "~/.cache/sunim-tui/cache.json — last sites/environments/backups (0600)",
    ),
    ("sunim --help", "Command-line usage"),
    ("sunim --version", "Print the version"),
];

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let popup = centered(
        area,
        100.min(area.width.saturating_sub(2)),
        area.height.saturating_sub(2),
    );
    let inner_width = usize::from(popup.width.saturating_sub(2)).max(1);
    let inner_height = popup.height.saturating_sub(2);

    let lines = content(app, inner_width);
    // Wrapped height, so scrolling can reach the very last line.
    let total: usize = lines
        .iter()
        .map(|l| l.width().max(1).div_ceil(inner_width))
        .sum();
    let total = u16::try_from(total).unwrap_or(u16::MAX);
    let max_scroll = total.saturating_sub(inner_height);
    app.help_max_scroll.set(max_scroll);
    let scroll = app.help_scroll.min(max_scroll);

    let position = if max_scroll == 0 {
        String::new()
    } else {
        format!(" {}% ", u32::from(scroll) * 100 / u32::from(max_scroll))
    };

    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .scroll((scroll, 0))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Help — j/k PgUp/PgDn scroll · Esc close ")
                    .title_bottom(Line::from(position).right_aligned()),
            ),
        popup,
    );
}

/// All help lines, current screen first, wrapped to `width` columns.
pub fn content(app: &App, width: usize) -> Vec<Line<'static>> {
    let current = keymap::screen_section(app.screen);
    let mut order: Vec<&'static Section> = vec![current];
    let list_screen = !matches!(app.screen, Screen::Login | Screen::Dashboard);
    if list_screen {
        order.push(&keymap::NAVIGATION);
    }
    if matches!(
        app.screen,
        Screen::Sites | Screen::SiteSelector | Screen::Environments
    ) {
        order.push(&keymap::FILTER);
    }
    if app.screen == Screen::Environments {
        order.extend([&keymap::NEW_MULTIDEV, &keymap::DELETE_MULTIDEV]);
    }
    order.extend([
        &keymap::GLOBAL,
        &keymap::COMMAND_BAR,
        &keymap::COMMANDS,
        &keymap::CONFIRM,
    ]);
    let others: Vec<&'static Section> = keymap::SCREENS
        .iter()
        .map(|(_, s)| *s)
        .chain([
            &keymap::NAVIGATION,
            &keymap::FILTER,
            &keymap::NEW_MULTIDEV,
            &keymap::DELETE_MULTIDEV,
            &keymap::HELP,
        ])
        .collect();

    let mut lines = vec![
        Line::styled(
            format!(
                " sunim {} — everything you can do",
                env!("CARGO_PKG_VERSION")
            ),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Line::default(),
    ];
    let mut shown: Vec<&'static Section> = vec![];
    for (i, section) in order.into_iter().chain(others).enumerate() {
        if shown.iter().any(|s| std::ptr::eq(*s, section)) {
            continue;
        }
        let title = if i == 0 {
            format!("▶ THIS SCREEN — {}", section.title.to_uppercase())
        } else {
            section.title.to_uppercase()
        };
        push_block(
            &mut lines,
            width,
            &title,
            section.keys.iter().map(|k| (k.keys, k.help)),
        );
        shown.push(section);
    }
    push_block(&mut lines, width, "TAGS", TAGS.iter().copied());
    push_block(&mut lines, width, "INDICATORS", INDICATORS.iter().copied());
    push_block(
        &mut lines,
        width,
        "FILES & COMMAND LINE",
        FILES.iter().copied(),
    );
    lines
}

fn push_block<'a>(
    lines: &mut Vec<Line<'static>>,
    width: usize,
    title: &str,
    rows: impl Iterator<Item = (&'a str, &'a str)>,
) {
    const INDENT: usize = 3;
    lines.push(Line::styled(
        format!(" {title}"),
        Style::default()
            .fg(Color::Yellow)
            .add_modifier(Modifier::BOLD),
    ));
    // Descriptions wrap with a hanging indent under their own column.
    let text_width = width.saturating_sub(INDENT + KEY_COLUMN).max(20);
    for (keys, help) in rows {
        // Display width, not chars: emoji such as 🔒 take two columns.
        let pad = KEY_COLUMN.saturating_sub(Span::raw(keys).width()).max(1);
        let mut chunks = wrap(help, text_width).into_iter();
        lines.push(Line::from(vec![
            Span::styled(
                format!("{}{keys}{}", " ".repeat(INDENT), " ".repeat(pad)),
                Style::default().fg(Color::Cyan),
            ),
            Span::raw(chunks.next().unwrap_or_default()),
        ]));
        for chunk in chunks {
            lines.push(Line::raw(format!(
                "{}{chunk}",
                " ".repeat(INDENT + KEY_COLUMN)
            )));
        }
    }
    lines.push(Line::default());
}

/// Greedy word wrap.
fn wrap(text: &str, width: usize) -> Vec<String> {
    let mut out = vec![];
    let mut line = String::new();
    for word in text.split_whitespace() {
        if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
            out.push(std::mem::take(&mut line));
        }
        if !line.is_empty() {
            line.push(' ');
        }
        line.push_str(word);
    }
    if !line.is_empty() || out.is_empty() {
        out.push(line);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{app::App, storage::Config};
    use tokio::sync::mpsc;

    fn text(lines: &[Line]) -> String {
        lines
            .iter()
            .map(|l| {
                l.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    #[test]
    fn help_contains_every_section_once_on_every_screen() {
        let (tx, _rx) = mpsc::unbounded_channel();
        let mut app = App::new(Config::default(), None, None, tx);
        for (screen, current) in keymap::SCREENS {
            app.screen = screen;
            // Wide enough that no description wraps.
            let help = text(&content(&app, 300));
            assert!(help.contains(&format!("THIS SCREEN — {}", current.title.to_uppercase())));
            for (_, section) in keymap::SCREENS {
                let title = section.title.to_uppercase();
                assert_eq!(help.matches(&title).count(), 1, "{title} on {screen:?}");
            }
            for section in [
                &keymap::NAVIGATION,
                &keymap::GLOBAL,
                &keymap::FILTER,
                &keymap::NEW_MULTIDEV,
                &keymap::DELETE_MULTIDEV,
                &keymap::CONFIRM,
                &keymap::COMMAND_BAR,
                &keymap::COMMANDS,
                &keymap::HELP,
            ] {
                for k in section.keys {
                    assert!(help.contains(k.help), "{} missing on {screen:?}", k.help);
                }
            }
            for (label, _) in TAGS.iter().chain(INDICATORS).chain(FILES) {
                assert!(help.contains(label));
            }
        }
    }

    #[test]
    fn long_descriptions_wrap_under_their_column() {
        assert_eq!(wrap("a bb ccc dddd", 6), ["a bb", "ccc", "dddd"]);
        assert_eq!(wrap("", 6), [""]);

        let (tx, _rx) = mpsc::unbounded_channel();
        let app = App::new(Config::default(), None, None, tx);
        for line in content(&app, 60) {
            assert!(line.width() <= 60, "{line:?}");
        }
    }
}
