mod app;
mod render;

use automerge::AutoCommit;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind, KeyModifiers, KeyboardEnhancementFlags, PushKeyboardEnhancementFlags, PopKeyboardEnhancementFlags},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use lib::{error::Result, store::{Session, Store}};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::time::Duration;

use app::App;

pub async fn run(doc: AutoCommit, store: Store, session: Session, invite_token: String, device_name: String, vault_name: String, storage_backend: String) -> Result<()> {
    enable_raw_mode().map_err(|e| lib::error::Error::Other(e.to_string()))?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::REPORT_EVENT_TYPES)
    ).map_err(|e| lib::error::Error::Other(e.to_string()))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)
        .map_err(|e| lib::error::Error::Other(e.to_string()))?;

    let result = run_app(&mut terminal, doc, store, session, invite_token, device_name, vault_name, storage_backend).await;

    disable_raw_mode().map_err(|e| lib::error::Error::Other(e.to_string()))?;
    execute!(terminal.backend_mut(), PopKeyboardEnhancementFlags, LeaveAlternateScreen)
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
    invite_token: String,
    device_name: String,
    vault_name: String,
    storage_backend: String,
) -> Result<()> {
    let mut app = App::new(doc, store, session, invite_token, device_name, vault_name, storage_backend)?;

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
                // Hold 'v' to reveal secret values
                if key.code == KeyCode::Char('v') {
                    app.show_values = key.kind != KeyEventKind::Release;
                    continue;
                }
                // Ignore key-release events for all other keys
                if key.kind == KeyEventKind::Release {
                    continue;
                }
                if app.handle_key(key).await? {
                    break; // quit
                }
            }
        }
    }

    Ok(())
}
