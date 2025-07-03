use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{error, info};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

// Module declarations
mod app;
mod ui;
mod tui;
mod config;
mod auth;
mod api;

// Internal application events for cross-task communication
pub enum AppEvent {
    Refresh,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize logging
    simple_logging::log_to_file("365cal-tui.log", log::LevelFilter::Debug)?;
    info!("Application started.");

    // Load configuration
    let settings = match config::load_config() {
        Ok(s) => s,
        Err(e) => {
            error!("Failed to load configuration file: {}", e);
            println!("ERROR: Could not find or read the configuration file.");
            println!("Please ensure 'Settings.toml' exists at ~/.config/365cal-tui/");
            return Err(e.into());
        }
    };
    
    // Create a channel for the refresh timer
    let (tx, rx) = mpsc::channel(1);
    
    // Get interval from config file or default to 5 minutes
    let refresh_interval_minutes = settings.refresh_interval_minutes.unwrap_or(5);
    let refresh_duration = Duration::from_secs(refresh_interval_minutes * 60);

    // Spawn the timer task in the background
    tokio::spawn(async move {
        let mut interval = time::interval(refresh_duration);
        loop {
            interval.tick().await;
            info!("Refresh timer triggered. Sending Refresh event.");
            if tx.send(AppEvent::Refresh).await.is_err() {
                // If sending fails, the TUI has closed. We can stop the task.
                break;
            }
        }
    });

    // Authenticate user and get token
    let access_token = match auth::authenticate(settings.client_id).await {
        Ok(token) => token,
        Err(e) => {
            error!("Authentication failed: {}", e);
            return Err(e);
        }
    };

    // Fetch initial data
    info!("Fetching calendars...");
    let calendars = api::list_calendars(&access_token).await?;
    
    // Create application state
    let mut app = app::App::new(access_token, calendars);

    // Initialize the terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the main TUI loop, passing the channel receiver
    let res = tui::run_app(&mut terminal, &mut app, rx).await;

    // Restore terminal on exit
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    info!("Application terminated.");
    
    if let Err(err) = res {
        error!("Application runtime error: {}", err);
    }

    Ok(())
}