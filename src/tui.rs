use crate::{
    api::list_events,
    app::{App, ColorEvent, CurrentView, EventViewMode},
    ui::{ui, Theme},
    AppEvent,
};
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, Utc, Weekday};
use crossterm::event::{self, Event as CEvent, KeyCode, MouseButton, MouseEventKind};
use futures::future::join_all;
use log::{error, info, warn};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

/// Asynchronously fetches events and handles token refresh logic.
async fn refresh_events(app: &mut App) {
    let calendars_to_fetch = if let Some(id) = &app.current_calendar_id {
        app.calendars
            .iter()
            .filter(|c| c.calendar.id == *id)
            .cloned()
            .collect()
    } else {
        app.calendars.clone()
    };

    let (start_date, end_date) = get_view_date_range(app);
    info!(
        "Refreshing events for {} calendars...",
        calendars_to_fetch.len()
    );

    // --- NEW LOGIC: API Call with Retry-on-Refresh ---
    let mut futures = Vec::new();
    for color_cal in &calendars_to_fetch {
        futures.push(list_events(
            &app.access_token,
            &color_cal.calendar.id,
            start_date,
            end_date,
        ));
    }
    let mut results = join_all(futures).await;

    // Check if any of the results failed due to an authorization error.
    let needs_token_refresh = results.iter().any(|res| {
        if let Err(err) = res {
            if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
                return req_err.status() == Some(reqwest::StatusCode::UNAUTHORIZED);
            }
        }
        false
    });

    // If a token refresh is needed, attempt it and retry the API calls.
    if needs_token_refresh {
        warn!("Access token expired or invalid. Attempting to refresh...");
        if app.refresh_auth_token().await.is_ok() {
            info!("Token refreshed successfully. Retrying API calls...");
            let retry_futures = calendars_to_fetch.iter().map(|color_cal| {
                list_events(
                    &app.access_token,
                    &color_cal.calendar.id,
                    start_date,
                    end_date,
                )
            });
            results = join_all(retry_futures).await;
        } else {
            error!("Failed to refresh token. User might need to log in again.");
            return; // Stop processing if refresh fails.
        }
    }

    // Process the final results.
    let mut all_events: Vec<ColorEvent> = Vec::new();
    for (i, result) in results.into_iter().enumerate() {
        match result {
            Ok(events) => {
                let color = calendars_to_fetch[i].color;
                all_events.extend(events.into_iter().map(|event| ColorEvent { event, color }));
            }
            Err(e) => error!("Error fetching events for a calendar: {}", e),
        }
    }

    all_events.sort_by(|a, b| a.event.start.date_time.cmp(&b.event.start.date_time));
    app.events = all_events;

    if !app.events.is_empty() {
        app.event_list_state.select(Some(0));
    } else {
        app.event_list_state.select(None);
    }
}

