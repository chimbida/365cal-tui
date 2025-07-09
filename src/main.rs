use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{error, info};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

mod app;
mod ui;
mod tui;
mod config;
mod auth;
mod api;

pub enum AppEvent {
    Refresh,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    
    let settings = config::load_config().map_err(|e| {
        println!("ERROR: Could not find or read the configuration file.");
        println!("Please ensure 'Settings.toml' exists at ~/.config/365cal-tui/");
        e
    })?;

    let enable_logging = cli.debug || settings.enable_debug_log.unwrap_or(false);
    if enable_logging {
        simple_logging::log_to_file("365cal-tui.log", log::LevelFilter::Debug)?;
    }
    
    info!("Application started.");

    let (tx, rx) = mpsc::channel(1);
    let refresh_interval_minutes = settings.refresh_interval_minutes.unwrap_or(5);
    let refresh_duration = Duration::from_secs(refresh_interval_minutes * 60);

    tokio::spawn(async move {
        let mut interval = time::interval(refresh_duration);
        loop {
            interval.tick().await;
            info!("Refresh timer triggered. Sending Refresh event.");
            if tx.send(AppEvent::Refresh).await.is_err() {
                break;
            }
        }
    });

    // Guardamos o client_id para passá-lo para o AppState
    let client_id_for_app = settings.client_id.clone();
    let access_token = auth::authenticate(settings.client_id).await?;

    info!("Fetching calendars...");
    let calendars = api::list_calendars(&access_token).await?;
    
    // CORREÇÃO: Passando o client_id para o construtor do App
    let mut app = app::App::new(client_id_for_app, access_token, calendars);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = tui::run_app(&mut terminal, &mut app, rx).await;

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    
    info!("Application terminated.");
    
    if let Err(err) = res {
        error!("Application runtime error: {}", err);
    }

    Ok(())
}