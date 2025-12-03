use crate::api::{GraphCalendar, GraphEvent};
use chrono::{Datelike, Duration, Local, NaiveDate, NaiveDateTime};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, RedirectUrl, TokenResponse,
    TokenUrl,
};
use ratatui::layout::Rect;
use ratatui::style::Color;
use ratatui::widgets::{ListState, ScrollbarState};
use sqlx::SqlitePool;
use std::time::{Duration as StdDuration, Instant};

pub const MY_CALENDARS_ID: &str = "MY_CALENDARS";

/// Represents the state of a view transition animation.
pub struct Transition {
    pub start: Instant,
    pub duration: StdDuration,
}

/// The different views available for displaying events.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EventViewMode {
    List,
    Month,
    Week,
    WorkWeek,
    Day,
}

/// The main screens of the application.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CurrentView {
    Calendars,
    Events,
    EventDetail,
}

use crate::notifications::NotificationManager;
use crate::ui::{Symbols, Theme};

/// Holds the entire state of the application.
pub struct App {
    pub client_id: String,
    pub access_token: String,
    pub calendars: Vec<ColorCalendar>,
    pub events: Vec<ColorEvent>,
    pub calendar_list_state: ListState,
    pub event_list_state: ListState,
    pub current_view: CurrentView,
    pub event_view_mode: EventViewMode,
    pub current_calendar_id: Option<String>,
    pub detail_view_scroll: u16,
    pub displayed_date: NaiveDate,
    pub transition: Option<Transition>,
    pub calendar_list_area: Rect,
    pub event_list_area: Rect,
    pub help_area: Rect,
    pub show_help: bool,
    pub show_legend: bool,
    pub calendar_list_scroll_state: ScrollbarState,
    pub event_list_scroll_state: ScrollbarState,
    pub detail_scroll_state: ScrollbarState,
    pub db_pool: SqlitePool,
    pub theme: Theme,
    pub symbols: Symbols,
    pub notification_manager: NotificationManager,
}

// CORRECTION: These structs are now public so other modules can use them.
#[derive(Clone)]
pub struct ColorCalendar {
    pub calendar: GraphCalendar,
    pub color: Color,
}

#[derive(Clone)]
pub struct ColorEvent {
    pub event: GraphEvent,
    pub color: Color,
}

impl App {
    pub fn new(
        client_id: String,
        access_token: String,
        db_pool: SqlitePool,
        theme: Theme,
        symbols: Symbols,
        notification_manager: NotificationManager,
    ) -> Self {
        let mut calendar_list_state = ListState::default();
        calendar_list_state.select(Some(0)); // Default select first item

        let mut event_list_state = ListState::default();
        event_list_state.select(None);

        App {
            client_id,
            access_token,
            calendars: Vec::new(),
            events: Vec::new(),
            calendar_list_state,
            event_list_state,
            current_view: CurrentView::Calendars,
            event_view_mode: EventViewMode::List,
            current_calendar_id: None,
            detail_view_scroll: 0,
            displayed_date: Local::now().date_naive(),
            transition: None,
            calendar_list_area: Rect::default(),
            event_list_area: Rect::default(),
            help_area: Rect::default(),
            show_help: false,
            show_legend: false,
            calendar_list_scroll_state: ScrollbarState::default(),
            event_list_scroll_state: ScrollbarState::default(),
            detail_scroll_state: ScrollbarState::default(),
            db_pool,
            theme,
            symbols,
            notification_manager,
        }
    }

    fn create_oauth_client(&self) -> BasicClient {
        let client_id = ClientId::new(self.client_id.clone());
        let auth_url = AuthUrl::new(
            "https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string(),
        )
        .unwrap();
        let token_url = Some(
            TokenUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string())
                .unwrap(),
        );
        let redirect_url = RedirectUrl::new("http://localhost:8080".to_string()).unwrap();

