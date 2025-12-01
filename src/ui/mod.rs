use crate::app::{App, CurrentView, EventViewMode};
use chrono::Local;
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, Widget},
    Frame,
};

pub mod calendar;
pub mod event;

use calendar::{draw_calendar_list, draw_month_view, draw_week_view, draw_work_week_view};
use event::{draw_event_detail_view, draw_event_list};

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
    f.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        f.size(),
    );

    let show_legend = app.current_calendar_id.is_none()
        && matches!(
            app.current_view,
            CurrentView::Events | CurrentView::EventDetail
        );
    let legend_height = if show_legend {
        app.calendars.len() as u16 + 2
    } else {
        0
    };

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3),
                Constraint::Min(0),
                Constraint::Length(legend_height),
            ]
            .as_ref(),
        )
        .split(f.size());

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(22), // Help text
            Constraint::Length(28), // Date/Time
        ])
        .split(main_chunks[0]);

    // Title / Breadcrumbs (Left)
    // For now, just a spacer or we can put the current view name
    let title_paragraph = Paragraph::new(match app.current_view {
        CurrentView::Calendars => "  Calendars",
        CurrentView::Events => "  Events",
        CurrentView::EventDetail => "  Details",
    })
    .style(
        Style::default()
            .fg(theme.mauve)
            .add_modifier(ratatui::style::Modifier::BOLD),
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.mauve)),
    );
    f.render_widget(title_paragraph, header_chunks[0]);

    // Help Text (Middle/Right)
    let help_text = "  Press ? for help ";
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(theme.blue))
        .alignment(Alignment::Center)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.mauve)),
        );
    f.render_widget(help_paragraph, header_chunks[1]);
    app.help_area = header_chunks[1];

    // Date/Time (Right)
    let now = Local::now();
    let datetime_str = format!(
        " {} {} ",
        now.format(" %d/%m/%Y"),
        now.format(" %H:%M:%S")
    );
    let datetime_paragraph = Paragraph::new(datetime_str)
        .style(Style::default().fg(theme.foreground))
        .alignment(Alignment::Right)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.mauve)),
        );
    f.render_widget(datetime_paragraph, header_chunks[2]);

    let content_area = main_chunks[1];
    match app.current_view {
        CurrentView::Calendars => draw_calendar_list(f, app, content_area, theme),
        CurrentView::Events | CurrentView::EventDetail => {
            let calendar_name = app
                .current_calendar_id
                .as_ref()
                .and_then(|id| {
                    if id == crate::app::MY_CALENDARS_ID {
                        Some("My Calendars".to_string())
                    } else {
                        app.calendars
                            .iter()
                            .find(|c| &c.calendar.id == id)
                            .map(|c| c.calendar.name.clone())
                    }
                })
                .unwrap_or_else(|| "All Calendars".to_string());

            match app.event_view_mode {
                EventViewMode::List => draw_event_list(f, app, content_area, theme, &calendar_name),
                EventViewMode::Month => {
                    draw_month_view(f, app, content_area, theme, &calendar_name)
                }
                EventViewMode::Week => draw_week_view(f, app, content_area, theme, &calendar_name),
                EventViewMode::WorkWeek => {
                    draw_work_week_view(f, app, content_area, theme, &calendar_name)
                }
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
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(theme.mauve))
                    .title("  Legend "),
            );
        f.render_widget(legend_paragraph, legend_area);
    }

    if let CurrentView::EventDetail = app.current_view {
        let area = centered_rect(80, 80, f.size());
        draw_event_detail_view(f, app, area, theme);
    }

    if let Some(transition) = &app.transition {
        let progress = transition.start.elapsed().as_secs_f32() / transition.duration.as_secs_f32();
        if progress < 1.0 {
            f.render_widget(DissolveEffect::new(progress), f.size());
        }
    }

    if app.show_help {
        let area = centered_rect(60, 60, f.size());
        draw_help_popup(f, area, theme);
    }
}

fn draw_help_popup(f: &mut Frame, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area);

    let rows = vec![
        Row::new(vec!["Key", "Action"]),
        Row::new(vec!["?", "Toggle Help"]),
        Row::new(vec!["q", "Quit"]),
        Row::new(vec!["r", "Refresh Events"]),
        Row::new(vec!["b", "Back"]),
        Row::new(vec!["Enter", "Select / Details"]),
        Row::new(vec!["Tab", "Cycle Views"]),
        Row::new(vec!["↑/↓", "Navigate List / Scroll"]),
        Row::new(vec!["a/d", "Navigate Month/Week"]),
    ];

    let table = Table::new(
        rows,
        [Constraint::Percentage(30), Constraint::Percentage(70)],
    )
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.mauve))
            .title(" Keyboard Shortcuts "),
    )
    .header(
        Row::new(vec!["Key", "Action"])
            .style(
                Style::default()
                    .fg(theme.yellow)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .bottom_margin(1),
    )
    .column_spacing(1)
    .style(Style::default().fg(theme.foreground));

    f.render_widget(table, area);
}

struct DissolveEffect {
    progress: f32,
}
impl DissolveEffect {
    fn new(progress: f32) -> Self {
        Self { progress }
    }
}
impl Widget for DissolveEffect {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let chars = ['█', '▇', '▆', '▅', '▄', '▃', '▂', ' '];
        let char_count = chars.len() as f32;
        for y in area.top()..area.bottom() {
            for x in area.left()..area.right() {
                let hash = ((x as u32).wrapping_mul(31) ^ (y as u32).wrapping_mul(17)) % 100;
                let hash_f32 = hash as f32 / 100.0;
                if hash_f32 > self.progress {
                    let char_index = ((hash_f32 - self.progress) * char_count).floor() as usize;
                    let char_index = char_index.min(chars.len() - 1);
                    buf.get_mut(x, y).set_char(chars[char_index]);
                }
            }
        }
    }
}
