use crate::{
    api::list_events,
    app::{App, ColorEvent, CurrentView, EventViewMode, MY_CALENDARS_ID},
    ui::ui,
    AppEvent,
};
use chrono::{
    DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDate, NaiveDateTime, Utc, Weekday,
};
use crossterm::event::{self, Event as CEvent, KeyCode, MouseButton, MouseEventKind};
use futures::future::join_all;
use log::{error, info, warn};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout, Rect},
    terminal::Terminal,
};
use unicode_width::UnicodeWidthStr;
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

/// Asynchronously fetches events and handles token refresh logic.
async fn refresh_events(app: &mut App, tx: mpsc::Sender<AppEvent>) {
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
    
    // 1. Load from DB (Instant)
    let mut all_events = Vec::new();
    for cal in &calendars_to_fetch {
        if let Ok(events) = crate::db::get_events(&app.db_pool, &cal.calendar.id).await {
             let color = cal.color;
             all_events.extend(events.into_iter().map(|event| ColorEvent { event, color }));
        }
    }
    
    if !all_events.is_empty() {
        all_events.sort_by(|a, b| a.event.start.date_time.cmp(&b.event.start.date_time));
        app.events = all_events;
        if app.event_list_state.selected().is_none() {
            app.event_list_state.select(Some(0));
        }
    }

    info!(
        "Refreshing events for {} calendars...",
        calendars_to_fetch.len()
    );

    // 2. Spawn API Fetch (Background)
    let access_token = app.access_token.clone();
    let db_pool = app.db_pool.clone();
    let calendars = calendars_to_fetch;
    let tx_clone = tx.clone();
    
    tokio::spawn(async move {
        let mut futures = Vec::new();
        for color_cal in &calendars {
            futures.push(list_events(
                &access_token,
                &color_cal.calendar.id,
                start_date,
                end_date,
            ));
        }
        let results = join_all(futures).await;
        
        let needs_token_refresh = results.iter().any(|res| {
            if let Err(err) = res {
                if let Some(req_err) = err.downcast_ref::<reqwest::Error>() {
                    return req_err.status() == Some(reqwest::StatusCode::UNAUTHORIZED);
                }
            }
            false
        });
        
        if needs_token_refresh {
            warn!("Access token expired. Requesting refresh.");
            let _ = tx_clone.send(AppEvent::TokenExpired).await;
            return;
        }
        
        let mut fetched_events = Vec::new();
        for (i, result) in results.into_iter().enumerate() {
            match result {
                Ok(events) => {
                    if let Err(e) = crate::db::save_events_with_range(&db_pool, &events, &calendars[i].calendar.id, &start_date, &end_date).await {
                        error!("Failed to save events to DB: {}", e);
                    }
                    let color = calendars[i].color;
                    fetched_events.extend(events.into_iter().map(|event| ColorEvent { event, color }));
                }
                Err(e) => error!("Error fetching events: {}", e),
            }
        }
        
        let _ = tx_clone.send(AppEvent::EventsLoaded(fetched_events)).await;
    });
}

