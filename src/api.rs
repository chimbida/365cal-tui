use chrono::{DateTime, Utc};
use serde::Deserialize;

// --- Data Structures for Deserializing API Responses ---

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
    // The timezone name (e.g., "UTC") is captured but not used in our formatting logic.
    pub _time_zone: String,
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
    // This is optional because some attendees (like meeting rooms) might not have an email.
    pub email_address: Option<EmailAddress>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GraphEvent {
    pub subject: String,
    pub start: DateTimeTimeZone,
    pub end: DateTimeTimeZone,
    // These fields are optional because the API might not return them for all events.
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub attendees: Vec<Attendee>,
}

#[derive(Debug, Deserialize)]
struct EventListResponse {
    value: Vec<GraphEvent>,
}


// --- API Call Functions ---

/// Fetches the list of all calendars for the authenticated user.
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

/// Fetches events for a specific calendar within a given date range.
/// Uses the `/calendarview` endpoint which is optimized for time-window queries.
pub async fn list_events(
    access_token: &str,
    calendar_id: &str,
    start_date: DateTime<Utc>,
    end_date: DateTime<Utc>,
) -> Result<Vec<GraphEvent>, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    
    // Format dates into the ISO 8601 strings required by the API.
    let start_str = start_date.to_rfc3339();
    let end_str = end_date.to_rfc3339();

    let base_url = format!(
        "https://graph.microsoft.com/v1.0/me/calendars/{}/calendarview",
        calendar_id
    );
    
    // Request the specific fields we need to keep the response size small.
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
    
    // Attempt to deserialize the JSON response.
    let event_list: EventListResponse = serde_json::from_str(&text).map_err(|e| {
        log::error!("Failed to decode JSON: {}. JSON received: {}", e, text);
        e // Return the original serde error.
    })?;

    Ok(event_list.value)
}