use chrono::{DateTime, Utc};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct GraphCalendar {
    pub id: String,
    pub name: String,
}
#[derive(Debug, Deserialize)]
struct CalendarListResponse {
    value: Vec<GraphCalendar>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    pub _time_zone: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct GraphEvent {
    pub subject: String,
    pub start: DateTimeTimeZone,
    pub end: DateTimeTimeZone,
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub attendees: Vec<Attendee>,
}
#[derive(Debug, Clone, Deserialize)]
pub struct ItemBody {
    pub content: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub name: String,
    pub address: String,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    pub email_address: Option<EmailAddress>,
}
#[derive(Debug, Deserialize)]
struct EventListResponse {
    value: Vec<GraphEvent>,
}

pub async fn list_calendars(access_token: &str) -> Result<Vec<GraphCalendar>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let response = client
        .get("https://graph.microsoft.com/v1.0/me/calendars")
        .bearer_auth(access_token)
        .send()
        .await?;
    let calendar_list = response.json::<CalendarListResponse>().await?;
    Ok(calendar_list.value)
}

pub async fn list_events(
    access_token: &str,
    calendar_id: &str,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
) -> Result<Vec<GraphEvent>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let start_str = start_date.to_rfc3339();
    let end_str = end_date.to_rfc3339();
    let base_url = format!(
        "https://graph.microsoft.com/v1.0/me/calendars/{}/calendarview",
        calendar_id
    );
    let select_fields = "subject,start,end,body,attendees".to_string();
    let response = client
        .get(&base_url)
        .bearer_auth(access_token)
        .query(&[
            ("startDateTime", &start_str),
            ("endDateTime", &end_str),
            ("$select", &select_fields),
            ("$orderby", &"start/dateTime".to_string()),
        ])
        .send()
        .await?;
    let text = response.text().await?;
    let event_list: EventListResponse = serde_json::from_str(&text).map_err(|e| {
        log::error!("Failed to decode JSON: {}. JSON received: {}", e, text);
        e
    })?;
    Ok(event_list.value)
}