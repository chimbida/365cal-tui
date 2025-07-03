use crate::app::{App, CurrentView, EventViewMode}; // Adicionado ColorCalendar, ColorEvent
use chrono::{Datelike, DateTime, Duration as ChronoDuration, Local, NaiveDateTime, Utc, Weekday};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub yellow: Color,
    pub blue: Color,
    pub mauve: Color,
}

impl Theme {
    pub fn catppuccin_mocha() -> Self {
        Self {
            background: Color::Rgb(30, 30, 46),
            foreground: Color::Rgb(205, 214, 244),
            yellow: Color::Rgb(249, 226, 175),
            blue: Color::Rgb(137, 180, 250),
            mauve: Color::Rgb(203, 166, 247),
        }
    }
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

pub fn ui(f: &mut Frame, app: &mut App, theme: &Theme) {
    f.render_widget(Block::default().style(Style::default().bg(theme.background)), f.size());

    let show_legend = app.current_calendar_id.is_none() && matches!(app.current_view, CurrentView::Events | CurrentView::EventDetail);
    let legend_height = if show_legend { app.calendars.len() as u16 + 2 } else { 0 };

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(legend_height),
        ].as_ref())
        .split(f.size());
    
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
        .split(main_chunks[0]);

    let help_text = match app.current_view {
        CurrentView::Calendars => "↑/↓ Nav, Enter Select, q Quit",
        CurrentView::Events => match app.event_view_mode {
            EventViewMode::List => "↑/↓ Nav, ←/→ Month, Enter Details, Tab Month, r Refresh, b Back, q Quit",
            EventViewMode::Month => "←/→ Month, Tab Week, r Refresh, b Back, q Quit",
            EventViewMode::Week => "←/→ Week, Tab Work Week, r Refresh, b Back, q Quit",
            EventViewMode::WorkWeek => "←/→ Week, Tab List, r Refresh, b Back, q Quit",
        },
        CurrentView::EventDetail => "↑/↓ Scroll, b Back, q Quit",
    };
    let help_paragraph = Paragraph::new(format!("  {}", help_text))
        .style(Style::default().fg(theme.foreground))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)));
    f.render_widget(help_paragraph, header_chunks[0]);

    let now = Local::now();
    let datetime_str = format!(" {} {} ", now.format(" %a, %d/%m/%Y"), now.format(" %H:%M:%S"));
    let datetime_paragraph = Paragraph::new(datetime_str)
        .style(Style::default().fg(theme.foreground))
        .alignment(Alignment::Right)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)));
    f.render_widget(datetime_paragraph, header_chunks[1]);
    
    let content_area = main_chunks[1];
    match app.current_view {
        CurrentView::Calendars => draw_calendar_list(f, app, content_area, theme),
        CurrentView::Events | CurrentView::EventDetail => {
            // CORREÇÃO: Copiamos o nome para uma nova String, liberando o 'app'
            let calendar_name = app.current_calendar_id.as_ref()
                .and_then(|id| app.calendars.iter().find(|c| &c.calendar.id == id))
                .map_or("All Calendars".to_string(), |c| c.calendar.name.clone());

            match app.event_view_mode {
                EventViewMode::List => draw_event_list(f, app, content_area, theme, &calendar_name),
                EventViewMode::Month => draw_month_view(f, app, content_area, theme, &calendar_name),
                EventViewMode::Week => draw_week_view(f, app, content_area, theme, &calendar_name),
                EventViewMode::WorkWeek => draw_work_week_view(f, app, content_area, theme, &calendar_name),
            }
        }
    }

    if show_legend {
        let legend_area = main_chunks[2];
        let mut legend_lines: Vec<Line> = Vec::new();
        for color_calendar in &app.calendars {
            let line = Line::from(vec![
                Span::styled("■ ", Style::default().fg(color_calendar.color)),
                Span::raw(color_calendar.calendar.name.clone()),
            ]);
            legend_lines.push(line);
        }
        let legend_paragraph = Paragraph::new(legend_lines)
            .style(Style::default().fg(theme.foreground))
            .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)).title("  Legend "));
        f.render_widget(legend_paragraph, legend_area);
    }

    if let CurrentView::EventDetail = app.current_view {
        let area = centered_rect(80, 80, f.size());
        draw_event_detail_view(f, app, area, theme);
    }
}

fn draw_calendar_list(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    let mut items: Vec<ListItem> = Vec::new();
    let all_calendars_style = Style::default().fg(theme.foreground).add_modifier(Modifier::BOLD);
    items.push(ListItem::new("✨ All Calendars").style(all_calendars_style));
    for c in &app.calendars {
        let line = Line::from(vec![
            Span::styled("■ ", Style::default().fg(c.color)),
            Span::raw(c.calendar.name.clone()),
        ]);
        items.push(ListItem::new(line).style(Style::default().fg(theme.foreground)));
    }
    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)).title("  My Calendars "))
        .highlight_style(Style::default().fg(theme.blue).add_modifier(Modifier::BOLD))
        .highlight_symbol("❯ ");
    f.render_stateful_widget(list, area, &mut app.calendar_list_state);
}

