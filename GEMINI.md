# Project Context: 365cal-tui

## 1. Project Goal & Origin

The primary goal of this project is to create a fast, modern, and keyboard-driven terminal user interface (TUI) application to view Microsoft 365 / Outlook calendar events. The application is named `365cal-tui`.

This entire project was developed iteratively through a "Vibe Coding" session, where all code was generated and refined in a collaborative conversation with the Gemini 1.5 Pro AI model. The origin was a simple request to read an `.ics` file, which evolved into a full-featured, authenticated client for the Microsoft Graph API.

## 2. Core Features

- **Modern TUI:** Built with Rust 🦀 and `ratatui` for a snappy, responsive terminal interface.
- **Secure Microsoft 365 Login:** Uses the standard OAuth2 Authorization Code Flow with PKCE.
- **Persistent Sessions:** Securely stores the refresh token in the native OS keyring (`keyring` crate), enabling automatic login on subsequent runs.
- **Automatic Token Refresh:** Detects expired access tokens (`401 Unauthorized`) and automatically uses the refresh token to get a new one, providing a seamless user experience.
- **Multi-Calendar Support:**
  - Fetches and lists all of the user's calendars.
  - Provides a special "All Calendars" view to see an aggregated timeline of events.
  - Assigns a unique color to each calendar for easy visual distinction.
  - Displays a dynamic color legend when in the "All Calendars" view.
- **Multiple Event Views:**
  - **List View:** A classic, dense list of events for the selected time period.
  - **Month View:** A traditional grid-based monthly calendar that shows colored event indicators.
  - **Week View:** A 7-day (Sun-Sat) view showing event times and subjects.
  - **Work Week View:** A 5-day (Mon-Fri) view.
- **Seamless Navigation:**
  - `Tab` key cycles through List, Month, Week, and Work Week views.
  - `←`/`→` keys navigate between months or weeks depending on the current view.
  - `↑`/`↓` keys for list selection.
  - `Enter` to select a calendar or view event details.
  - `'b'` to go back.
- **Polished UI & UX:**
  - A live clock and date display in the header.
  - A beautiful and consistent [Catppuccin Mocha](https://github.com/catppuccin) color theme.
  - [Nerd Font](https://www.nerdfonts.com/) glyphs for modern icons.
  - A scrollable popup for viewing full event details (subject, time, attendees, description).
  - A smooth "dissolve" transition effect when switching between views.
- **Configurable & Smart:**
  - Auto-refreshes events periodically; interval is configurable in `Settings.toml`.
  - Manual refresh key (`r`).
  - External configuration file for the Azure `client_id`.
  - Optional debug logging to a file (`365cal-tui.log`), activated by a command-line flag (`-d`) or a config setting.

## 3. Core Technologies & Dependencies

- **Language:** Rust (2021 Edition)
- **TUI Framework:** `ratatui` with `crossterm`.
- **Asynchronous Runtime:** `tokio`.
- **Authentication:** `oauth2`, `keyring`.
- **HTTP Client:** `reqwest`.
- **Configuration:** `config`, `dirs`.
- **Date & Time:** `chrono`.
- **Concurrency:** `futures` (for parallel API calls).
- **CLI Parsing:** `clap`.
- **Logging:** `log`, `simple_logging`.
- **Utilities:** `regex`, `webbrowser`.

## 4. File Structure

The project is organized into a modular structure within the `src/` directory:

```.
├── src/
│ ├── main.rs # Entry point, orchestrates all modules
│ ├── config.rs # Handles loading Settings.toml
│ ├── auth.rs # Manages the entire OAuth2 and token keyring logic
│ ├── api.rs # Contains data models and functions to call the MS Graph API
│ ├── app.rs # Defines the application's state (struct App, enums)
│ ├── tui.rs # The "controller": handles the main event loop and user input
│ └── ui.rs # The "view": handles all rendering logic with ratatui
└── Cargo.toml
```

## 5. Module Breakdown

- `main.rs`: Parses CLI arguments, initializes logging, loads config, triggers authentication, spawns the auto-refresh task, and starts the TUI event loop.
- `config.rs`: Defines the `Settings` struct and the logic to find and parse `~/.config/365cal-tui/Settings.toml`.
- `auth.rs`: Contains the `authenticate()` function which encapsulates the entire login flow.
- `api.rs`: Defines the Rust structs that match the JSON responses from the Microsoft Graph API. Contains the `async` functions `Calendar` and `list_events`, with logic to handle API pagination.
- `app.rs`: The heart of the state management. Defines the main `App` struct and crucial enums like `CurrentView` and `EventViewMode`. Holds all data and state used by the UI.
- `tui.rs`: Contains the main `run_app` async function. Its `loop` listens for keyboard events and internal `AppEvent` messages, calling methods on the `App` struct to update the state and trigger data refreshes.
- `ui.rs`: Contains all rendering logic. The main `ui` function acts as a router, calling specific `draw_*` functions based on the current application state. It also holds the `Theme` definition.

## 6. Key State Management and Logic

The core state is managed by the `App` struct and several key enums in `app.rs`.

```rust
// Key enums defining the application's state
pub enum CurrentView { Calendars, Events, EventDetail }
pub enum EventViewMode { List, Month, Week, WorkWeek }

// The main state struct
pub struct App {
    pub client_id: String,
    pub access_token: String,
    pub calendars: Vec<ColorCalendar>,
    pub events: Vec<ColorEvent>,

    // UI list states
    pub calendar_list_state: ListState,
    pub event_list_state: ListState,

    // View and navigation state
    pub current_view: CurrentView,
    pub event_view_mode: EventViewMode,
    pub current_calendar_id: Option<String>, // None means "All Calendars"
    pub detail_view_scroll: u16,
    pub displayed_date: NaiveDate, // The anchor date for month/week views

    // Animation state
    pub transition: Option<Transition>,
}

7. Configuration

The application requires a configuration file located at: ~/.config/365cal-tui/Settings.toml

Example Settings.toml:
Ini, TOML

client_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
refresh_interval_minutes = 15
enable_debug_log = false
```
