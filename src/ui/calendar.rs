use crate::app::App;
use crate::ui::centered_rect;
use crate::ui::Theme;
use chrono::{DateTime, Datelike, Duration as ChronoDuration, Local, NaiveDateTime, Utc, Weekday};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, Wrap},
    Frame,
};

pub fn draw_calendar_list(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    border_color: ratatui::style::Color,
) {
    let mut items: Vec<ListItem> = Vec::new();
    let all_calendars_style = Style::default()
        .fg(theme.foreground)
        .add_modifier(Modifier::BOLD);
    let get_override = |name: &str, default_icon: &str| -> (String, Style) {
        let mut icon = default_icon.to_string();
        let mut style = all_calendars_style;

        if let Some(overrides) = &app.settings.calendar_overrides {
            if let Some(cal_config) = overrides.get(&name.to_lowercase()) {
                if let Some(i) = &cal_config.icon {
                    icon = i.clone();
                }
                if let Some(c) = &cal_config.color {
                    if let Ok(parsed_color) = c.parse::<ratatui::style::Color>() {
                        style = Style::default()
                            .fg(parsed_color)
                            .add_modifier(Modifier::BOLD);
                    }
                }
            }
        }
        (icon, style)
    };

    let (all_icon, all_style) = get_override("All Calendars", "✨");
    items.push(ListItem::new(format!("{} All Calendars", all_icon)).style(all_style));

    let (my_icon, my_style) = get_override("My Calendars", "👤");
    items.push(ListItem::new(format!("{} My Calendars", my_icon)).style(my_style));
    for c in &app.calendars {
        let icon = c.icon.clone().unwrap_or_else(|| "■ ".to_string());
        let line = Line::from(vec![
            Span::styled(icon, Style::default().fg(c.color)),
            Span::raw(format!(" {}", c.calendar.name)),
        ]);
        items.push(ListItem::new(line).style(Style::default().fg(theme.foreground)));
    }
    let items_len = items.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(Style::default().fg(theme.blue).add_modifier(Modifier::BOLD))
        .highlight_symbol("❯ ");
    app.calendar_list_area = area;
    f.render_stateful_widget(list, area, &mut app.calendar_list_state);

    app.calendar_list_scroll_state = app
        .calendar_list_scroll_state
        .content_length(items_len)
        .position(app.calendar_list_state.selected().unwrap_or(0));

    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        area,
        &mut app.calendar_list_scroll_state,
    );
}

pub fn draw_month_view(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    _calendar_name: &str,
    border_color: ratatui::style::Color,
) {
    let today = Local::now().date_naive();
    let displayed_date = app.displayed_date;
    let _month_str = format!(
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
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
    app.event_list_area = area;
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(inner_area);
    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let header_spans: Vec<Span> = weekdays
        .iter()
        .map(|&d| {
            Span::styled(
                format!("{:^width$}", d, width = chunks[0].width as usize / 7),
                Style::default().fg(theme.blue).bold(),
            )
        })
        .collect();
    let header = Line::from(header_spans);
    f.render_widget(Paragraph::new(header), chunks[0]);
    let first_day = displayed_date.with_day(1).unwrap();
    let mut starting_day = first_day;
    while starting_day.weekday() != Weekday::Mon {
        starting_day = starting_day.pred_opt().unwrap();
    }
    let week_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(vec![Constraint::Ratio(1, 6); 6])
        .split(chunks[1]);
    for (week_index, week_area) in week_chunks.iter().enumerate() {
        let day_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(vec![Constraint::Ratio(1, 7); 7])
            .split(*week_area);
        for day_index in 0..7 {
            let current_day =
                starting_day + ChronoDuration::days((week_index * 7 + day_index) as i64);
            if current_day.month() == displayed_date.month() {
                let day_number = current_day.day().to_string();
                let mut day_style = Style::default().fg(theme.foreground);
                if current_day == today {
                    day_style = Style::default().fg(theme.background).bg(theme.blue).bold();
                }
                let mut day_events_text = vec![Line::from(Span::styled(day_number, day_style))];
                for (i, color_event) in app.events.iter().enumerate() {
                    let e = &color_event.event;
                    if let (Ok(start_naive), Ok(end_naive)) = (
                        NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                        NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                    ) {
                        let start_date = start_naive.date();
                        let effective_end_date = if end_naive.time() == chrono::NaiveTime::MIN
                            && end_naive.date() > start_date
                        {
                            end_naive.date().pred_opt().unwrap()
                        } else {
                            end_naive.date()
                        };

                        if start_date <= current_day && effective_end_date >= current_day {
                            let start_local =
                                DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc)
                                    .with_timezone(&Local);
                            let end_local =
                                DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc)
                                    .with_timezone(&Local);

                            let is_selected = Some(i) == app.event_list_state.selected();
                            let style = if is_selected {
                                Style::default()
                                    .fg(theme.background)
                                    .bg(theme.blue)
                                    .add_modifier(Modifier::BOLD)
                            } else {
                                Style::default().fg(color_event.color)
                            };

                            let event_line = Line::from(vec![
                                Span::styled("■ ", style),
                                Span::styled(
                                    format!(
                                        "{}-{}",
                                        start_local.format("%H:%M"),
                                        end_local.format("%H:%M")
                                    ),
                                    if is_selected {
                                        style
                                    } else {
                                        Style::default().fg(theme.foreground)
                                    },
                                ),
                            ]);
                            day_events_text.push(event_line);
                        }
                    }
                }
                let paragraph =
                    Paragraph::new(Text::from(day_events_text).alignment(Alignment::Left)).block(
                        Block::default()
                            .borders(Borders::ALL)
                            .border_style(Style::default().fg(theme.mauve)),
                    );
                f.render_widget(paragraph, day_chunks[day_index]);
            }
        }
    }
}

