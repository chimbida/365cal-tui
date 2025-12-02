use clap::Parser;
use crossterm::{
    event::{DisableMouseCapture, EnableMouseCapture},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use log::{error, info};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use tokio::sync::mpsc;
use tokio::time::{self, Duration};

mod api;
mod app;
mod auth;
mod config;
mod db;
mod tui;
mod ui;

pub enum AppEvent {
    Refresh,
    EventsLoaded(Vec<app::ColorEvent>),
    TokenExpired,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Cli {
    #[arg(short, long)]
    debug: bool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
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
    let tx_clone = tx.clone();

    tokio::spawn(async move {
        let mut interval = time::interval(refresh_duration);
        loop {
            interval.tick().await;
            info!("Refresh timer triggered. Sending Refresh event.");
            if tx_clone.send(AppEvent::Refresh).await.is_err() {
                break;
            }
        }
    });

    // DB Init
    let config_dir = dirs::config_dir()
        .ok_or("Could not find config directory")?
        .join("365cal-tui");
    std::fs::create_dir_all(&config_dir)?;
    let db_path = config_dir.join("365cal.db");
    // Use mode=rwc to create if missing
    let db_url = format!("sqlite://{}?mode=rwc", db_path.to_string_lossy());

    let db_pool = db::init_db(&db_url).await?;

    // Load calendars from DB
    let mut calendars = db::get_calendars(&db_pool).await?;

    // Guardamos o client_id para passá-lo para o AppState
    let client_id_for_app = settings.client_id.clone();
    let access_token = auth::authenticate(settings.client_id).await?;

    // If DB empty, fetch from API
    if calendars.is_empty() {
        info!("Fetching calendars from API...");
        calendars = api::list_calendars(&access_token).await?;
        db::save_calendars(&db_pool, &calendars).await?;
    }

    // CORREÇÃO: Passando o client_id e db_pool para o construtor do App
    let mut app = app::App::new(client_id_for_app, access_token, calendars, db_pool);

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = tui::run_app(&mut terminal, &mut app, rx, tx).await;

    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    info!("Application terminated.");

    if let Err(err) = res {
        error!("Application runtime error: {}", err);
    }

    Ok(())
}
