mod app;
mod render;

use automerge::AutoCommit;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use envilib::{error::Result, store::{Session, Store}};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::time::Duration;

use app::App;

pub async fn run(doc: AutoCommit, store: Store, session: Session, invite_link: String) -> Result<()> {
    enable_raw_mode().map_err(|e| envilib::error::Error::Other(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)
        .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

    let result = run_app(&mut terminal, doc, store, session, invite_link).await;

    disable_raw_mode().map_err(|e| envilib::error::Error::Other(e.to_string()))?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )
    .map_err(|e| envilib::error::Error::Other(e.to_string()))?;
    terminal.show_cursor()
        .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    doc: AutoCommit,
    store: Store,
    session: Session,
    invite_link: String,
) -> Result<()> {
    let mut app = App::new(doc, store, session, invite_link)?;

    loop {
        terminal
            .draw(|f| render::render(f, &app))
            .map_err(|e| envilib::error::Error::Other(e.to_string()))?;

        // Flush pending async results (persists, etc.)
        app.tick().await;

        // Poll for events with a short timeout so tick() runs regularly
        if event::poll(Duration::from_millis(50))
            .map_err(|e| envilib::error::Error::Other(e.to_string()))?
        {
            if let Event::Key(key) = event::read()
                .map_err(|e| envilib::error::Error::Other(e.to_string()))?
            {
                if key.modifiers.contains(KeyModifiers::CONTROL) && key.code == KeyCode::Char('c') {
                    break;
                }
                if app.handle_key(key).await? {
                    break; // quit
                }
            }
        }
    }

    Ok(())
}