pub fn draw_week_view(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    _calendar_name: &str,
    border_color: ratatui::style::Color,
) {
    let today = Local::now().date_naive();
    let mut week_start = app.displayed_date;
    while week_start.weekday() != Weekday::Sun {
        week_start = week_start.pred_opt().unwrap();
    }
    let _week_end = week_start + ChronoDuration::days(6);
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
    app.event_list_area = area;
    let day_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Ratio(1, 7); 7])
        .split(inner_area);
    let weekdays = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    for i in 0..7 {
        let day_area = day_chunks[i];
        let current_day = week_start + ChronoDuration::days(i as i64);
        let mut day_style = Style::default().fg(theme.foreground);
        if current_day == today {
            day_style = Style::default().fg(theme.background).bg(theme.blue).bold();
        }
        let title_span = Span::styled(
            format!(" {} {} ", weekdays[i], current_day.day()),
            day_style,
        );
        let mut day_events_text = vec![];
        for (i, color_event) in app.events.iter().enumerate() {
            let e = &color_event.event;
            if let (Ok(start_naive), Ok(end_naive)) = (
                NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
            ) {
                let start_date = start_naive.date();
                let effective_end_date = if end_naive.time() == chrono::NaiveTime::MIN
                    && end_naive.date() > start_date
                {
                    end_naive.date().pred_opt().unwrap()
                } else {
                    end_naive.date()
                };

                if start_date <= current_day && effective_end_date >= current_day {
                    let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc)
                        .with_timezone(&Local);
                    let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc)
                        .with_timezone(&Local);

                    let is_selected = Some(i) == app.event_list_state.selected();
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.background)
                            .bg(theme.blue)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(color_event.color)
                    };

                    let icon = color_event.icon.clone().unwrap_or_else(|| "■ ".to_string());
                    let event_line = Line::from(vec![
                        Span::styled(icon, style),
                        Span::styled(
                            format!(
                                "{}-{} {}",
                                start_local.format("%H:%M"),
                                end_local.format("%H:%M"),
                                e.subject
                            ),
                            if is_selected {
                                style
                            } else {
                                Style::default().fg(theme.foreground)
                            },
                        ),
                    ]);
                    day_events_text.push(event_line);
                }
            }
        }
        let paragraph = Paragraph::new(day_events_text)
            .block(
                Block::default()
                    .title(title_span)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.mauve)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, day_area);
    }
}

