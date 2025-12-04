use crate::app::{App, CurrentView, EventViewMode};
use chrono::{Datelike, Duration, Local, Weekday};
use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Row, Table, Tabs, Widget},
    Frame,
};

pub mod calendar;
pub mod event;

use calendar::{
    draw_calendar_list, draw_day_view, draw_month_view, draw_week_view, draw_work_week_view,
};
use event::{draw_event_detail_view, draw_event_list};

use crate::config::{ConfigSymbols, ConfigTheme};
use std::collections::HashMap;

#[derive(Clone)]
pub struct Symbols {
    pub calendar: String,
    pub clock: String,
    pub help: String,
    pub left_arrow: String,
    pub right_arrow: String,
    pub up_arrow: String,
    pub down_arrow: String,
}

impl Default for Symbols {
    fn default() -> Self {
        Self::nerd_font()
    }
}

impl Symbols {
    pub fn from_string(name: &str, custom_fonts: &Option<HashMap<String, ConfigSymbols>>) -> Self {
        if let Some(fonts) = custom_fonts {
            if let Some(custom) = fonts.get(name) {
                return Self::from_config(custom);
            }
        }

        match name.to_lowercase().as_str() {
            "unicode" => Self::unicode(),
            "ascii" => Self::ascii(),
            _ => Self::nerd_font(),
        }
    }

    pub fn from_config(config: &ConfigSymbols) -> Self {
        let default = Self::default();
        Self {
            calendar: config.calendar.clone().unwrap_or(default.calendar),
            clock: config.clock.clone().unwrap_or(default.clock),
            help: config.help.clone().unwrap_or(default.help),
            left_arrow: config.left_arrow.clone().unwrap_or(default.left_arrow),
            right_arrow: config.right_arrow.clone().unwrap_or(default.right_arrow),
            up_arrow: config.up_arrow.clone().unwrap_or(default.up_arrow),
            down_arrow: config.down_arrow.clone().unwrap_or(default.down_arrow),
        }
    }

    pub fn nerd_font() -> Self {
        Self {
            calendar: " ".to_string(),
            clock: " ".to_string(),
            help: "".to_string(),
            left_arrow: "".to_string(),
            right_arrow: "".to_string(),
            up_arrow: "".to_string(),
            down_arrow: "".to_string(),
        }
    }

    pub fn unicode() -> Self {
        Self {
            calendar: "📅".to_string(),
            clock: "🕒".to_string(),
            help: "?".to_string(),
            left_arrow: "◄".to_string(),
            right_arrow: "►".to_string(),
            up_arrow: "▲".to_string(),
            down_arrow: "▼".to_string(),
        }
    }

    pub fn ascii() -> Self {
        Self {
            calendar: "[C]".to_string(),
            clock: "[T]".to_string(),
            help: "[?]".to_string(),
            left_arrow: "<".to_string(),
            right_arrow: ">".to_string(),
            up_arrow: "^".to_string(),
            down_arrow: "v".to_string(),
        }
    }
}

#[derive(Clone)]
pub struct Theme {
    pub background: Color,
    pub foreground: Color,
    pub yellow: Color,
    pub blue: Color,
    pub mauve: Color,
    pub green: Color,
    pub red: Color,
    pub peach: Color,
    pub teal: Color,
}

impl Theme {
    pub fn from_string(name: &str, custom_themes: &Option<HashMap<String, ConfigTheme>>) -> Self {
        let name_lower = name.to_lowercase();

        if let Some(themes) = custom_themes {
            if let Some(custom) = themes.get(&name_lower) {
                return Self::from_config(custom);
            }
        }

        // Fallback to default if not found in config
        Self::default()
    }

    pub fn from_config(config: &ConfigTheme) -> Self {
        fn parse_color(s: &str) -> Color {
            if s.starts_with('#') && s.len() == 7 {
                let r = u8::from_str_radix(&s[1..3], 16).unwrap_or(255);
                let g = u8::from_str_radix(&s[3..5], 16).unwrap_or(255);
                let b = u8::from_str_radix(&s[5..7], 16).unwrap_or(255);
                Color::Rgb(r, g, b)
            } else {
                Color::White // Fallback
            }
        }

        Self {
            background: parse_color(&config.background),
            foreground: parse_color(&config.foreground),
            yellow: parse_color(&config.yellow),
            blue: parse_color(&config.blue),
            mauve: parse_color(&config.mauve),
            green: parse_color(&config.green),
            red: parse_color(&config.red),
            peach: parse_color(&config.peach),
            teal: parse_color(&config.teal),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            background: Color::Rgb(30, 30, 46),
            foreground: Color::Rgb(205, 214, 244),
            yellow: Color::Rgb(249, 226, 175),
            blue: Color::Rgb(137, 180, 250),
            mauve: Color::Rgb(203, 166, 247),
            green: Color::Rgb(166, 227, 161),
            red: Color::Rgb(243, 139, 168),
            peach: Color::Rgb(250, 179, 135),
            teal: Color::Rgb(148, 226, 213),
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

    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .margin(1)
        .constraints(
            [
                Constraint::Length(3), // Header
                Constraint::Min(0),    // Content
                Constraint::Length(1), // Footer
            ]
            .as_ref(),
        )
        .split(f.size());

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(68), // Tabs (tuned to 68 to remove extra space)
            Constraint::Min(0),     // Title (takes remaining space)
        ])
        .split(main_chunks[0]);

