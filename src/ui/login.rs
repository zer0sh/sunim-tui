use crate::{app::App, ui::with_footer};
use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Paragraph},
};

const LOGO: &str = r#"
  ___ _   _ _  _ ___ __  __
 / __| | | | \| |_ _|  \/  |
 \__ \ |_| | .` || || |\/| |
 |___/\___/|_|\_|___|_|  |_|
  Pantheon Site Manager
"#;

pub fn draw(f: &mut Frame, app: &App, area: Rect) {
    let area = with_footer(f, app, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(25),
            Constraint::Length(7), // logo
            Constraint::Length(1), // gap
            Constraint::Length(3), // input
            Constraint::Min(0),
        ])
        .split(area);

    f.render_widget(
        Paragraph::new(LOGO).alignment(Alignment::Center).style(
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        chunks[1],
    );

    let box_w = 60u16.min(area.width.saturating_sub(4));
    let box_x = (area.width.saturating_sub(box_w)) / 2;
    let input_area = Rect {
        x: box_x,
        y: chunks[3].y,
        width: box_w,
        height: chunks[3].height,
    };

    let display = if app.is_loading() {
        "  Signing in…".to_string()
    } else {
        "  ".to_string() + &"●".repeat(app.token_input.len().min(50))
    };

    f.render_widget(
        Paragraph::new(display)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Cyan))
                    .title(" Machine Token — paste and press Enter "),
            )
            .style(Style::default().fg(Color::White)),
        input_area,
    );
}
