# Project Context: 365cal-tui

## 1. Project Goal

The primary goal of this project is to create a terminal-based user interface (TUI) application written in Rust to view Microsoft 365 / Outlook calendar events. The application is named `365cal-tui`.

The entire project was developed iteratively through a "Vibe Coding" session, where all code was generated and refined in a conversation with the Gemini 1.5 Pro AI model.

## 2. Core Technologies & Dependencies

- **Language:** Rust (2021 Edition)
- **TUI Framework:** `ratatui` with `crossterm` as the backend.
- **Asynchronous Runtime:** `tokio`.
- **Authentication:** `oauth2` for the OAuth2 flow, and `keyring` for secure, persistent storage of refresh tokens.
- **HTTP Client:** `reqwest` for making calls to the Microsoft Graph API.
- **Configuration:** `config` and `dirs` to load a `Settings.toml` file from the user's standard config directory.
- **Date & Time:** `chrono`.
- **Concurrency:** `futures` for running parallel API calls.
- **Utilities:** `log` & `simple_logging` for file-based logging, `clap` for command-line argument parsing, `regex` for cleaning HTML content, `webbrowser` for opening the login URL.

## 3. File Structure

The project is organized into a modular structure within the `src/` directory:

.
├── src/
│ ├── main.rs # Entry point, orchestrates all modules
│ ├── config.rs # Handles loading Settings.toml
│ ├── auth.rs # Manages the entire OAuth2 and token keyring logic
│ ├── api.rs # Contains data models and functions to call the MS Graph API
│ ├── app.rs # Defines the application's state (struct App, enums)
│ ├── tui.rs # The "controller": handles the main event loop and user input
│ └── ui.rs # The "view": handles all rendering logic with ratatui
└── Cargo.toml

## 4. Module Breakdown

- `main.rs`: Parses command-line arguments, initializes logging, loads config, triggers authentication, and starts the TUI.
- `config.rs`: Defines the `Settings` struct and the logic to find and parse `~/.config/365cal-tui/Settings.toml`.
- `auth.rs`: Contains the `authenticate()` function which encapsulates the entire login flow: attempts to use a refresh token from the OS keyring first, and falls back to a browser-based login if needed.
- `api.rs`: Defines the Rust structs that match the JSON responses from the Microsoft Graph API (e.g., `GraphCalendar`, `GraphEvent`). Contains the `async` functions `Calendar` and `list_events`.
- `app.rs`: The heart of the state management. Defines the main `App` struct and crucial enums like `CurrentView` and `EventViewMode`. Holds all data currently used by the UI.
- `tui.rs`: Contains the main `run_app` async function. It has the primary `loop` that listens for keyboard events and internal `AppEvent` messages (from the refresh timer) and calls methods on the `App` struct to update the state.
- `ui.rs`: Contains all rendering logic. The main `ui` function acts as a router, calling specific `draw_*` functions based on the current application state. It also holds the `Theme` definition and custom widgets.

## 5. Key State Management and Logic

### Application State (`app.rs`)

The core state is managed by the `App` struct and two key enums.

```rust
// In src/app.rs

// Determines which "screen" is visible
pub enum CurrentView {
    Calendars,
    Events,
    EventDetail,
}

// Determines the sub-view when looking at events
#[derive(Clone, Copy)]
pub enum EventViewMode {
    List,
    Month,
    Week,
    WorkWeek,
}

// The main state struct
pub struct App {
    pub access_token: String,
    pub calendars: Vec<ColorCalendar>, // Calendars are paired with a color
    pub events: Vec<ColorEvent>,       // Events inherit the color from their calendar

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
```
