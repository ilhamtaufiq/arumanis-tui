mod api;
mod app;
mod ui;

use std::time::Duration;

use anyhow::Result;
use crossterm::event::{self, Event, KeyEventKind};

use api::{Client, DEFAULT_BASE};
use app::App;

fn main() -> Result<()> {
    let base = std::env::var("APIAMIS_URL").unwrap_or_else(|_| DEFAULT_BASE.to_string());
    let client = Client::new(base)?;
    let mut app = App::new(client);

    let mut terminal = ratatui::init();
    let result = run(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn run(terminal: &mut ratatui::DefaultTerminal, app: &mut App) -> Result<()> {
    while !app.should_quit {
        terminal.draw(|frame| ui::draw(frame, app))?;
        if event::poll(Duration::from_millis(200))? {
            if let Event::Key(key) = event::read()? {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
        }
    }
    Ok(())
}
