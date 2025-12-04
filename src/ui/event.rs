use crate::app::App;
use crate::ui::Theme;
use chrono::{DateTime, Datelike, Local, NaiveDateTime, Utc};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, Borders, Clear, List, ListItem, Paragraph, Scrollbar, ScrollbarOrientation, Wrap,
    },
    Frame,
};

pub fn draw_event_list(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &Theme,
    _calendar_name: &str,
    border_color: ratatui::style::Color,
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
            let icon = color_event.icon.clone().unwrap_or_else(|| "■ ".to_string());
            let line = Line::from(vec![
                Span::styled(icon, Style::default().fg(color_event.color)),
                Span::raw(line_content),
            ]);
            ListItem::new(line).style(Style::default().fg(theme.foreground))
        })
        .collect();

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
        ][app.displayed_date.month() as usize],
        app.displayed_date.year()
    );

    let items_len = items.len();
    let list = List::new(items)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(border_color)),
        )
        .highlight_style(
            Style::default()
                .fg(theme.yellow)
                .add_modifier(Modifier::BOLD),
        )
        .highlight_symbol("❯ ");
    app.event_list_area = area;
    f.render_stateful_widget(list, area, &mut app.event_list_state);

    app.event_list_scroll_state = app
        .event_list_scroll_state
        .content_length(items_len)
        .position(app.event_list_state.selected().unwrap_or(0));

    f.render_stateful_widget(
        Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓")),
        area,
        &mut app.event_list_scroll_state,
    );
}

pub fn draw_event_detail_view(f: &mut Frame, app: &mut App, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area);

    // Main Block
    let block = Block::default()
        .title("  Event Details ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.mauve))
        .style(Style::default().bg(theme.background));
    f.render_widget(block.clone(), area);

    // Inner Area (excluding borders)
    let inner_area = block.inner(area);

    if let Some(color_event) = app.get_selected_event() {
        let event = &color_event.event;

        // Layout:
        // Top: Subject (1 line)
        // Row 1: Time (Start/End) | Location
        // Row 2: Organizer | Attendees
        // Bottom: Description (Remaining)

        let chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Vertical)
            .constraints([
                ratatui::layout::Constraint::Length(3), // Subject
                ratatui::layout::Constraint::Length(3), // Time & Location
                ratatui::layout::Constraint::Length(3), // Organizer & Attendees
                ratatui::layout::Constraint::Min(0),    // Description
            ])
            .split(inner_area);

        // --- Subject ---
        let icon = color_event.icon.clone().unwrap_or_else(|| "■ ".to_string());
        let subject_paragraph = Paragraph::new(Line::from(vec![
            Span::styled(icon, Style::default().fg(color_event.color)),
            Span::styled(
                event.subject.clone(),
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Subject ")
                .border_style(Style::default().fg(theme.blue)),
        );
        f.render_widget(subject_paragraph, chunks[0]);

        // --- Row 1: Time & Location ---
        let row1_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(50),
            ])
            .split(chunks[1]);

        // Time
        let date_format = "%Y-%m-%dT%H:%M:%S%.f";
        let start_naive = NaiveDateTime::parse_from_str(&event.start.date_time, date_format);
        let end_naive = NaiveDateTime::parse_from_str(&event.end.date_time, date_format);
        let time_str = if let (Ok(s), Ok(e)) = (start_naive, end_naive) {
            let start_utc = DateTime::<Utc>::from_naive_utc_and_offset(s, Utc);
            let end_utc = DateTime::<Utc>::from_naive_utc_and_offset(e, Utc);
            let local_start = start_utc.with_timezone(&Local);
            let local_end = end_utc.with_timezone(&Local);
            format!(
                "{} {} - {}",
                local_start.format("%d/%m/%Y"),
                local_start.format("%H:%M"),
                local_end.format("%H:%M")
            )
        } else {
            "Invalid time".to_string()
        };

        let time_paragraph = Paragraph::new(time_str).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Time ")
                .border_style(Style::default().fg(theme.green)),
        );
        f.render_widget(time_paragraph, row1_chunks[0]);

        // Location
        let location_str = event
            .location
            .as_ref()
            .map(|l| l.display_name.clone())
            .unwrap_or_else(|| "N/A".to_string());
        let location_paragraph = Paragraph::new(location_str).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Location ")
                .border_style(Style::default().fg(theme.peach)),
        );
        f.render_widget(location_paragraph, row1_chunks[1]);

        // --- Row 2: Organizer & Attendees ---
        let row2_chunks = ratatui::layout::Layout::default()
            .direction(ratatui::layout::Direction::Horizontal)
            .constraints([
                ratatui::layout::Constraint::Percentage(50),
                ratatui::layout::Constraint::Percentage(50),
            ])
            .split(chunks[2]);

        // Organizer
        let organizer_str = event
            .organizer
            .as_ref()
            .map(|o| format!("{} <{}>", o.email_address.name, o.email_address.address))
            .unwrap_or_else(|| "N/A".to_string());
        let organizer_paragraph = Paragraph::new(organizer_str).block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Organizer ")
                .border_style(Style::default().fg(theme.teal)),
        );
        f.render_widget(organizer_paragraph, row2_chunks[0]);

        // Attendees
        let attendees_list: String = if event.attendees.is_empty() {
            "None".to_string()
        } else {
            event
                .attendees
                .iter()
                .map(|a| {
                    a.email_address
                        .as_ref()
                        .map(|e| e.name.as_str())
                        .unwrap_or("?")
                })
                .collect::<Vec<&str>>()
                .join(", ")
        };

        let attendees_paragraph = Paragraph::new(attendees_list)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Attendees ")
                    .border_style(Style::default().fg(theme.mauve)),
            )
            .wrap(Wrap { trim: true });
        f.render_widget(attendees_paragraph, row2_chunks[1]);

        // --- Description ---
        let mut description_text: Vec<Line> = Vec::new();
        if let Some(body) = &event.body {
            if body.content.is_empty() {
                description_text.push(Line::from("None"));
            } else {
                let width = (chunks[3].width as usize).saturating_sub(2); // Margin
                let formatted_content = html2text::from_read(body.content.as_bytes(), width)
                    .unwrap_or_else(|_| body.content.clone());
                for line in formatted_content.lines() {
                    description_text.push(Line::from(line.to_string()));
                }
            }
        } else {
            description_text.push(Line::from("None"));
        }

        let description_len = description_text.len();
        let description_paragraph = Paragraph::new(description_text)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .title(" Description ")
                    .border_style(Style::default().fg(theme.foreground)),
            )
            .wrap(Wrap { trim: false })
            .scroll((app.detail_view_scroll, 0));
        f.render_widget(description_paragraph, chunks[3]);

        // Scrollbar for description
        app.detail_scroll_state = app
            .detail_scroll_state
            .content_length(description_len)
            .position(app.detail_view_scroll as usize);

        f.render_stateful_widget(
            Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓")),
            chunks[3],
            &mut app.detail_scroll_state,
        );
    } else {
        let error_paragraph =
            Paragraph::new("Error: No event selected.").style(Style::default().fg(theme.red));
        f.render_widget(error_paragraph, inner_area);
    }
}
