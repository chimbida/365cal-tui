use crate::api::{DateTimeTimeZone, GraphCalendar, GraphEvent, ItemBody};
use sqlx::{sqlite::SqlitePool, Row};
use std::error::Error;

pub async fn init_db(db_url: &str) -> Result<SqlitePool, Box<dyn Error + Send + Sync>> {
    let pool = SqlitePool::connect(db_url).await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS calendars (
            id TEXT PRIMARY KEY,
            name TEXT NOT NULL,
            can_share BOOLEAN
        );",
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        "CREATE TABLE IF NOT EXISTS events (
            id TEXT PRIMARY KEY,
            subject TEXT NOT NULL,
            start_time TEXT NOT NULL,
            start_time_zone TEXT,
            end_time TEXT NOT NULL,
            end_time_zone TEXT,
            body_preview TEXT,
            attendees TEXT,
            calendar_id TEXT NOT NULL,
            FOREIGN KEY(calendar_id) REFERENCES calendars(id)
        );",
    )
    .execute(&pool)
    .await?;

    Ok(pool)
}

pub async fn save_calendars(
    pool: &SqlitePool,
    calendars: &[GraphCalendar],
) -> Result<(), Box<dyn Error + Send + Sync>> {
    for cal in calendars {
        sqlx::query("INSERT OR REPLACE INTO calendars (id, name, can_share) VALUES (?, ?, ?)")
            .bind(&cal.id)
            .bind(&cal.name)
            .bind(cal.can_share)
            .execute(pool)
            .await?;
    }
    Ok(())
}

pub async fn get_calendars(
    pool: &SqlitePool,
) -> Result<Vec<GraphCalendar>, Box<dyn Error + Send + Sync>> {
    let rows = sqlx::query("SELECT id, name, can_share FROM calendars")
        .fetch_all(pool)
        .await?;

    let mut calendars = Vec::new();
    for row in rows {
        calendars.push(GraphCalendar {
            id: row.get("id"),
            name: row.get("name"),
            can_share: row.get("can_share"),
        });
    }
    Ok(calendars)
}

pub async fn save_events(
    pool: &SqlitePool,
    events: &[GraphEvent],
    calendar_id: &str,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let mut tx = pool.begin().await?;

    for event in events {
        let attendees_json = serde_json::to_string(&event.attendees).unwrap_or_default();
        let body_content = event.body.as_ref().map(|b| b.content.clone());

        sqlx::query(
            "INSERT OR REPLACE INTO events (
                id, subject, start_time, start_time_zone, end_time, end_time_zone, 
                body_preview, attendees, calendar_id
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&event.id)
        .bind(&event.subject)
        .bind(&event.start.date_time)
        .bind(&event.start._time_zone)
        .bind(&event.end.date_time)
        .bind(&event.end._time_zone)
        .bind(body_content)
        .bind(attendees_json)
        .bind(calendar_id)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

pub async fn get_events(
    pool: &SqlitePool,
    calendar_id: &str,
) -> Result<Vec<GraphEvent>, Box<dyn Error + Send + Sync>> {
    let rows = sqlx::query("SELECT * FROM events WHERE calendar_id = ?")
        .bind(calendar_id)
        .fetch_all(pool)
        .await?;

    let mut events = Vec::new();
    for row in rows {
        let start_time: String = row.get("start_time");
        let start_time_zone: String = row.get("start_time_zone");
        let end_time: String = row.get("end_time");
        let end_time_zone: String = row.get("end_time_zone");
        let body_preview: Option<String> = row.get("body_preview");
        let attendees_json: String = row.get("attendees");

        events.push(GraphEvent {
            id: row.get("id"),
            subject: row.get("subject"),
            start: DateTimeTimeZone {
                date_time: start_time,
                _time_zone: start_time_zone,
            },
            end: DateTimeTimeZone {
                date_time: end_time,
                _time_zone: end_time_zone,
            },
            body: body_preview.map(|c| ItemBody { content: c }),
            attendees: serde_json::from_str(&attendees_json).unwrap_or_default(),
            location: None,
            organizer: None,
        });
    }
    Ok(events)
}