    // Tabs (Left)
    let selected_index = match app.current_view {
        CurrentView::Calendars => 0,
        CurrentView::Events | CurrentView::EventDetail => match app.event_view_mode {
            EventViewMode::List => 1,
            EventViewMode::Week => 2,
            EventViewMode::WorkWeek => 3,
            EventViewMode::Day => 4,
            EventViewMode::Month => 5,
        },
    };

    let calendar_icon = format!(" {} Cals ", app.symbols.calendar);
    let list_icon = "  List "; // Not configurable yet
    let week_icon = format!(" {} Week ", app.symbols.clock);
    let work_icon = "  Work "; // Not configurable yet
    let day_icon = "  Day "; // Not configurable yet
    let month_icon = "  Month "; // Not configurable yet

    let tab_data = vec![
        (calendar_icon.as_str(), theme.blue),
        (list_icon, theme.green),
        (week_icon.as_str(), theme.yellow),
        (work_icon, theme.peach),
        (day_icon, theme.teal),
        (month_icon, theme.red),
    ];

    let active_color = tab_data[selected_index].1;

    let titles: Vec<Line> = tab_data
        .iter()
        .enumerate()
        .map(|(i, (text, color))| {
            if i == selected_index {
                Line::from(Span::styled(
                    *text,
                    Style::default()
                        .bg(*color)
                        .fg(theme.background)
                        .add_modifier(Modifier::BOLD),
                ))
            } else {
                Line::from(Span::styled(
                    *text,
                    Style::default().fg(*color).add_modifier(Modifier::BOLD),
                ))
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(theme.mauve)),
        )
        .select(selected_index)
        .style(Style::default().fg(theme.foreground))
        .highlight_style(
            Style::default()
                .add_modifier(Modifier::BOLD)
                .bg(active_color)
                .fg(theme.background),
        )
        .divider(Span::raw("|"));
    f.render_widget(tabs, header_chunks[0]);

    // Window Title (Moved to Footer)

    // Footer Layout
    let footer_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(10), // Help
            Constraint::Min(0),     // Title
            Constraint::Length(20), // Date/Time
        ])
        .split(main_chunks[2]);

    // Help Text (Footer Left)
    let help_text = format!(" {} Help ", app.symbols.help);
    let help_paragraph = Paragraph::new(help_text)
        .style(Style::default().fg(theme.blue))
        .alignment(Alignment::Left);
    f.render_widget(help_paragraph, footer_chunks[0]);
    // Note: Help area for click detection might need adjustment if we want it clickable in footer
    // For now, let's keep it clickable but we need to update app.help_area
    app.help_area = footer_chunks[0];

    // Title (Footer Center/Right)
    let title_text = match app.current_view {
        CurrentView::Calendars => " Calendars ".to_string(),
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
                    let week_end = week_start + Duration::days(6);
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
                    let week_end = week_start + Duration::days(4);
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
            }
        }
    };

    let title_paragraph = Paragraph::new(title_text)
        .style(
            Style::default()
                .fg(theme.mauve)
                .add_modifier(Modifier::BOLD),
        )
        .alignment(Alignment::Right)
        .block(Block::default().borders(Borders::NONE));
    f.render_widget(title_paragraph, footer_chunks[1]);

    // Date/Time (Footer Right)
    let now = Local::now();
    let datetime_str = format!(" {} {} ", now.format("%d/%m"), now.format("%H:%M"));
    let datetime_paragraph = Paragraph::new(datetime_str)
        .style(Style::default().fg(theme.foreground))
        .alignment(Alignment::Right);
    f.render_widget(datetime_paragraph, footer_chunks[2]);

    let content_area = main_chunks[1];
    match app.current_view {
        CurrentView::Calendars => draw_calendar_list(f, app, content_area, theme, active_color),
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
                EventViewMode::List => {
                    draw_event_list(f, app, content_area, theme, &calendar_name, active_color)
                }
                EventViewMode::Month => {
                    draw_month_view(f, app, content_area, theme, &calendar_name, active_color)
                }
                EventViewMode::Week => {
                    draw_week_view(f, app, content_area, theme, &calendar_name, active_color)
                }
                EventViewMode::WorkWeek => {
                    draw_work_week_view(f, app, content_area, theme, &calendar_name, active_color)
                }
                EventViewMode::Day => {
                    draw_day_view(f, app, content_area, theme, &calendar_name, active_color)
                }
            }
        }
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
        draw_help_popup(f, app, area, theme);
    }

    // Legend Popup removed (merged into Help)
}

fn draw_help_popup(f: &mut Frame, app: &App, area: Rect, theme: &Theme) {
    f.render_widget(Clear, area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Shortcuts
            Constraint::Percentage(40), // Legend
        ])
        .split(area);

    let up_down_arrow = format!("{}/{}", app.symbols.up_arrow, app.symbols.down_arrow);
    let rows = vec![
        Row::new(vec!["Key", "Action"]),
        Row::new(vec![app.symbols.help.as_str(), "Toggle Help"]),
        Row::new(vec!["q", "Quit"]),
        Row::new(vec!["r", "Refresh Events"]),
        Row::new(vec!["b", "Back"]),
        Row::new(vec!["Enter", "Select / Details"]),
        Row::new(vec!["Tab", "Cycle Views"]),
        Row::new(vec![up_down_arrow.as_str(), "Navigate List / Scroll"]),
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

    f.render_widget(table, chunks[0]);

    // Legend Section
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
    f.render_widget(legend_paragraph, chunks[1]);
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
