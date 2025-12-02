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
    pub use_nerd_font: Option<bool>,
    pub custom_themes: Option<HashMap<String, ConfigTheme>>,
    pub symbols: Option<ConfigSymbols>,
}

pub fn get_config_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("365cal-tui/Settings.toml"))
}

pub fn save_default_config() -> Result<(), std::io::Error> {
    if let Some(path) = get_config_path() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        if !path.exists() {
            let default_config = r##"# 365cal-tui Configuration

# Azure Application (Client) ID
# Register your app at https://portal.azure.com/
# Select "Mobile and desktop applications" and add "http://localhost:8080" as Redirect URI.
client_id = "YOUR_CLIENT_ID_HERE"

# Refresh interval in minutes (default: 5)
refresh_interval_minutes = 15

# Enable debug logging to file (default: false)
enable_debug_log = false

# Theme selection: "catppuccin", "dracula", "gruvbox" or a custom theme name (default: "catppuccin")
theme = "catppuccin"

# Use Nerd Font icons (default: true)
use_nerd_font = true

# Custom Themes
[custom_themes.catppuccin]
background = "#1e1e2e"
foreground = "#cdd6f4"
yellow = "#f9e2af"
blue = "#89b4fa"
mauve = "#cba6f7"
green = "#a6e3a1"
red = "#f38ba8"
peach = "#fab387"
teal = "#94e2d5"

[custom_themes.dracula]
background = "#282a36"
foreground = "#f8f8f2"
yellow = "#f1fa8c"
blue = "#8be9fd"
mauve = "#bd93f9"
green = "#50fa7b"
red = "#ff5555"
peach = "#ffb86c"
teal = "#8be9fd"

[custom_themes.gruvbox]
background = "#282828"
foreground = "#ebdbb2"
yellow = "#fabd2f"
blue = "#83a598"
mauve = "#d3869b"
green = "#b8bb26"
red = "#fb4934"
peach = "#fe8019"
teal = "#8ec07c"

# Custom Symbols Example
# [symbols]
# calendar = "📅"
# clock = "🕒"
# help = "?"
# left_arrow = "◄"
# right_arrow = "►"
# up_arrow = "▲"
# down_arrow = "▼"
"##;
            let mut file = fs::File::create(path)?;
            file.write_all(default_config.as_bytes())?;
        }
    }
    Ok(())
}

pub fn load_config() -> Result<Settings, config::ConfigError> {
    // Ensure config exists
    if let Err(e) = save_default_config() {
        eprintln!("Failed to create default config: {}", e);
    }

    let config_dir = dirs::config_dir().ok_or_else(|| {
        config::ConfigError::Message("Could not find the config directory.".into())
    })?;

    let config_path = config_dir.join("365cal-tui/Settings.toml");

    let settings = config::Config::builder()
        .add_source(config::File::from(config_path).required(true))
        .build()?;

    settings.try_deserialize()
}
