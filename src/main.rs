use anyhow::{Result, bail};
use crossterm::event::{self, Event, KeyCode, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use std::{io::IsTerminal, time::Duration};
use tokio::sync::mpsc;

mod app;
mod model;
mod storage;
mod system;
mod terminus;
mod ui;

use app::{App, LogLevel, TaskDone};

const USAGE: &str = "\
sunim — terminal UI for managing Pantheon sites via the Terminus CLI

Usage: sunim [OPTIONS]

Options:
  -h, --help       Print this help
  -V, --version    Print version

Requires `terminus` in $PATH. Settings: ~/.config/sunim-tui/config.toml
Press h inside the app for keyboard shortcuts.";

#[tokio::main]
async fn main() -> Result<()> {
    match std::env::args().nth(1).as_deref() {
        None => {}
        Some("-V" | "--version") => {
            println!("sunim {}", env!("CARGO_PKG_VERSION"));
            return Ok(());
        }
        Some("-h" | "--help") => {
            println!("{USAGE}");
            return Ok(());
        }
        Some(other) => bail!("unknown argument '{other}' (see sunim --help)"),
    }
    if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
        bail!("sunim needs an interactive terminal");
    }

    let config_path = storage::default_path();
    let (config, config_error) = match config_path.as_deref().map(storage::load) {
        Some(Ok(config)) => (config, None),
        Some(Err(e)) => (Default::default(), Some(format!("{e:#}"))),
        None => (Default::default(), None),
    };

    let (tx, mut rx) = mpsc::unbounded_channel::<TaskDone>();
    let mut app = App::new(config, config_path, storage::default_cache_path(), tx);
    if let Some(e) = config_error {
        app.push_log(LogLevel::Error, format!("Ignoring unreadable config: {e}"));
    }
    app.start();

    // Raw mode + alternate screen; the hook restores the terminal on a panic.
    let mut terminal = ratatui::try_init()?;
    install_panic_hook();
    let result = run(&mut terminal, &mut app, &mut rx).await;
    ratatui::restore();
    result
}

/// Background tasks report their panics through the UI (see `App::spawn`), so
/// only a panic on the UI thread tears the terminal down and gets printed —
/// printing from a worker would scribble over the TUI.
fn install_panic_hook() {
    let previous = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        if std::thread::current().name() == Some("main") {
            previous(info);
        }
    }));
}

async fn run(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    rx: &mut mpsc::UnboundedReceiver<TaskDone>,
) -> Result<()> {
    loop {
        // Apply finished background work before drawing so results show at once.
        while let Ok(done) = rx.try_recv() {
            app.handle_task_done(done);
        }

        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                return Ok(());
            }
            app.handle_key(key);
        }

        app.tick();

        if app.should_quit {
            return Ok(());
        }
    }
}
