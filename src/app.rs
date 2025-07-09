use crate::api::{GraphCalendar, GraphEvent};
use chrono::{Datelike, Duration, Local, NaiveDate};
use oauth2::{
    basic::BasicClient, reqwest::async_http_client, AuthUrl, ClientId, RedirectUrl,
    TokenResponse, TokenUrl,
};
use ratatui::style::Color;
use ratatui::widgets::ListState;
use std::time::{Duration as StdDuration, Instant};

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
}

/// The main screens of the application.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum CurrentView {
    Calendars,
    Events,
    EventDetail,
}

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
    pub fn new(client_id: String, access_token: String, calendars: Vec<GraphCalendar>) -> Self {
        let mut calendar_list_state = ListState::default();
        calendar_list_state.select(Some(0));
        
        let colors = vec![
            Color::Rgb(203, 166, 247), Color::Rgb(245, 194, 231),
            Color::Rgb(235, 160, 172), Color::Rgb(243, 139, 168),
            Color::Rgb(250, 179, 135), Color::Rgb(249, 226, 175),
            Color::Rgb(166, 227, 161), Color::Rgb(148, 226, 213),
            Color::Rgb(137, 220, 235), Color::Rgb(116, 199, 236),
            Color::Rgb(137, 180, 250), Color::Rgb(180, 190, 254),
        ];

        let color_calendars = calendars
            .into_iter()
            .enumerate()
            .map(|(i, calendar)| ColorCalendar {
                calendar,
                color: colors[i % colors.len()],
            })
            .collect();
        
        Self {
            client_id,
            access_token,
            calendars: color_calendars,
            events: vec![],
            calendar_list_state,
            event_list_state: ListState::default(),
            current_view: CurrentView::Calendars,
            event_view_mode: EventViewMode::List,
            current_calendar_id: None,
            detail_view_scroll: 0,
            displayed_date: Local::now().date_naive(),
            transition: None,
        }
    }

    fn create_oauth_client(&self) -> BasicClient {
        let client_id = ClientId::new(self.client_id.clone());
        let auth_url = AuthUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/authorize".to_string()).unwrap();
        let token_url = Some(TokenUrl::new("https://login.microsoftonline.com/common/oauth2/v2.0/token".to_string()).unwrap());
        let redirect_url = RedirectUrl::new("http://localhost:8080".to_string()).unwrap();
        
        BasicClient::new(client_id, None, auth_url, token_url)
            .set_redirect_uri(redirect_url)
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
            EventViewMode::List => EventViewMode::Month,
            EventViewMode::Month => EventViewMode::Week,
            EventViewMode::Week => EventViewMode::WorkWeek,
            EventViewMode::WorkWeek => EventViewMode::List,
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
            CurrentView::Calendars => (&mut self.calendar_list_state, self.calendars.len() + 1),
            CurrentView::Events => {
                if let EventViewMode::List = self.event_view_mode {
                    (&mut self.event_list_state, self.events.len())
                } else { return; }
            }
            _ => return,
        };
        
        if len == 0 { return; }
        let i = state.selected().map_or(0, |i| (i + 1) % len);
        state.select(Some(i));
    }

    pub fn previous_item(&mut self) {
        let (state, len) = match self.current_view {
            CurrentView::Calendars => (&mut self.calendar_list_state, self.calendars.len() + 1),
            CurrentView::Events => {
                if let EventViewMode::List = self.event_view_mode {
                    (&mut self.event_list_state, self.events.len())
                } else { return; }
            }
            _ => return,
        };
        
        if len == 0 { return; }
        let i = state.selected().map_or(len - 1, |i| (i + len - 1) % len);
        state.select(Some(i));
    }

    pub fn get_selected_event(&self) -> Option<&ColorEvent> {
        if let EventViewMode::List = self.event_view_mode {
            if let Some(index) = self.event_list_state.selected() {
                return self.events.get(index);
            }
        }
        None
    }

    pub fn scroll_down(&mut self) {
        self.detail_view_scroll = self.detail_view_scroll.saturating_add(1);
    }

    pub fn scroll_up(&mut self) {
        self.detail_view_scroll = self.detail_view_scroll.saturating_sub(1);
    }
}
