//! Modal dialogs for creating and deleting multidevs.

use crate::{
    app::{DeleteForm, MultidevForm},
    ui::centered,
};
use ratatui::{
    Frame,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
};

const CURSOR: &str = "▏";

fn dim() -> Style {
    Style::default().fg(Color::DarkGray)
}

fn key(text: &'static str) -> Span<'static> {
    Span::styled(
        text,
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )
}

fn render(f: &mut Frame, title: String, color: Color, lines: Vec<Line<'static>>) {
    let area = f.area();
    let height = u16::try_from(lines.len() + 2).unwrap_or(u16::MAX);
    let popup = centered(area, 70.min(area.width.saturating_sub(4)), height);
    f.render_widget(Clear, popup);
    f.render_widget(
        Paragraph::new(lines).wrap(Wrap { trim: false }).block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(color))
                .title(title),
        ),
        popup,
    );
}

pub fn new_multidev(f: &mut Frame, form: &MultidevForm) {
    let mut sources = vec![Span::raw("  From      ")];
    for (i, source) in form.sources.iter().enumerate() {
        let style = if i == form.source {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Cyan)
                .add_modifier(Modifier::BOLD)
        } else {
            dim()
        };
        sources.push(Span::styled(format!(" {source} "), style));
        sources.push(Span::raw(" "));
    }

    let feedback = match (form.name.is_empty(), form.problem()) {
        (true, _) => Line::styled(
            "  Max 11 characters: lowercase letters, numbers and hyphens.",
            dim(),
        ),
        (false, Some(problem)) => {
            Line::styled(format!("  ✗ {problem}"), Style::default().fg(Color::Red))
        }
        (false, None) => Line::styled(
            format!(
                "  ✓ Creates {}.{} with the database and files of {}.",
                form.site,
                form.name,
                form.source_env()
            ),
            Style::default().fg(Color::Green),
        ),
    };

    let lines = vec![
        Line::default(),
        Line::from(vec![
            Span::raw("  Name      "),
            Span::styled(
                form.name.clone(),
                Style::default().add_modifier(Modifier::BOLD),
            ),
            Span::styled(CURSOR, Style::default().fg(Color::Cyan)),
        ]),
        Line::default(),
        Line::from(sources),
        Line::styled(
            "            database & files are cloned from this environment",
            dim(),
        ),
        Line::default(),
        feedback,
        Line::default(),
        Line::from(vec![
            Span::raw("  "),
            key("Enter"),
            Span::styled(" create   ", dim()),
            key("Tab/←→"),
            Span::styled(" source   ", dim()),
            key("Esc"),
            Span::styled(" cancel", dim()),
        ]),
    ];
    render(
        f,
        format!(" New multidev — {} ", form.site),
        Color::Cyan,
        lines,
    );
}

pub fn delete_multidev(f: &mut Frame, form: &DeleteForm) {
    let typed_style = if form.env.starts_with(&form.typed) {
        Style::default()
            .fg(Color::Green)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    };
    let branch = if form.delete_branch { "[x]" } else { "[ ]" };
    let enter_style = if form.confirmed() {
        Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)
    } else {
        dim()
    };

    let lines = vec![
        Line::default(),
        Line::from(vec![
            Span::raw("  This permanently deletes "),
            Span::styled(
                format!("{}.{}", form.site, form.env),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::raw("  with its database and files. This can't be undone."),
        Line::default(),
        Line::from(vec![
            Span::raw(format!("  Type {} to confirm:  ", form.env)),
            Span::styled(form.typed.clone(), typed_style),
            Span::styled(CURSOR, Style::default().fg(Color::Cyan)),
        ]),
        Line::default(),
        Line::from(vec![
            Span::raw(format!(
                "  {branch} Also delete the git branch '{}'",
                form.env
            )),
            Span::styled("  (Tab)", dim()),
        ]),
        Line::default(),
        Line::from(vec![
            Span::raw("  "),
            Span::styled("Enter delete", enter_style),
            Span::styled("   ", dim()),
            key("Tab"),
            Span::styled(" branch   ", dim()),
            key("Esc"),
            Span::styled(" cancel", dim()),
        ]),
    ];
    render(f, " Delete multidev ".into(), Color::Red, lines);
}
