# 🚀 365cal-tui 🚀

Welcome to `365cal-tui`, a terminal-based calendar viewer for your Office 365 account, conjured into existence through the power of **Vibe Coding**!

## What is Vibe Coding?

It's what happens when a developer has an idea and uses a friendly AI as a co-pilot to bring it to life, iterating and building features based on conversation and, well, good vibes. This entire application was built from scratch, piece by piece, through a collaborative dialogue with **Google's Gemini 1.5 Pro**.

The goal was simple: create a fast, keyboard-driven, and visually pleasing way to check your Microsoft 365 calendar without ever leaving the terminal.

## ✨ Features

- **Blazing Fast TUI:** Built with Rust 🦀 and `ratatui` for a snappy, responsive terminal interface.
- **Secure Microsoft 365 Login:** Uses the proper OAuth2 flow to connect to your account. Your credentials are never stored!
- **Persistent Sessions:** Saves a secure refresh token in your operating system's native keyring (macOS Keychain, Windows Credential Manager, etc.), so you only have to log in through the browser once.
- **Multiple Calendar Support:**
  - View a list of all your calendars.
  - An "All Calendars" view that aggregates events from all sources.
  - **Color-coded events** to easily distinguish which calendar an event belongs to.
- **Multiple Event Views:**
  - **List View:** A classic, dense list of upcoming events.
  - **Month View:** A traditional grid-based monthly calendar.
  - **Week View:** A 7-day (Sun-Sat) detailed view.
  - **Work Week View:** A 5-day (Mon-Fri) view focused on the work week.
- **Seamless Navigation:**
  - `Tab` key to cycle through List, Month, Week, and Work Week views.
  - `←`/`→` arrow keys to navigate between months or weeks.
  - `↑`/`↓` arrow keys for list selection.
- **Polished UI & UX:**
  - A live clock and date display in the header.
  - Beautiful [Catppuccin Mocha](https://github.com/catppuccin) color theme.
  - Glyphs and icons for a modern look (requires a [Nerd Font](https://www.nerdfonts.com/)).
  - A popup for viewing event details, including description and attendees.
  - Scrollable popups for long event descriptions.
  - Smooth dissolve transitions between views.
- **Configurable & Smart:**
  - Auto-refreshes events periodically (configurable interval).
  - Manual refresh key (`r`).
  - External configuration file for your `client_id`.
  - Debug logging to a file (`365cal-tui.log`).

## 🚀 Getting Started

### Prerequisites

- The [Rust toolchain](https://www.rust-lang.org/tools/install) installed (`rustc`, `cargo`).
- A [Nerd Font](https://www.nerdfonts.com/) installed and set as your terminal's font to see the icons correctly.

### ⚙️ Configuration

This is the most important part! To allow the app to access your calendar, you must register it on Microsoft's platform.

#### Step 1: Azure App Registration

1.  Go to the **Azure Portal**: [https://portal.azure.com/](https://portal.azure.com/)
2.  Navigate to **Azure Active Directory** > **App registrations**.
3.  Click **+ New registration**.
4.  Fill out the form:
    - **Name:** `365cal-tui` (or any name you like).
    - **Supported account types:** Select **"Accounts in any organizational directory (Any Azure AD directory - Multitenant) and personal Microsoft accounts (e.g. Skype, Xbox)"**. This is crucial for it to work.
    - **Redirect URI:**
      - Select the platform **"Mobile and desktop applications"**.
      - Enter `http://localhost:8080` as the redirect URI.
5.  Click **Register**.
6.  On your app's overview page, copy the **`Application (client) ID`**. You will need this for the next step.
7.  Go to **API permissions** in your app's menu:
    - Click **+ Add a permission**, then select **Microsoft Graph**.
    - Choose **Delegated permissions**.
    - Add the following permissions:
      - `offline_access`
      - `openid`
      - `User.Read`
      - `Calendars.Read`
    - Click "Add permissions".

#### Step 2: Create the Config File

1.  Create the application's configuration directory. In your terminal, run:
    ```bash
    mkdir -p ~/.config/365cal-tui
    ```
2.  Create a new file inside that directory named `Settings.toml`:
    ```bash
    # For Linux/macOS
    touch ~/.config/365cal-tui/Settings.toml
    ```
3.  Open this new file and add your **Application (client) ID** that you copied from the Azure portal:

    ```toml
    # ~/.config/365cal-tui/Settings.toml

    client_id = "xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"

    # Optional: time for automatic refresh in minutes (defaults to 5)
    refresh_interval_minutes = 15
    ```

### 🛠️ Building and Running

1.  Clone the repository.
2.  Navigate to the project directory.
3.  To run in development mode:
    ```bash
    cargo run
    ```
4.  To build a final, optimized executable:
    ```bash
    cargo build --release
    ```
    The executable will be located at `./target/release/365cal-tui`. You can copy this file anywhere you like!

## 📦 Dependencies

This project stands on the shoulders of giants. Key dependencies include:

- `ratatui` & `crossterm` for the TUI framework.
- `tokio` for the asynchronous runtime.
- `reqwest` for making HTTP requests to the Graph API.
- `oauth2` for handling the authentication flow.
- `keyring` for securely storing the session token.
- `chrono` for all things date and time.
- `config` & `dirs` for easy configuration.
- `log` & `simple_logging` for file-based logging.

## 📜 License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details. Feel free to use, modify, and distribute it as you wish!