        BasicClient::new(client_id, None, auth_url, token_url).set_redirect_uri(redirect_url)
    }

    pub async fn refresh_auth_token(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        if let Some(refresh_token) = crate::auth::load_refresh_token() {
            let client = self.create_oauth_client();
            let token_result = client
                .exchange_refresh_token(&refresh_token)
                .request_async(async_http_client)
                .await;

            if let Ok(refreshed_token) = token_result {
                self.access_token = refreshed_token.access_token().secret().clone();
                if let Some(new_refresh_token) = refreshed_token.refresh_token() {
                    crate::auth::save_refresh_token(new_refresh_token.secret())?;
                }
                return Ok(());
            }
        }
        Err("Failed to refresh token.".into())
    }

    pub fn start_transition(&mut self, ms: u64) {
        self.transition = Some(Transition {
            start: Instant::now(),
            duration: StdDuration::from_millis(ms),
        });
    }

    pub fn toggle_event_view(&mut self) {
        self.event_view_mode = match self.event_view_mode {
            EventViewMode::List => EventViewMode::Week,
            EventViewMode::Week => EventViewMode::WorkWeek,
            EventViewMode::WorkWeek => EventViewMode::Day,
            EventViewMode::Day => EventViewMode::Month,
            EventViewMode::Month => EventViewMode::List,
        };
        self.start_transition(300);
    }

    pub fn next_month(&mut self) {
        let (year, month) = (self.displayed_date.year(), self.displayed_date.month());
        let new_month = if month == 12 { 1 } else { month + 1 };
        let new_year = if month == 12 { year + 1 } else { year };
        self.displayed_date = NaiveDate::from_ymd_opt(new_year, new_month, 1).unwrap();
    }

    pub fn previous_month(&mut self) {
        let (year, month) = (self.displayed_date.year(), self.displayed_date.month());
        let new_month = if month == 1 { 12 } else { month - 1 };
        let new_year = if month == 1 { year - 1 } else { year };
        self.displayed_date = NaiveDate::from_ymd_opt(new_year, new_month, 1).unwrap();
    }

    pub fn next_week(&mut self) {
        self.displayed_date += Duration::weeks(1);
    }

    pub fn previous_week(&mut self) {
        self.displayed_date -= Duration::weeks(1);
    }

    pub fn next_item(&mut self) {
        let (state, len) = match self.current_view {
            CurrentView::Calendars => (&mut self.calendar_list_state, self.calendars.len() + 2),
            CurrentView::Events => (&mut self.event_list_state, self.events.len()),
            _ => return,
        };

        if len == 0 {
            return;
        }
        let i = state.selected().map_or(0, |i| (i + 1) % len);
        state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        let (state, len) = match self.current_view {
            CurrentView::Calendars => (&mut self.calendar_list_state, self.calendars.len() + 2),
            CurrentView::Events => (&mut self.event_list_state, self.events.len()),
            _ => return,
        };

        if len == 0 {
            return;
        }
        let i = state.selected().map_or(len - 1, |i| (i + len - 1) % len);
        state.select(Some(i));
    }

    pub fn jump_to_next_day(&mut self) {
        if let Some(selected_index) = self.event_list_state.selected() {
            if let Some(current_event) = self.events.get(selected_index) {
                if let Ok(current_start) = NaiveDateTime::parse_from_str(
                    &current_event.event.start.date_time,
                    "%Y-%m-%dT%H:%M:%S%.f",
                ) {
                    let current_date = current_start.date();
                    // Find the first event that is strictly after the current date
                    if let Some(next_index) = self.events.iter().position(|e| {
                        if let Ok(start) = NaiveDateTime::parse_from_str(
                            &e.event.start.date_time,
                            "%Y-%m-%dT%H:%M:%S%.f",
                        ) {
                            start.date() > current_date
                        } else {
                            false
                        }
                    }) {
                        self.event_list_state.select(Some(next_index));
                    }
                }
            }
        }
    }

    pub fn jump_to_previous_day(&mut self) {
        if let Some(selected_index) = self.event_list_state.selected() {
            if let Some(current_event) = self.events.get(selected_index) {
                if let Ok(current_start) = NaiveDateTime::parse_from_str(
                    &current_event.event.start.date_time,
                    "%Y-%m-%dT%H:%M:%S%.f",
                ) {
                    let current_date = current_start.date();
                    // Find the first event of the previous day (or the day before that if none)
                    // We iterate backwards from the current index
                    let mut prev_index = None;
                    for (_i, e) in self.events.iter().enumerate().take(selected_index).rev() {
                        if let Ok(start) = NaiveDateTime::parse_from_str(
                            &e.event.start.date_time,
                            "%Y-%m-%dT%H:%M:%S%.f",
                        ) {
                            if start.date() < current_date {
                                // We found an event on a previous date.
                                // Now we want to find the *first* event of that date.
                                let target_date = start.date();
                                // Search forward from 0 to find the first match for target_date
                                // Optimization: We could search forward from `i` if we knew `i` was the last one,
                                // but `i` is just *one* of them.
                                // Actually, since the list is sorted, the first event of `target_date`
                                // is the first one we encounter when iterating forwards.
                                // So let's just find the first event with `target_date`.
                                if let Some(first_of_day) = self.events.iter().position(|ev| {
                                    if let Ok(s) = NaiveDateTime::parse_from_str(
                                        &ev.event.start.date_time,
                                        "%Y-%m-%dT%H:%M:%S%.f",
                                    ) {
                                        s.date() == target_date
                                    } else {
                                        false
                                    }
                                }) {
                                    prev_index = Some(first_of_day);
                                    break;
                                }
                            }
                        }
                    }
                    if let Some(idx) = prev_index {
                        self.event_list_state.select(Some(idx));
                    }
                }
            }
        }
    }

    pub fn get_selected_event(&self) -> Option<&ColorEvent> {
        if let Some(index) = self.event_list_state.selected() {
            return self.events.get(index);
        }
        None
    }

    pub fn scroll_down(&mut self) {
        self.detail_view_scroll = self.detail_view_scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.detail_view_scroll = self.detail_view_scroll.saturating_sub(1);
    }

    pub fn select_nearest_event(&mut self) {
        if self.events.is_empty() {
            self.event_list_state.select(None);
            return;
        }

        let now = Local::now().naive_local();
        let mut nearest_index = 0;
        let mut min_diff = i64::MAX;

        for (i, color_event) in self.events.iter().enumerate() {
            if let Ok(start) = NaiveDateTime::parse_from_str(
                &color_event.event.start.date_time,
                "%Y-%m-%dT%H:%M:%S%.f",
            ) {
                let diff = start.signed_duration_since(now).num_seconds().abs();
                if diff < min_diff {
                    min_diff = diff;
                    nearest_index = i;
                }
            }
        }

        self.event_list_state.select(Some(nearest_index));

        // Also update displayed_date to match the event
        if let Some(event) = self.events.get(nearest_index) {
            if let Ok(start) =
                NaiveDateTime::parse_from_str(&event.event.start.date_time, "%Y-%m-%dT%H:%M:%S%.f")
            {
                self.displayed_date = start.date();
            }
        }
    }
}