fn draw_event_list(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme, calendar_name: &str) {
    let items: Vec<ListItem> = app.events.iter().map(|color_event| {
        let e = &color_event.event;
        let date_format = "%Y-%m-%dT%H:%M:%S%.f";
        let start_naive = NaiveDateTime::parse_from_str(&e.start.date_time, date_format);
        let end_naive = NaiveDateTime::parse_from_str(&e.end.date_time, date_format);
        let line_content = match (start_naive, end_naive) {
            (Ok(s), Ok(e_dt)) => {
                let start_utc = DateTime::<Utc>::from_naive_utc_and_offset(s, Utc);
                let end_utc = DateTime::<Utc>::from_naive_utc_and_offset(e_dt, Utc);
                let local_start = start_utc.with_timezone(&Local);
                let local_end = end_utc.with_timezone(&Local);
                format!("{} | {} - {} | {}", local_start.format("%d/%m"), local_start.format("%H:%M"), local_end.format("%H:%M"), e.subject)
            }
            _ => format!("[Invalid Date] | {}", e.subject)
        };
        let line = Line::from(vec![
            Span::styled("■ ", Style::default().fg(color_event.color)),
            Span::raw(line_content),
        ]);
        ListItem::new(line).style(Style::default().fg(theme.foreground))
    }).collect();

    let month_str = format!("{} {}", ["", "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][app.displayed_date.month() as usize], app.displayed_date.year());
    let title = format!("  Event List for '{}' - {} ", calendar_name, month_str);

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)).title(title))
        .highlight_style(Style::default().fg(theme.yellow).add_modifier(Modifier::BOLD))
        .highlight_symbol("❯ ");
    f.render_stateful_widget(list, area, &mut app.event_list_state);
}

fn draw_month_view(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme, calendar_name: &str) {
    let today = Local::now().date_naive();
    let displayed_date = app.displayed_date;
    let month_str = format!("{} {}", ["", "January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"][displayed_date.month() as usize], displayed_date.year());
    let title = format!("   Month View for '{}' - {} ", calendar_name, month_str);
    let main_block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme.blue).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.mauve));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(0)].as_ref())
        .split(inner_area);
    let weekdays = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    let header_spans: Vec<Span> = weekdays.iter().map(|&d| Span::styled(format!("{:^width$}", d, width=chunks[0].width as usize / 7), Style::default().fg(theme.blue).bold())).collect();
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
            let current_day = starting_day + ChronoDuration::days((week_index * 7 + day_index) as i64);
            if current_day.month() == displayed_date.month() {
                let day_number = current_day.day().to_string();
                let mut day_style = Style::default().fg(theme.foreground);
                if current_day == today {
                    day_style = Style::default().fg(theme.background).bg(theme.blue).bold();
                }
                let mut day_events_text = vec![Line::from(Span::styled(day_number, day_style))];
                for color_event in &app.events {
                    let e = &color_event.event;
                    if let (Ok(start_naive), Ok(end_naive)) = (
                        NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                        NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f")
                    ) {
                        if start_naive.date() == current_day {
                            let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc).with_timezone(&Local);
                            let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc).with_timezone(&Local);
                            let event_line = Line::from(vec![
                                Span::styled("■ ", Style::default().fg(color_event.color)),
                                Span::raw(format!("{}-{}", start_local.format("%H:%M"), end_local.format("%H:%M"))),
                            ]);
                            day_events_text.push(event_line);
                        }
                    }
                }
                let paragraph = Paragraph::new(Text::from(day_events_text).alignment(Alignment::Left))
                    .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)));
                f.render_widget(paragraph, day_chunks[day_index]);
            }
        }
    }
}

fn draw_week_view(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme, calendar_name: &str) {
    let today = Local::now().date_naive();
    let mut week_start = app.displayed_date;
    while week_start.weekday() != Weekday::Sun {
        week_start = week_start.pred_opt().unwrap();
    }
    let week_end = week_start + ChronoDuration::days(6);
    let title = format!("   Week View for '{}' ({} to {}) ", 
        calendar_name,
        week_start.format("%d/%m"), 
        week_end.format("%d/%m")
    );
    let main_block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme.blue).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.mauve));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
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
        let title_span = Span::styled(format!(" {} {} ", weekdays[i], current_day.day()), day_style);
        let mut day_events_text = vec![];
        for color_event in &app.events {
            let e = &color_event.event;
            if let (Ok(start_naive), Ok(end_naive)) = (
                NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f")
            ) {
                if start_naive.date() == current_day {
                    let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc).with_timezone(&Local);
                    let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc).with_timezone(&Local);
                    let event_line = Line::from(vec![
                        Span::styled("■ ", Style::default().fg(color_event.color)),
                        Span::raw(format!("{}-{} {}", start_local.format("%H:%M"), end_local.format("%H:%M"), e.subject)),
                    ]);
                    day_events_text.push(event_line);
                }
            }
        }
        let paragraph = Paragraph::new(day_events_text)
            .block(Block::default().title(title_span).borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)))
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, day_area);
    }
}

