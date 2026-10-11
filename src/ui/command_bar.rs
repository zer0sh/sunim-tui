use crate::app::App;
use ratatui::{
    Frame,
    layout::Rect,
    style::{Color, Style},
    widgets::{Block, Borders, Paragraph},
};

pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    let bar = Rect {
        x: 0,
        y: area.height.saturating_sub(3),
        width: area.width,
        height: 3,
    };

    f.render_widget(
        Paragraph::new(format!(":{}_", app.command_input))
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(Color::Yellow))
                    .title(" Command — Esc to cancel "),
            )
            .style(Style::default().fg(Color::White)),
        bar,
    );
}