/// The main application loop. Handles events and updates the app state.
pub async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    mut rx: mpsc::Receiver<AppEvent>,
    tx: mpsc::Sender<AppEvent>,
) -> io::Result<()> {
    let theme = app.theme.clone();

    if !app.calendars.is_empty() {
        refresh_events(app, tx.clone()).await;
    }

    app.start_transition(500);

    let mut last_notification_check = std::time::Instant::now();

    loop {
        terminal.draw(|f| ui(f, app, &theme))?;

        // Check notifications every minute
        if last_notification_check.elapsed() >= Duration::from_secs(60) {
            let events: Vec<_> = app.events.iter().map(|e| e.event.clone()).collect();
            app.notification_manager.check_and_notify(&events);
            last_notification_check = std::time::Instant::now();
        }

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

                    if app.show_legend {
                        match key.code {
                            KeyCode::Esc
                            | KeyCode::Char('q')
                            | KeyCode::Char('l')
                            | KeyCode::Char('L')
                            | KeyCode::Enter => {
                                app.show_legend = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if let KeyCode::Char('?') = key.code {
                        app.show_help = true;
                        continue;
                    }

                    if let KeyCode::Char('l') | KeyCode::Char('L') = key.code {
                        app.show_legend = true;
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
                            KeyCode::Char('b') | KeyCode::Esc => {
                                app.current_view = CurrentView::Calendars;
                                app.event_view_mode = EventViewMode::List;
                                app.displayed_date = Local::now().date_naive();
                                app.start_transition(300);
                            }
                            KeyCode::Char('r') => needs_refresh = true,
                            KeyCode::Tab => {
                                app.toggle_event_view();
                                needs_refresh = true;
                            }
                            KeyCode::Enter => {
                                if app.get_selected_event().is_some() {
                                    app.detail_view_scroll = 0;
                                    app.current_view = CurrentView::EventDetail;
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
                                    EventViewMode::Day => {
                                        app.displayed_date =
                                            app.displayed_date.pred_opt().unwrap();
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
                                    EventViewMode::Day => {
                                        app.displayed_date =
                                            app.displayed_date.succ_opt().unwrap();
                                    }
                                }
                                needs_refresh = true;
                            }
                            KeyCode::Left => app.jump_to_previous_day(),
                            KeyCode::Right => app.jump_to_next_day(),
                            _ => {}
                        },
                        CurrentView::EventDetail => match key.code {
                            KeyCode::Char('q') => return Ok(()),
                            KeyCode::Char('b') | KeyCode::Esc => {
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

                    if app.show_legend {
                        // Click anywhere to close legend
                        if let MouseEventKind::Down(_) = mouse.kind {
                            app.show_legend = false;
                        }
                        continue;
                    }

                    // Handle Event Detail View specific mouse logic (Click outside to close)
                    if let CurrentView::EventDetail = app.current_view {
                        if let MouseEventKind::Down(MouseButton::Left) = mouse.kind {
                            let x = mouse.column;
                            let y = mouse.row;
                            
                            // Re-calculate the centered rect for the popup
                            // This duplicates logic from ui.rs, which is not ideal but necessary for hit testing
                            // unless we store the area in App state.
                            // Let's assume the same 80% logic.
                            let size = terminal.size()?;
                            let percent_x = 80;
                            let percent_y = 80;
                            
                            let popup_layout = Layout::default()
                                .direction(Direction::Vertical)
                                .constraints([
                                    Constraint::Percentage((100 - percent_y) / 2),
                                    Constraint::Percentage(percent_y),
                                    Constraint::Percentage((100 - percent_y) / 2),
                                ])
                                .split(size);

                            let popup_area = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Percentage((100 - percent_x) / 2),
                                    Constraint::Percentage(percent_x),
                                    Constraint::Percentage((100 - percent_x) / 2),
                                ])
                                .split(popup_layout[1])[1];

                            if x < popup_area.left() || x >= popup_area.right() || y < popup_area.top() || y >= popup_area.bottom() {
                                // Clicked outside
                                app.current_view = CurrentView::Events;
                                app.start_transition(300);
                                continue;
                            }
                        }
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

                            // Check for Footer Navigation Click
                            // We need to know where the footer title is.
                            // Since we don't store it in App, we approximate or need to store it.
                            // Storing in App is better. But for now, let's recalculate.
                            let size = terminal.size()?;
                            let main_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .margin(1)
                                .constraints([
                                    Constraint::Length(3), // Header
                                    Constraint::Min(0),    // Content
                                    Constraint::Length(1), // Footer
                                ].as_ref())
                                .split(size);
                            
                            let footer_chunks = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Length(10), // Help
                                    Constraint::Min(0),     // Title
                                    Constraint::Length(20), // Date/Time
                                ])
                                .split(main_chunks[2]);
                            
                            let title_area = footer_chunks[1];
                            if x >= title_area.left() && x < title_area.right() && y >= title_area.top() && y < title_area.bottom() {
                                // Reconstruct the footer title to calculate its width
                                let calendar_name = app.current_calendar_id.as_ref()
                                    .and_then(|id| app.calendars.iter().find(|c| c.calendar.id == *id).map(|c| c.calendar.name.clone()))
                                    .unwrap_or_else(|| "All Calendars".to_string());

                                let footer_text = match app.event_view_mode {
                                    EventViewMode::List => format!(
                                        " {} {} {} ",
                                        app.symbols.left_arrow, calendar_name, app.symbols.right_arrow
                                    ),
                                    EventViewMode::Month => {
                                        let displayed_date = app.displayed_date;
                                        let month_str = format!(
                                            "{} {}",
                                            [
                                                "",
                                                "January",
                                                "February",
                                                "March",
                                                "April",
                                                "May",
                                                "June",
                                                "July",
                                                "August",
                                                "September",
                                                "October",
                                                "November",
                                                "December"
                                            ][displayed_date.month() as usize],
                                            displayed_date.year()
                                        );
                                        format!(
                                            " {} {} - {} {} ",
                                            app.symbols.left_arrow, calendar_name, month_str, app.symbols.right_arrow
                                        )
                                    }
                                    EventViewMode::Week => {
                                        let mut week_start = app.displayed_date;
                                        while week_start.weekday() != Weekday::Sun {
                                            week_start = week_start.pred_opt().unwrap();
                                        }
                                        let week_end = week_start + ChronoDuration::days(6);
                                        format!(
                                            " {} {} ({} to {}) {} ",
                                            app.symbols.left_arrow,
                                            calendar_name,
                                            week_start.format("%d/%m"),
                                            week_end.format("%d/%m"),
                                            app.symbols.right_arrow
                                        )
                                    }
                                    EventViewMode::WorkWeek => {
                                        let mut week_start = app.displayed_date;
                                        while week_start.weekday() != Weekday::Mon {
                                            week_start = week_start.pred_opt().unwrap();
                                        }
                                        let week_end = week_start + ChronoDuration::days(4);
                                        format!(
                                            " {} {} ({} to {}) {} ",
                                            app.symbols.left_arrow,
                                            calendar_name,
                                            week_start.format("%d/%m"),
                                            week_end.format("%d/%m"),
                                            app.symbols.right_arrow
                                        )
                                    }
                                    EventViewMode::Day => {
                                        let current_day = app.displayed_date;
                                        format!(
                                            " {} {} ({}) {} ",
                                            app.symbols.left_arrow,
                                            calendar_name,
                                            current_day.format("%a, %d %b %Y"),
                                            app.symbols.right_arrow
                                        )
                                    }
                                };

                                let text_width = UnicodeWidthStr::width(footer_text.as_str()) as u16;
                                // Footer is Right Aligned
                                let end_x = title_area.right();
                                let start_x = end_x.saturating_sub(text_width);

                                // Check if click is within the text bounds
                                if x >= start_x && x < end_x {
                                    // Clicked on the text. Now check if left or right arrow.
                                    // Left arrow is at the beginning, Right arrow is at the end.
                                    // Let's assume a generous hit area of 4 chars from edges.
                                    if x < start_x + 4 {
                                        // Previous
                                        match app.event_view_mode {
                                            EventViewMode::List | EventViewMode::Month => app.previous_month(),
                                            EventViewMode::Week | EventViewMode::WorkWeek => app.previous_week(),
                                            EventViewMode::Day => { app.displayed_date = app.displayed_date.pred_opt().unwrap(); }
                                        }
                                        needs_refresh = true;
                                    } else if x >= end_x - 4 {
                                        // Next
                                        match app.event_view_mode {
                                            EventViewMode::List | EventViewMode::Month => app.next_month(),
                                            EventViewMode::Week | EventViewMode::WorkWeek => app.next_week(),
                                            EventViewMode::Day => { app.displayed_date = app.displayed_date.succ_opt().unwrap(); }
                                        }
                                        needs_refresh = true;
                                    }
                                }
                            }


                            // Check for Tabs Click
                            // Replicate layout logic to find tabs area
                            let size = terminal.size()?;
                            let main_chunks = Layout::default()
                                .direction(Direction::Vertical)
                                .margin(1)
                                .constraints([Constraint::Length(3), Constraint::Min(0)].as_ref())
                                .split(size);
                            let header_chunks = Layout::default()
                                .direction(Direction::Horizontal)
                                .constraints([
                                    Constraint::Length(68), // Tabs (tuned to 68 to remove extra space)
                                    Constraint::Min(0),     // Title (takes remaining space)
                                ])
                                .split(main_chunks[0]);
                            
                            let tabs_area = header_chunks[0];
                            if x >= tabs_area.left() && x < tabs_area.right() && y >= tabs_area.top() && y < tabs_area.bottom() {
                                // Inside tabs area
                                // Revert to logic compatible with Tabs widget
                                let relative_x = x.saturating_sub(tabs_area.left() + 1); // +1 for left border
                                
                                let calendar_icon = format!(" {} Cals ", app.symbols.calendar);
                                let list_icon = "  List ".to_string();
                                let week_icon = format!(" {} Week ", app.symbols.clock);
                                let work_icon = "  Work ".to_string();
                                let day_icon = "  Day ".to_string();
                                let month_icon = "  Month ".to_string();

                                let tab_data = [
                                    (calendar_icon, CurrentView::Calendars, None),
                                    (list_icon, CurrentView::Events, Some(EventViewMode::List)),
                                    (week_icon, CurrentView::Events, Some(EventViewMode::Week)),
                                    (work_icon, CurrentView::Events, Some(EventViewMode::WorkWeek)),
                                    (day_icon, CurrentView::Events, Some(EventViewMode::Day)),
                                    (month_icon, CurrentView::Events, Some(EventViewMode::Month)),
                                ];

                                let mut current_width_sum = 0;
                                for (text, view, mode) in tab_data.iter() {
                                    // Width = text length + 1 (divider "|")
                                    // The Tabs widget renders: " item1 " | " item2 "
                                    // Our strings already include padding spaces e.g. "  List "
                                    // Ratatui Tabs widget joins them with the divider.
                                    // So the width of an item is its content width + divider width (1), except the last one?
                                    // Actually, let's look at how we calculate it.
                                    // If we are at the first item, it occupies text.width.
                                    // Then a divider.
                                    // Let's approximate: text.width + 3 to be safe and cover spacing.
                                    let width = text.width() as u16 + 3; 
                                    
                                    if relative_x < current_width_sum + width {
                                        // Clicked this tab
                                        app.current_view = *view;
                                        if let Some(m) = mode {
                                            app.event_view_mode = *m;
                                            refresh_events(app, tx.clone()).await;
                                        }
                                        break;
                                    }
                                    current_width_sum += width;
                                }
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
                                        let inner_area =
                                            app.event_list_area.inner(ratatui::layout::Margin {
                                                vertical: 1,
                                                horizontal: 1,
                                            });

                                        // Header is 1 line
                                        let grid_area = Rect {
                                            x: inner_area.x,
                                            y: inner_area.y + 1,
                                            width: inner_area.width,
                                            height: inner_area.height.saturating_sub(1),
                                        };

                                        let row_chunks = Layout::default()
                                            .direction(Direction::Vertical)
                                            .constraints(vec![Constraint::Ratio(1, 6); 6])
                                            .split(grid_area);

                                        if let Some(row) = row_chunks
                                            .iter()
                                            .position(|r| y >= r.top() && y < r.bottom())
                                        {
                                            let row_area = row_chunks[row];
                                            let col_chunks = Layout::default()
                                                .direction(Direction::Horizontal)
                                                .constraints(vec![Constraint::Ratio(1, 7); 7])
                                                .split(row_area);

                                            if let Some(col) = col_chunks
                                                .iter()
                                                .position(|c| x >= c.left() && x < c.right())
                                            {
                                                // Calculate the date
                                                let first_day =
                                                    app.displayed_date.with_day(1).unwrap();
                                                let mut starting_day = first_day;
                                                while starting_day.weekday() != Weekday::Mon {
                                                    starting_day = starting_day.pred_opt().unwrap();
                                                }

                                                let days_offset = (row * 7 + col) as i64;
                                                let clicked_date = starting_day
                                                    + ChronoDuration::days(days_offset);

                                                // Check if we clicked on an event
                                                let local_y = y - row_area.top();

                                                // Filter events for this day
                                                let day_events: Vec<usize> = app
                                                    .events
                                                    .iter()
                                                    .enumerate()
                                                    .filter_map(|(i, e)| {
                                                        if let (Ok(start), Ok(end)) = (
                                                            NaiveDateTime::parse_from_str(
                                                                &e.event.start.date_time,
                                                                "%Y-%m-%dT%H:%M:%S%.f",
                                                            ),
                                                            NaiveDateTime::parse_from_str(
                                                                &e.event.end.date_time,
                                                                "%Y-%m-%dT%H:%M:%S%.f",
                                                            ),
                                                        ) {
                                                            let start_date = start.date();
                                                            let effective_end_date = if end.time() == chrono::NaiveTime::MIN && end.date() > start_date {
                                                                end.date().pred_opt().unwrap()
                                                            } else {
                                                                end.date()
                                                            };

                                                            if start_date <= clicked_date && effective_end_date >= clicked_date {
                                                                return Some(i);
                                                            }
                                                        }
                                                        None
                                                    })
                                                    .collect();

                                                info!("Mouse Click Month: x={}, y={}, local_y={}, date={}", x, y, local_y, clicked_date);

                                                if local_y > 1 {
                                                    let event_visual_index = (local_y - 2) as usize;
                                                    info!("  Checking event index: {} (total events on day: {})", event_visual_index, day_events.len());

                                                    if event_visual_index < day_events.len() {
                                                        let event_index =
                                                            day_events[event_visual_index];
                                                        info!("  Event found! Selecting global index {}", event_index);
                                                        app.event_list_state
                                                            .select(Some(event_index));
                                                        app.detail_view_scroll = 0;
                                                        app.current_view = CurrentView::EventDetail;
                                                        continue;
                                                    }
                                                }

                                                // Switch to List View for this date if clicked on header or empty space
                                                app.displayed_date = clicked_date;
                                                app.event_view_mode = EventViewMode::List;
                                                app.start_transition(300);
                                                needs_refresh = true;
                                            }
                                        }
                                    } else if let EventViewMode::Week
                                        | EventViewMode::WorkWeek = app.event_view_mode
                                        {
                                            // Week/Work Week View Click Logic
                                            let inner_area = app.event_list_area.inner(
                                                ratatui::layout::Margin {
                                                    vertical: 1,
                                                    horizontal: 1,
                                                },
                                            );
                                            if x >= inner_area.left()
                                                && x < inner_area.right()
                                                && y >= inner_area.top()
                                                && y < inner_area.bottom()
                                            {
                                                let num_days = if let EventViewMode::Week =
                                                    app.event_view_mode
                                                {
                                                    7
                                                } else {
                                                    5
                                                };

                                                let col_chunks = Layout::default()
                                                    .direction(Direction::Horizontal)
                                                    .constraints(vec![
                                                        Constraint::Ratio(
                                                            1,
                                                            num_days as u32
                                                        );
                                                        num_days
                                                    ])
                                                    .split(inner_area);

                                                if let Some(col) = col_chunks
                                                    .iter()
                                                    .position(|c| x >= c.left() && x < c.right())
                                                {
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

                                                    // Check if we clicked on an event
                                                    let local_y = y - inner_area.top();

                                                    // Filter events for this day
                                                    let day_events: Vec<(usize, String)> = app
                                                        .events
                                                        .iter()
                                                        .enumerate()
                                                        .filter_map(|(i, e)| {
                                                            if let (Ok(start), Ok(end)) = (
                                                                NaiveDateTime::parse_from_str(
                                                                    &e.event.start.date_time,
                                                                    "%Y-%m-%dT%H:%M:%S%.f",
                                                                ),
                                                                NaiveDateTime::parse_from_str(
                                                                    &e.event.end.date_time,
                                                                    "%Y-%m-%dT%H:%M:%S%.f",
                                                                ),
                                                            ) {
                                                                let start_date = start.date();
                                                                let effective_end_date = if end.time() == chrono::NaiveTime::MIN && end.date() > start_date {
                                                                    end.date().pred_opt().unwrap()
                                                                } else {
                                                                    end.date()
                                                                };

                                                                if start_date <= clicked_date && effective_end_date >= clicked_date {
                                                                    // Reconstruct the event string to calculate height
                                                                    let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start, Utc)
                                                                        .with_timezone(&Local);
                                                                    let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end, Utc)
                                                                        .with_timezone(&Local);
                                                                    
                                                                    let event_str = format!(
                                                                        "■ {}-{} {}",
                                                                        start_local.format("%H:%M"),
                                                                        end_local.format("%H:%M"),
                                                                        e.event.subject
                                                                    );
                                                                    return Some((i, event_str));
                                                                }
                                                            }
                                                            None
                                                        })
                                                        .collect();

                                                    // Get actual column width from the layout chunk
                                                    let col_rect = col_chunks[col];
                                                    let content_width = col_rect.width.saturating_sub(2) as usize;

                                                    if local_y > 0 {
                                                        let content_y = (local_y - 1) as usize; // Adjust for top border
                                                        let mut accumulated_height = 0;
                                                        let mut event_clicked = false;

                                                        for (index, text) in day_events {
                                                            // Calculate wrapped height using word wrapping approximation
                                                            let height = if content_width > 0 {
                                                                let mut lines = 1;
                                                                let mut current_line_len = 0;
                                                                for word in text.split_whitespace() {
                                                                    let word_len = UnicodeWidthStr::width(word);
                                                                    if current_line_len + word_len + (if current_line_len > 0 { 1 } else { 0 }) > content_width {
                                                                        lines += 1;
                                                                        current_line_len = word_len;
                                                                    } else {
                                                                        current_line_len += word_len + (if current_line_len > 0 { 1 } else { 0 });
                                                                    }
                                                                }
                                                                lines
                                                            } else {
                                                                1
                                                            };
                                                            
                                                            if content_y >= accumulated_height && content_y < accumulated_height + height {
                                                                app.event_list_state.select(Some(index));
                                                                app.detail_view_scroll = 0;
                                                                app.current_view = CurrentView::EventDetail;
                                                                event_clicked = true;
                                                                break; 
                                                            }
                                                            accumulated_height += height;
                                                        }

                                                        if !event_clicked {
                                                            // Switch to List View
                                                            app.displayed_date = clicked_date;
                                                            app.event_view_mode = EventViewMode::List;
                                                            app.start_transition(300);
                                                            needs_refresh = true;
                                                        }
                                                    } else {
                                                        // Clicked on header (local_y == 0)
                                                        // Switch to List View
                                                        app.displayed_date = clicked_date;
                                                        app.event_view_mode = EventViewMode::List;
                                                        app.start_transition(300);
                                                        needs_refresh = true;
                                                    }
                                                }
                                            }
                                        } else if let EventViewMode::Day = app.event_view_mode {
                                            // Day View Click Logic
                                            let inner_area = app.event_list_area.inner(
                                                ratatui::layout::Margin {
                                                    vertical: 1,
                                                    horizontal: 1,
                                                },
                                            );
                                            if x >= inner_area.left()
                                                && x < inner_area.right()
                                                && y >= inner_area.top()
                                                && y < inner_area.bottom()
                                            {
                                                let clicked_date = app.displayed_date;
                                                let local_y = y - inner_area.top();

                                                // Filter events for this day
                                                let day_events: Vec<(usize, String)> = app
                                                    .events
                                                    .iter()
                                                    .enumerate()
                                                    .filter_map(|(i, e)| {
                                                        if let (Ok(start), Ok(end)) = (
                                                            NaiveDateTime::parse_from_str(
                                                                &e.event.start.date_time,
                                                                "%Y-%m-%dT%H:%M:%S%.f",
                                                            ),
                                                            NaiveDateTime::parse_from_str(
                                                                &e.event.end.date_time,
                                                                "%Y-%m-%dT%H:%M:%S%.f",
                                                            ),
                                                        ) {
                                                            let start_date = start.date();
                                                            let effective_end_date = if end.time() == chrono::NaiveTime::MIN && end.date() > start_date {
                                                                end.date().pred_opt().unwrap()
                                                            } else {
                                                                end.date()
                                                            };

                                                            if start_date <= clicked_date && effective_end_date >= clicked_date {
                                                                let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start, Utc)
                                                                    .with_timezone(&Local);
                                                                let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end, Utc)
                                                                    .with_timezone(&Local);
                                                                
                                                                let event_str = format!(
                                                                    "■ {}-{} {}",
                                                                    start_local.format("%H:%M"),
                                                                    end_local.format("%H:%M"),
                                                                    e.event.subject
                                                                );
                                                                return Some((i, event_str));
                                                            }
                                                        }
                                                        None
                                                    })
                                                    .collect();

                                                let content_width = inner_area.width as usize;

                                                // No extra top border inside inner_area
                                                let content_y = local_y as usize;
                                                let mut accumulated_height = 0;

                                                    for (index, text) in day_events {
                                                        let height = if content_width > 0 {
                                                            let mut lines = 1;
                                                            let mut current_line_len = 0;
                                                            for word in text.split_whitespace() {
                                                                let word_len = UnicodeWidthStr::width(word);
                                                                if current_line_len + word_len + (if current_line_len > 0 { 1 } else { 0 }) > content_width {
                                                                    lines += 1;
                                                                    current_line_len = word_len;
                                                                } else {
                                                                    current_line_len += word_len + (if current_line_len > 0 { 1 } else { 0 });
                                                                }
                                                            }
                                                            lines
                                                        } else {
                                                            1
                                                        };
                                                        
                                                        if content_y >= accumulated_height && content_y < accumulated_height + height {
                                                            app.event_list_state.select(Some(index));
                                                            app.detail_view_scroll = 0;
                                                            app.current_view = CurrentView::EventDetail;
                                                            continue; 
                                                        }
                                                        accumulated_height += height;
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
                                            EventViewMode::Day => {
                                                app.displayed_date =
                                                    app.displayed_date.succ_opt().unwrap();
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
                                        EventViewMode::Day => {
                                            app.displayed_date =
                                                app.displayed_date.pred_opt().unwrap();
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
                AppEvent::EventsLoaded(mut events) => {
                    events.sort_by(|a, b| a.event.start.date_time.cmp(&b.event.start.date_time));
                    // Check notifications for new events
                    app.notification_manager.check_and_notify(&events.iter().map(|e| e.event.clone()).collect::<Vec<_>>());
                    
                    app.events = events;
                    if !app.events.is_empty() {
                        app.select_nearest_event();
                    } else {
                        app.event_list_state.select(None);
                    }
                }
                AppEvent::TokenExpired => {
                    warn!("Token expired. Refreshing...");
                    if app.refresh_auth_token().await.is_ok() {
                        info!("Token refreshed. Retrying refresh...");
                        // We can't easily trigger refresh here because we are in the loop.
                        // But we can set needs_refresh = true?
                        // No, needs_refresh calls refresh_events which spawns task.
                        // So yes, needs_refresh = true works.
                        needs_refresh = true;
                    } else {
                        error!("Failed to refresh token.");
                    }
                }
            }
        }

        if needs_refresh {
            refresh_events(app, tx.clone()).await;
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
        EventViewMode::Day => {
            let start = app.displayed_date;
            let end = start + ChronoDuration::days(1);
            (to_utc(start), to_utc(end))
        }
    }
}
