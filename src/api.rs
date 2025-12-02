use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// --- Data Structures for Deserializing API Responses ---

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphCalendar {
    pub id: String,
    pub name: String,
    #[serde(rename = "canShare")]
    pub can_share: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct CalendarListResponse {
    value: Vec<GraphCalendar>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DateTimeTimeZone {
    pub date_time: String,
    pub _time_zone: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Location {
    #[serde(rename = "displayName")]
    pub display_name: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Organizer {
    pub email_address: EmailAddress,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GraphEvent {
    pub id: String,
    pub subject: String,
    pub start: DateTimeTimeZone,
    pub end: DateTimeTimeZone,
    #[serde(default)]
    pub body: Option<ItemBody>,
    #[serde(default)]
    pub attendees: Vec<Attendee>,
    pub location: Option<Location>,
    pub organizer: Option<Organizer>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ItemBody {
    pub content: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EmailAddress {
    pub name: String,
    pub address: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Attendee {
    pub email_address: Option<EmailAddress>,
}

#[derive(Debug, Deserialize)]
struct EventListResponse {
    value: Vec<GraphEvent>,
    #[serde(rename = "@odata.nextLink")]
    next_link: Option<String>,
}

// --- API Call Functions ---

pub async fn list_calendars(
    access_token: &str,
) -> Result<Vec<GraphCalendar>, Box<dyn std::error::Error + Send + Sync>> {
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
) -> Result<Vec<GraphEvent>, Box<dyn std::error::Error + Send + Sync>> {
    let client = reqwest::Client::new();
    let mut all_events = Vec::new();

    // Base URL without query parameters
    let base_url = format!(
        "https://graph.microsoft.com/v1.0/me/calendars/{}/calendarview",
        calendar_id
    );

    // Parameters for the initial request
    let start_str = start_date.to_rfc3339();
    let end_str = end_date.to_rfc3339();
    let select_fields = "subject,start,end,body,attendees,location,organizer".to_string();
    let orderby_field = "start/dateTime".to_string();

    // Build the first request using .query() for proper URL encoding
    let initial_response = client
        .get(&base_url)
        .bearer_auth(access_token)
        .query(&[
            ("startDateTime", &start_str),
            ("endDateTime", &end_str),
            ("$select", &select_fields),
            ("$orderby", &orderby_field),
        ])
        .send()
        .await?;

    // Process the first page of results
    let text = initial_response.text().await?;
    // CORRECTION: Removed the unnecessary `mut` keyword.
    let event_response: EventListResponse = serde_json::from_str(&text).map_err(|e| {
        log::error!(
            "Failed to decode JSON on first page: {}. JSON received: {}",
            e,
            text
        );
        e
    })?;

    all_events.extend(event_response.value);
    let mut next_url = event_response.next_link;

    // Loop for subsequent pages using the nextLink provided by the API
    while let Some(url) = next_url {
        log::info!("Fetching next event page from: {}", url);
        let response = client.get(&url).bearer_auth(access_token).send().await?;

        let text = response.text().await?;
        let event_response: EventListResponse = serde_json::from_str(&text).map_err(|e| {
            log::error!(
                "Failed to decode JSON on paginated request: {}. JSON received: {}",
                e,
                text
            );
            e
        })?;

        all_events.extend(event_response.value);
        next_url = event_response.next_link;
    }

    Ok(all_events)
}
