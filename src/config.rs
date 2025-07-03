use serde::Deserialize;

/// Holds settings loaded from the configuration file.
#[derive(Deserialize, Default)]
pub struct Settings {
    pub client_id: String,
    pub enable_debug_log: Option<bool>,
    pub refresh_interval_minutes: Option<u64>,
}

/// Loads settings from the user's config directory.
/// (e.g., ~/.config/365cal-tui/Settings.toml on Linux)
pub fn load_config() -> Result<Settings, config::ConfigError> {
    let config_dir = dirs::config_dir()
        .ok_or_else(|| config::ConfigError::Message("Could not find the config directory.".into()))?;
    
    // Path is based on the new application name
    let config_path = config_dir.join("365cal-tui/Settings.toml");

    let settings = config::Config::builder()
        .add_source(config::File::from(config_path).required(true))
        .build()?;
        
    settings.try_deserialize()
}