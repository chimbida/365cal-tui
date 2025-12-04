use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct ConfigTheme {
    pub background: String,
    pub foreground: String,
    pub yellow: String,
    pub blue: String,
    pub mauve: String,
    pub green: String,
    pub red: String,
    pub peach: String,
    pub teal: String,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct ConfigSymbols {
    pub calendar: Option<String>,
    pub clock: Option<String>,
    pub help: Option<String>,
    pub left_arrow: Option<String>,
    pub right_arrow: Option<String>,
    pub up_arrow: Option<String>,
    pub down_arrow: Option<String>,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct CalendarConfig {
    pub icon: Option<String>,
    pub color: Option<String>,
}

#[derive(Deserialize, Serialize, Default, Clone)]
pub struct Settings {
    pub client_id: String,
    pub enable_debug_log: Option<bool>,
    pub refresh_interval_minutes: Option<u64>,
    pub theme: Option<String>,
    pub font: Option<String>,
    pub use_nerd_font: Option<bool>, // Deprecated, kept for backward compatibility
    pub custom_themes: Option<HashMap<String, ConfigTheme>>,
    pub symbols: Option<ConfigSymbols>,
    pub custom_fonts: Option<HashMap<String, ConfigSymbols>>,
    pub enable_notifications: Option<bool>,
    pub notification_minutes_before: Option<u64>,
    pub calendar_overrides: Option<HashMap<String, CalendarConfig>>,
}

pub fn get_config_dir() -> PathBuf {
    let mut path = dirs::config_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push("365cal-tui");
    path
}

fn save_default_config(config_path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(parent) = config_path.parent() {
        fs::create_dir_all(parent)?;
    }

    if !config_path.exists() {
        let default_config = r##"# 365cal-tui Configuration

# --- Authentication ---
# Azure Application (Client) ID
# Register your app at https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade
# Select "Mobile and desktop applications" as the platform and http://localhost:8080 as the redirect URI.
client_id = "YOUR_CLIENT_ID_HERE"

# --- General ---
# Refresh interval in minutes (default: 5)
refresh_interval_minutes = 15

# Enable debug logging to 365cal-tui.log (default: false)
enable_debug_log = false

# --- Notifications ---
# Enable system notifications (default: true)
enable_notifications = true

# Time in minutes before event to notify (default: 15)
notification_minutes_before = 15

# --- Appearance ---
# Theme selection: "catppuccin", "dracula", "gruvbox" (default: "catppuccin")
theme = "catppuccin"

# Font/Symbol set selection: "nerd", "unicode", "ascii" (default: "nerd")
# "nerd" requires a Nerd Font installed.
font = "nerd"

# --- Customization ---

# [custom_themes.my_theme]
# background = "#1e1e2e"
# foreground = "#cdd6f4"
# yellow = "#f9e2af"
# blue = "#89b4fa"
# mauve = "#cba6f7"
# green = "#a6e3a1"
# red = "#f38ba8"
# peach = "#fab387"
# teal = "#94e2d5"

# Customize specific symbols/icons globally
# [symbols]
# calendar = " "
# clock = " "
# help = ""
# left_arrow = ""
# right_arrow = ""
# up_arrow = ""
# down_arrow = ""

# Define a custom font set (use by setting font = "my_font")
# [custom_fonts.my_font]
# calendar = "C"
# clock = "T"
# help = "?"
# left_arrow = "<"
# right_arrow = ">"
# up_arrow = "^"
# down_arrow = "v"

# Override calendar icon and color by name (Name Match - Case Insensitive)
# [calendar_overrides."My Calendar"]
# icon = "📅"
# color = "#FF0000"

# Override "All Calendars" and "My Calendars"
# [calendar_overrides."All Calendars"]
# icon = "🌎"
# color = "#00FF00"

# [calendar_overrides."My Calendars"]
# icon = "🏠"
# color = "#0000FF"
"##;
        let mut file = fs::File::create(config_path)?;
        file.write_all(default_config.as_bytes())?;
    }
    Ok(())
}

pub fn load_config() -> Result<Settings, config::ConfigError> {
    let config_dir = get_config_dir();
    let config_path = config_dir.join("Settings.toml");

    // Ensure config exists
    if let Err(e) = save_default_config(&config_path) {
        eprintln!("Failed to create default config: {}", e);
    }

    let settings = config::Config::builder()
        .add_source(config::File::from(config_path).required(true))
        .build()?;

    settings.try_deserialize()
}
