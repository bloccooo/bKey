mod app;
mod render;

use automerge::AutoCommit;
use crossterm::{
    event::{self, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lib::{error::Result, store::{Session, Store}};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::time::Duration;

use app::App;

pub async fn run(doc: AutoCommit, store: Store, session: Session, invite_link: String, account_name: String, workspace_name: String, storage_backend: String) -> Result<()> {
    enable_raw_mode().map_err(|e| lib::error::Error::Other(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)
        .map_err(|e| lib::error::Error::Other(e.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| lib::error::Error::Other(e.to_string()))?;

    let result = run_app(&mut terminal, doc, store, session, invite_link, account_name, workspace_name, storage_backend).await;

    disable_raw_mode().map_err(|e| lib::error::Error::Other(e.to_string()))?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)
        .map_err(|e| lib::error::Error::Other(e.to_string()))?;
    terminal.show_cursor()
        .map_err(|e| lib::error::Error::Other(e.to_string()))?;

    result
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    doc: AutoCommit,
    store: Store,
    session: Session,
    invite_link: String,
    account_name: String,
    workspace_name: String,
    storage_backend: String,
) -> Result<()> {
    let mut app = App::new(doc, store, session, invite_link, account_name, workspace_name, storage_backend)?;

    loop {
        terminal
            .draw(|f| render::render(f, &app))
            .map_err(|e| lib::error::Error::Other(e.to_string()))?;

        // Flush pending async results (persists, etc.)
        app.tick().await;

        // Poll for events with a short timeout so tick() runs regularly
        if event::poll(Duration::from_millis(50))
            .map_err(|e| lib::error::Error::Other(e.to_string()))?
        {
            if let Event::Key(key) = event::read()
                .map_err(|e| lib::error::Error::Other(e.to_string()))?
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
