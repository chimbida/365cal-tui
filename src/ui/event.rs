use crate::app::App;
use crate::ui::Theme;
use chrono::{DateTime, Datelike, Local, NaiveDateTime, Utc};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};

pub fn draw_event_list(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    calendar_name: &str,
) {
    let items: Vec<ListItem> = app
        .events
        .iter()
        .map(|color_event| {
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
                    format!(
                        "{} | {} - {} | {}",
                        local_start.format("%d/%m"),
                        local_start.format("%H:%M"),
                        local_end.format("%H:%M"),
                        e.subject
                    )
                }
                _ => format!("[Invalid Date] | {}", e.subject),
            };
            let line = Line::from(vec![
                Span::styled("■ ", Style::default().fg(color_event.color)),
                Span::raw(line_content),
            ]);
            ListItem::new(line).style(Style::default().fg(theme.foreground))
        })
        .collect();

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
        ][app.displayed_date.month() as usize],
        app.displayed_date.year()
    );
    let title = format!("  Event List for '{}' - {} ", calendar_name, month_str);

    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.mauve))
                .title(title),
        )
        .highlight_style(
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ");
    app.event_list_area = area;
    f.render_stateful_widget(list, area, &mut app.event_list_state);
}

pub fn draw_event_detail_view(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
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
            Span::styled(
                event.subject.clone(),
                Style::default().fg(theme.yellow).bold(),
            ),
        ]));
        let date_format = "%Y-%m-%dT%H:%M:%S%.f";
        let start_naive = NaiveDateTime::parse_from_str(&event.start.date_time, date_format);
        let end_naive = NaiveDateTime::parse_from_str(&event.end.date_time, date_format);
        let time_str = if let (Ok(s), Ok(e)) = (start_naive, end_naive) {
            let start_utc = DateTime::<Utc>::from_naive_utc_and_offset(s, Utc);
            let end_utc = DateTime::<Utc>::from_naive_utc_and_offset(e, Utc);
            let local_start = start_utc.with_timezone(&Local);
            let local_end = end_utc.with_timezone(&Local);
            format!(
                "When: {} from {} to {}",
                local_start.format("%d/%m/%Y"),
                local_start.format("%H:%M"),
                local_end.format("%H:%M")
            )
        } else {
            "When: Invalid time".to_string()
        };
        text.push(Line::from(time_str));
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            " Attendees:",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )));
        if event.attendees.is_empty() {
            text.push(Line::from("None"));
        } else {
            for attendee in &event.attendees {
                if let Some(email) = &attendee.email_address {
                    text.push(Line::from(format!(
                        "  - {} <{}>",
                        email.name, email.address
                    )));
                }
            }
        }
        text.push(Line::from(""));
        text.push(Line::from(Span::styled(
            " Description:",
            Style::default().add_modifier(Modifier::UNDERLINED),
        )));
        if let Some(body) = &event.body {
            if body.content.is_empty() {
                text.push(Line::from("None"));
            } else {
                let width = (area.width as usize).saturating_sub(4); // Margin
                let formatted_content = html2text::from_read(body.content.as_bytes(), width)
                    .unwrap_or_else(|_| body.content.clone());
                for line in formatted_content.lines() {
                    text.push(Line::from(line.to_string()));
                }
            }
        } else {
            text.push(Line::from("None"));
        }
    } else {
        text.push(Line::from("Error: No event selected."));
    }

    let paragraph = Paragraph::new(text)
        .style(Style::default().fg(theme.foreground))
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((app.detail_view_scroll, 0));
    f.render_widget(paragraph, area);
}