/// The main application loop. Handles events and updates the app state.
pub async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut rx: mpsc::Receiver<AppEvent>,
) -> io::Result<()> {
    let theme = Theme::catppuccin_mocha();

    if !app.calendars.is_empty() {
        refresh_events(app).await;
    }

    app.start_transition(500);

    loop {
        terminal.draw(|f| ui(f, app, &theme))?;

        let mut needs_refresh = false;

        let poll_timeout = if app.transition.is_some() {
            Duration::from_millis(16)
        } else {
            Duration::from_millis(250)
        };

        if let Some(transition) = &app.transition {
            if transition.start.elapsed() >= transition.duration {
                app.transition = None;
            }
        }

        if event::poll(poll_timeout)? {
            match event::read()? {
                CEvent::Key(key) => {
                    if app.transition.is_some() {
                        continue;
                    }
                    match app.current_view {
                        CurrentView::Calendars => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Down => app.next_item(),
                            KeyCode::Up => app.previous_item(),
                            KeyCode::Enter => {
                                if let Some(selected) = app.calendar_list_state.selected() {
                                    if selected == 0 {
                                        app.current_calendar_id = None;
                                    } else {
                                        let calendar_index = selected - 1;
                                        app.current_calendar_id =
                                            Some(app.calendars[calendar_index].calendar.id.clone());
                                    }
                                    app.current_view = CurrentView::Events;
                                    app.start_transition(300);
                                    needs_refresh = true;
                                }
                            }
                            _ => {}
                        },
                        CurrentView::Events => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('b') => {
                                app.current_view = CurrentView::Calendars;
                                app.event_view_mode = EventViewMode::List;
                                app.displayed_date = Local::now().date_naive();
                                app.start_transition(300);
                            }
                            KeyCode::Char('r') => needs_refresh = true,
                            KeyCode::Tab => app.toggle_event_view(),
                            KeyCode::Enter => {
                                if let EventViewMode::List = app.event_view_mode {
                                    if app.get_selected_event().is_some() {
                                        app.detail_view_scroll = 0;
                                        app.current_view = CurrentView::EventDetail;
                                    }
                                }
                            }
                            KeyCode::Down => app.next_item(),
                            KeyCode::Up => app.previous_item(),
                            KeyCode::Left => {
                                match app.event_view_mode {
                                    EventViewMode::List | EventViewMode::Month => {
                                        app.previous_month()
                                    }
                                    EventViewMode::Week | EventViewMode::WorkWeek => {
                                        app.previous_week()
                                    }
                                }
                                needs_refresh = true;
                            }
                            KeyCode::Right => {
                                match app.event_view_mode {
                                    EventViewMode::List | EventViewMode::Month => app.next_month(),
                                    EventViewMode::Week | EventViewMode::WorkWeek => {
                                        app.next_week()
                                    }
                                }
                                needs_refresh = true;
                            }
                            _ => {}
                        },
                        CurrentView::EventDetail => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('b') => {
                                app.current_view = CurrentView::Events;
                                app.start_transition(300);
                            }
                            KeyCode::Down => app.scroll_down(),
                            KeyCode::Up => app.scroll_up(),
                            _ => {}
                        },
                    }
                }
                CEvent::Mouse(mouse) => {
                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            let x = mouse.column;
                            let y = mouse.row;
                            match app.current_view {
                                CurrentView::Calendars => {
                                    let area = app.calendar_list_area;
                                    if x >= area.left()
                                        && x < area.right()
                                        && y >= area.top()
                                        && y < area.bottom()
                                    {
                                        // Calculate index relative to the list area
                                        // List usually has a block, so content starts at top+1 if block is present.
                                        // But here we are using the area of the widget which includes the block.
                                        // The List widget renders items inside the block.
                                        // Assuming 1 line border and title.
                                        if y > area.top() && y < area.bottom() - 1 {
                                            let index = (y - area.top() - 1) as usize;
                                            // Adjust for scroll if we had it, but ListState handles selection index.
                                            // Wait, ListState selection is logical index.
                                            // If the list is scrolled, the visual index 0 might be logical index 5.
                                            // We don't easily know the scroll offset of the List widget unless we track it manually or force it.
                                            // For now, let's assume no scrolling or simple scrolling.
                                            // Actually, ListState tracks `offset` (private) but we can set `selected`.
                                            // If we click, we want to select the item at that visual position.
                                            // This is hard without knowing the scroll offset.
                                            // However, for small lists (calendars), it fits on screen.
                                            if index < app.calendars.len() + 1 {
                                                app.calendar_list_state.select(Some(index));
                                                // Trigger selection action
                                                if index == 0 {
                                                    app.current_calendar_id = None;
                                                } else {
                                                    let calendar_index = index - 1;
                                                    app.current_calendar_id = Some(
                                                        app.calendars[calendar_index]
                                                            .calendar
                                                            .id
                                                            .clone(),
                                                    );
                                                }
                                                app.current_view = CurrentView::Events;
                                                app.start_transition(300);
                                                needs_refresh = true;
                                            }
                                        }
                                    }
                                }
                                CurrentView::Events => {
                                    if let EventViewMode::List = app.event_view_mode {
                                        let area = app.event_list_area;
                                        if x >= area.left()
                                            && x < area.right()
                                            && y >= area.top()
                                            && y < area.bottom()
                                        {
                                            if y > area.top() && y < area.bottom() - 1 {
                                                // let relative_y = (y - area.top() - 1) as usize;
                                                // We need to account for the list offset.
                                                // Since we don't have access to the internal offset of the List widget,
                                                // we can only support clicking if the list is not scrolled or if we track offset.
                                                // A simple workaround is to just select the item if it's within bounds of the *data*.
                                                // But if scrolled, we select the wrong item.
                                                // Let's just support clicking the first N items for now or try to guess.
                                                // Better: Use Scroll Up/Down to navigate, and Click to select *currently selected*? No that's weird.
                                                // Let's just implement Scroll for now, and Click for Calendars (which are few).
                                                // For events, maybe just click to open details if we can map it?
                                                // Let's try to map it assuming offset 0 for now, or just don't support click-to-select on long lists yet.
                                                // Actually, we can just use the scroll wheel to change selection!
                                            }
                                        }
                                    }
                                }
                                _ => {}
                            }
                        }
                        MouseEventKind::ScrollDown => {
                            match app.current_view {
                                CurrentView::Calendars => app.next_item(),
                                CurrentView::Events => {
                                    if let EventViewMode::List = app.event_view_mode {
                                        app.next_item();
                                    } else {
                                        // For other views, maybe next month/week?
                                        match app.event_view_mode {
                                            EventViewMode::Month => {
                                                app.next_month();
                                                needs_refresh = true;
                                            }
                                            EventViewMode::Week | EventViewMode::WorkWeek => {
                                                app.next_week();
                                                needs_refresh = true;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                                CurrentView::EventDetail => app.scroll_down(),
                            }
                        }
                        MouseEventKind::ScrollUp => match app.current_view {
                            CurrentView::Calendars => app.previous_item(),
                            CurrentView::Events => {
                                if let EventViewMode::List = app.event_view_mode {
                                    app.previous_item();
                                } else {
                                    match app.event_view_mode {
                                        EventViewMode::Month => {
                                            app.previous_month();
                                            needs_refresh = true;
                                        }
                                        EventViewMode::Week | EventViewMode::WorkWeek => {
                                            app.previous_week();
                                            needs_refresh = true;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            CurrentView::EventDetail => app.scroll_up(),
                        },
                        _ => {}
                    }
                }
                _ => {}
            }
        }

        if let Ok(app_event) = rx.try_recv() {
            match app_event {
                AppEvent::Refresh => {
                    if let CurrentView::Events = app.current_view {
                        info!("Automatic refresh triggered.");
                        needs_refresh = true;
                    }
                }
            }
        }

        if needs_refresh {
            refresh_events(app).await;
        }
    }
}

fn get_view_date_range(app: &App) -> (DateTime<Utc>, DateTime<Utc>) {
    let to_utc = |naive_date: NaiveDate| {
        naive_date
            .and_hms_opt(0, 0, 0)
            .unwrap()
            .and_local_timezone(Utc)
            .unwrap()
    };

    match app.event_view_mode {
        EventViewMode::List | EventViewMode::Month => {
            let y = app.displayed_date.year();
            let m = app.displayed_date.month();
            let start = NaiveDate::from_ymd_opt(y, m, 1).unwrap();
            let next_m = if m == 12 { 1 } else { m + 1 };
            let next_y = if m == 12 { y + 1 } else { y };
            let end = NaiveDate::from_ymd_opt(next_y, next_m, 1).unwrap();
            (to_utc(start), to_utc(end))
        }
        EventViewMode::Week => {
            let mut start = app.displayed_date;
            while start.weekday() != Weekday::Sun {
                start = start.pred_opt().unwrap();
            }
            let end = start + ChronoDuration::days(7);
            (to_utc(start), to_utc(end))
        }
        EventViewMode::WorkWeek => {
            let mut start = app.displayed_date;
            while start.weekday() != Weekday::Mon {
                start = start.pred_opt().unwrap();
            }
            let end = start + ChronoDuration::days(5);
            (to_utc(start), to_utc(end))
        }
    }
}
