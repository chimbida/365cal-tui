use crate::{
    api::list_events,
    app::{App, ColorEvent, CurrentView, EventViewMode, MY_CALENDARS_ID},
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
        if id == MY_CALENDARS_ID {
            app.calendars
                .iter()
                .filter(|c| c.calendar.can_share.unwrap_or(false))
                .cloned()
                .collect()
        } else {
            app.calendars
                .iter()
                .filter(|c| c.calendar.id == *id)
                .cloned()
                .collect()
        }
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

                    if app.show_help {
                        match key.code {
                            KeyCode::Esc
                            | KeyCode::Char('q')
                            | KeyCode::Char('?')
                            | KeyCode::Enter => {
                                app.show_help = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if let KeyCode::Char('?') = key.code {
                        app.show_help = true;
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
                                    } else if selected == 1 {
                                        app.current_calendar_id = Some(MY_CALENDARS_ID.to_string());
                                    } else {
                                        let calendar_index = selected - 2;
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
                            KeyCode::Char('a') => {
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
                            KeyCode::Char('d') => {
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
                    if app.show_help {
                        // Click anywhere to close help
                        if let MouseEventKind::Down(_) = mouse.kind {
                            app.show_help = false;
                        }
                        continue;
                    }

                    match mouse.kind {
                        MouseEventKind::Down(MouseButton::Left) => {
                            let x = mouse.column;
                            let y = mouse.row;

                            // Check for Help Click
                            let help_area = app.help_area;
                            if x >= help_area.left()
                                && x < help_area.right()
                                && y >= help_area.top()
                                && y < help_area.bottom()
                            {
                                app.show_help = true;
                                continue;
                            }

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
                                            if index < app.calendars.len() + 2 {
                                                app.calendar_list_state.select(Some(index));
                                                // Trigger selection action
                                                if index == 0 {
                                                    app.current_calendar_id = None;
                                                } else if index == 1 {
                                                    app.current_calendar_id =
                                                        Some(MY_CALENDARS_ID.to_string());
                                                } else {
                                                    let calendar_index = index - 2;
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
                                                let visual_index = (y - area.top() - 1) as usize;
                                                let offset = app.event_list_state.offset();
                                                let index = offset + visual_index;

                                                if index < app.events.len() {
                                                    app.event_list_state.select(Some(index));
                                                    app.detail_view_scroll = 0;
                                                    app.current_view = CurrentView::EventDetail;
                                                }
                                            }
                                        }
                                    } else if let EventViewMode::Month = app.event_view_mode {
                                        // Month View Click Logic
                                        // We need to replicate the layout logic from draw_month_view
                                        // Main block borders
                                        let inner_area =
                                            app.event_list_area.inner(ratatui::layout::Margin {
                                                vertical: 1,
                                                horizontal: 1,
                                            });
                                        if x >= inner_area.left()
                                            && x < inner_area.right()
                                            && y >= inner_area.top()
                                            && y < inner_area.bottom()
                                        {
                                            // Header is 1 line
                                            let grid_area_top = inner_area.top() + 1;
                                            if y >= grid_area_top {
                                                let grid_height =
                                                    inner_area.height.saturating_sub(1);
                                                let grid_width = inner_area.width;

                                                // 6 rows, 7 columns
                                                let row_height = grid_height / 6;
                                                let col_width = grid_width / 7;

                                                if row_height > 0 && col_width > 0 {
                                                    let row = (y - grid_area_top) / row_height;
                                                    let col = (x - inner_area.left()) / col_width;

                                                    if row < 6 && col < 7 {
                                                        // Calculate the date
                                                        let first_day =
                                                            app.displayed_date.with_day(1).unwrap();
                                                        let mut starting_day = first_day;
                                                        while starting_day.weekday() != Weekday::Mon
                                                        {
                                                            starting_day =
                                                                starting_day.pred_opt().unwrap();
                                                        }

                                                        let days_offset = (row * 7 + col) as i64;
                                                        let clicked_date = starting_day
                                                            + ChronoDuration::days(days_offset);

                                                        // Switch to List View for this date
                                                        app.displayed_date = clicked_date;
                                                        app.event_view_mode = EventViewMode::List;
                                                        app.start_transition(300);
                                                        needs_refresh = true;
                                                    }
                                                }
                                            }
                                        }
                                    } else if let EventViewMode::Week | EventViewMode::WorkWeek =
                                        app.event_view_mode
                                    {
                                        // Week/Work Week View Click Logic
                                        let inner_area =
                                            app.event_list_area.inner(ratatui::layout::Margin {
                                                vertical: 1,
                                                horizontal: 1,
                                            });
                                        if x >= inner_area.left()
                                            && x < inner_area.right()
                                            && y >= inner_area.top()
                                            && y < inner_area.bottom()
                                        {
                                            let num_days =
                                                if let EventViewMode::Week = app.event_view_mode {
                                                    7
                                                } else {
                                                    5
                                                };
                                            let col_width = inner_area.width / num_days as u16;

                                            if col_width > 0 {
                                                let col = (x - inner_area.left()) / col_width;
                                                if col < num_days as u16 {
                                                    // Calculate date
                                                    let mut start_date = app.displayed_date;
                                                    if let EventViewMode::Week = app.event_view_mode
                                                    {
                                                        while start_date.weekday() != Weekday::Sun {
                                                            start_date =
                                                                start_date.pred_opt().unwrap();
                                                        }
                                                    } else {
                                                        while start_date.weekday() != Weekday::Mon {
                                                            start_date =
                                                                start_date.pred_opt().unwrap();
                                                        }
                                                    }

                                                    let clicked_date = start_date
                                                        + ChronoDuration::days(col as i64);

                                                    // Switch to List View
                                                    app.displayed_date = clicked_date;
                                                    app.event_view_mode = EventViewMode::List;
                                                    app.start_transition(300);
                                                    needs_refresh = true;
                                                }
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
