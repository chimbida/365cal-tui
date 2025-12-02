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

#[derive(Deserialize, Serialize, Default)]
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

# Azure Application (Client) ID
# Register your app at https://portal.azure.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade
# Select "Mobile and desktop applications" as the platform and http://localhost:8080 as the redirect URI.
client_id = "YOUR_CLIENT_ID_HERE"

# Refresh interval in minutes (default: 5)
refresh_interval_minutes = 15

# Enable debug logging to 365cal-tui.log (default: false)
enable_debug_log = false

# Theme selection: "catppuccin", "dracula", "gruvbox" (default: "catppuccin")
theme = "catppuccin"

# Font/Symbol set selection: "nerd", "unicode", "ascii" (default: "nerd")
# "nerd" requires a Nerd Font installed.
font = "nerd"

# [custom_themes.my_theme]
# background = "#000000"
# foreground = "#FFFFFF"
# ...

# [custom_fonts.my_font]
# calendar = "C"
# clock = "T"
# help = "?"
# left_arrow = "<"
# right_arrow = ">"
# up_arrow = "^"
# down_arrow = "v"
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
