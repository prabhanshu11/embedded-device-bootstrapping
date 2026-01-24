//! pibox-tui: Terminal UI client for pibox
//!
//! Vim-style file manager with:
//! - hjkl navigation
//! - Dynamic status bar
//! - Real-time WebSocket updates
//! - Works on Pi Zero 2W (low memory)

mod app;
mod input;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    Terminal,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

use crate::app::{App, AppResult};
use crate::input::handle_key;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing to file (not stdout, would interfere with TUI)
    let log_file = dirs::cache_dir()
        .map(|d| d.join("pibox").join("tui.log"))
        .unwrap_or_else(|| std::path::PathBuf::from("/tmp/pibox-tui.log"));

    if let Some(parent) = log_file.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let file_appender = tracing_appender::rolling::never(
        log_file.parent().unwrap_or(std::path::Path::new("/tmp")),
        log_file.file_name().unwrap_or(std::ffi::OsStr::new("pibox-tui.log")),
    );

    tracing_subscriber::registry()
        .with(EnvFilter::try_from_default_env().unwrap_or_else(|_| "pibox_tui=debug".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(file_appender))
        .init();

    // Load config
    let config = pibox_core::Config::load().unwrap_or_default();

    // Create app
    let mut app = App::new(config);

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run main loop
    let result = run_app(&mut terminal, &mut app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(e) = result {
        eprintln!("Error: {}", e);
        return Err(e);
    }

    Ok(())
}

/// Main application loop
async fn run_app(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>, app: &mut App) -> anyhow::Result<()> {
    loop {
        // Draw UI
        terminal.draw(|f| ui::draw(f, app))?;

        // Poll for events with timeout (allows async tasks to progress)
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                // Check for quit
                if key.code == KeyCode::Char('q') && key.modifiers.is_empty() {
                    if app.state.input_mode == pibox_core::state::InputMode::Normal {
                        return Ok(());
                    }
                }
                if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL) {
                    return Ok(());
                }

                // Handle key input
                match handle_key(app, key).await {
                    AppResult::Continue => {}
                    AppResult::Quit => return Ok(()),
                }
            }
        }

        // Process any pending async operations
        app.tick().await;
    }
}