fn draw_work_week_view(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme, calendar_name: &str) {
    let today = Local::now().date_naive();
    let mut week_start = app.displayed_date;
    while week_start.weekday() != Weekday::Mon {
        week_start = week_start.pred_opt().unwrap();
    }
    let week_end = week_start + ChronoDuration::days(4);
    let title = format!("   Work Week for '{}' ({} to {}) ", 
        calendar_name,
        week_start.format("%d/%m"), 
        week_end.format("%d/%m")
    );
    let main_block = Block::default()
        .title(Span::styled(title, Style::default().fg(theme.blue).bold()))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.mauve));
    let inner_area = main_block.inner(area);
    f.render_widget(main_block, area);
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
        let title_span = Span::styled(format!(" {} {} ", weekdays[i], current_day.day()), day_style);
        let mut day_events_text = vec![];
        for color_event in &app.events {
            let e = &color_event.event;
            if let (Ok(start_naive), Ok(end_naive)) = (
                NaiveDateTime::parse_from_str(&e.start.date_time, "%Y-%m-%dT%H:%M:%S%.f"),
                NaiveDateTime::parse_from_str(&e.end.date_time, "%Y-%m-%dT%H:%M:%S%.f")
            ) {
                if start_naive.date() == current_day {
                    let start_local = DateTime::<Utc>::from_naive_utc_and_offset(start_naive, Utc).with_timezone(&Local);
                    let end_local = DateTime::<Utc>::from_naive_utc_and_offset(end_naive, Utc).with_timezone(&Local);
                    let event_line = Line::from(vec![
                        Span::styled("■ ", Style::default().fg(color_event.color)),
                        Span::raw(format!("{}-{} {}", start_local.format("%H:%M"), end_local.format("%H:%M"), e.subject)),
                    ]);
                    day_events_text.push(event_line);
                }
            }
        }
        let paragraph = Paragraph::new(day_events_text)
            .block(Block::default().title(title_span).borders(Borders::ALL).border_style(Style::default().fg(theme.mauve)))
            .wrap(Wrap { trim: true });
        f.render_widget(paragraph, day_area);
    }
}

fn draw_event_detail_view(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area); 
    let block = Block::default()
        .title("  Event Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.mauve))
        .style(Style::default().bg(theme.background));
    let mut text: Vec<Line> = Vec::new();
    if let Some(color_event) = app.get_selected_event() {
        let event = &color_event.event;
        text.push(Line::from(vec![
            Span::styled("■ ", Style::default().fg(color_event.color)),
            Span::styled(event.subject.clone(), Style::default().fg(theme.yellow).bold()),
        ]));
        let date_format = "%Y-%m-%dT%H:%M:%S%.f";
        let start_naive = NaiveDateTime::parse_from_str(&event.start.date_time, date_format);
        let end_naive = NaiveDateTime::parse_from_str(&event.end.date_time, date_format);
        let time_str = if let (Ok(s), Ok(e)) = (start_naive, end_naive) {
            let start_utc = DateTime::<Utc>::from_naive_utc_and_offset(s, Utc);
            let end_utc = DateTime::<Utc>::from_naive_utc_and_offset(e, Utc);
            let local_start = start_utc.with_timezone(&Local);
            let local_end = end_utc.with_timezone(&Local);
            format!("When: {} from {} to {}", local_start.format("%d/%m/%Y"), local_start.format("%H:%M"), local_end.format("%H:%M"))
        } else { "When: Invalid time".to_string() };
        text.push(Line::from(time_str));
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(" Attendees:", Style::default().add_modifier(Modifier::UNDERLINED))));
        if event.attendees.is_empty() {
            text.push(Line::from("None"));
        } else {
            for attendee in &event.attendees {
                if let Some(email) = &attendee.email_address {
                    text.push(Line::from(format!("  - {} <{}>", email.name, email.address)));
                }
            }
        }
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(" Description:", Style::default().add_modifier(Modifier::UNDERLINED))));
        if let Some(body) = &event.body {
            if body.content.is_empty() { text.push(Line::from("None")); } 
            else {
                let cleaned_content = body.content.replace("<br>", "\n").replace("</p>", "\n");
                let re = regex::Regex::new(r"<[^>]*>").unwrap();
                let final_content = re.replace_all(&cleaned_content, "");
                for line in final_content.lines() { text.push(Line::from(line.to_string())); }
            }
        } else { text.push(Line::from("None")); }
    } else { text.push(Line::from("Error: No event selected.")); }

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(theme.foreground))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_view_scroll, 0));
    f.render_widget(paragraph, area);
}