pub fn draw_work_week_view(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    _calendar_name: &str,
    border_color: ratatui::style::Color,
) {
    let today = Local::now().date_naive();
    let mut week_start = app.displayed_date;
    while week_start.weekday() != Weekday::Mon {
        week_start = week_start.pred_opt().unwrap();
    }
    let _week_end = week_start + ChronoDuration::days(4);
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
    app.event_list_area = area;
    let day_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints(vec![Constraint::Ratio(1, 5); 5])
        .split(inner_area);
    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri"];
    for i in 0..5 {
        let day_area = day_chunks[i];
        let current_day = week_start + ChronoDuration::days(i as i64);
        let mut day_style = Style::default().fg(theme.foreground);
        if current_day == today {
            day_style = Style::default().fg(theme.background).bg(theme.blue).bold();
        }
        let title_span = Span::styled(
            format!(" {} {} ", weekdays[i], current_day.day()),
            day_style,
        );
        let mut day_events_text = vec![];
        for (i, color_event) in app.events.iter().enumerate() {
            let e = &color_event.event;
            if let (Ok(start_naive), Ok(end_naive)) = (
                NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
            ) {
                let start_date = start_naive.date();
                let effective_end_date = if end_naive.time() == chrono::NaiveTime::MIN
                    && end_naive.date() > start_date
                {
                    end_naive.date().pred_opt().unwrap()
                } else {
                    end_naive.date()
                };

                if start_date <= current_day && effective_end_date >= current_day {
                    let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc)
                        .with_timezone(&Local);
                    let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc)
                        .with_timezone(&Local);

                    let is_selected = Some(i) == app.event_list_state.selected();
                    let style = if is_selected {
                        Style::default()
                            .fg(theme.background)
                            .bg(theme.blue)
                            .add_modifier(Modifier::BOLD)
                    } else {
                        Style::default().fg(color_event.color)
                    };

                    let icon = color_event.icon.clone().unwrap_or_else(|| "■ ".to_string());
                    let event_line = Line::from(vec![
                        Span::styled(icon, style),
                        Span::styled(
                            format!(
                                "{}-{} {}",
                                start_local.format("%H:%M"),
                                end_local.format("%H:%M"),
                                e.subject
                            ),
                            if is_selected {
                                style
                            } else {
                                Style::default().fg(theme.foreground)
                            },
                        ),
                    ]);
                    day_events_text.push(event_line);
                }
            }
        }
        let paragraph = Paragraph::new(day_events_text)
            .block(
                Block::default()
                    .title(title_span)
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.mauve)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, day_area);
    }
}

pub fn draw_day_view(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    _calendar_name: &str,
    border_color: ratatui::style::Color,
) {
    let _today = Local::now().date_naive();
    let current_day = app.displayed_date;
    let main_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
    app.event_list_area = area;

    let mut day_events_text = vec![];
    for (i, color_event) in app.events.iter().enumerate() {
        let e = &color_event.event;
        if let (Ok(start_naive), Ok(end_naive)) = (
            NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
            NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
        ) {
            let start_date = start_naive.date();
            let effective_end_date =
                if end_naive.time() == chrono::NaiveTime::MIN && end_naive.date() > start_date {
                    end_naive.date().pred_opt().unwrap()
                } else {
                    end_naive.date()
                };

            if start_date <= current_day && effective_end_date >= current_day {
                let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc)
                    .with_timezone(&Local);
                let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc)
                    .with_timezone(&Local);

                let is_selected = Some(i) == app.event_list_state.selected();
                let style = if is_selected {
                    Style::default()
                        .fg(theme.background)
                        .bg(theme.blue)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(color_event.color)
                };

                let icon = color_event.icon.clone().unwrap_or_else(|| "■ ".to_string());
                let event_line = Line::from(vec![
                    Span::styled(icon, style),
                    Span::styled(
                        format!(
                            "{}-{} {}",
                            start_local.format("%H:%M"),
                            end_local.format("%H:%M"),
                            e.subject
                        ),
                        if is_selected {
                            style
                        } else {
                            Style::default().fg(theme.foreground)
                        },
                    ),
                ]);
                day_events_text.push(event_line);
            }
        }
    }

    if day_events_text.is_empty() {
        let empty_message = vec![
            Line::from(Span::styled(
                "We don't have anything scheduled for today!",
                Style::default()
                    .fg(theme.green)
                    .add_modifier(Modifier::BOLD),
            )),
            Line::from(""),
            Line::from(Span::styled(
                "🌟  ☕  🎉",
                Style::default().fg(theme.yellow),
            )),
        ];

        let paragraph = Paragraph::new(empty_message)
            .alignment(Alignment::Center)
            .block(Block::default().borders(Borders::NONE));

        let area = centered_rect(60, 20, inner_area);
        f.render_widget(paragraph, area);
    } else {
        let events_list = Paragraph::new(day_events_text)
            .block(Block::default().borders(Borders::NONE))
            .wrap(Wrap { trim: true });
        f.render_widget(events_list, inner_area);
    }
}
