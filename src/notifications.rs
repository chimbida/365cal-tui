use crate::api::GraphEvent;
use chrono::Local;
use log::{debug, error, info};
use notify_rust::Notification;
use std::collections::HashSet;

pub struct NotificationManager {
    notified_events: HashSet<String>,
    minutes_before: u64,
    enabled: bool,
}

impl NotificationManager {
    pub fn new(enabled: bool, minutes_before: u64) -> Self {
        Self {
            notified_events: HashSet::new(),
            minutes_before,
            enabled,
        }
    }

    pub fn check_and_notify(&mut self, events: &[GraphEvent]) {
        if !self.enabled {
            return;
        }

        let now = Local::now();
        let threshold_time = now + chrono::Duration::minutes(self.minutes_before as i64);

        for event in events {
            // Parse event start time
            // Assuming event.start.date_time is in ISO 8601 format or similar that chrono parses
            // The Graph API returns UTC usually, but we need to handle it correctly.
            // Our GraphEvent struct has DateTimeTimeZone.

            // We need to parse the date string.
            // Since we don't have easy access to the exact timezone offset from the string sometimes,
            // we'll try to parse it as NaiveDateTime and assume it's in the user's local time
            // if the API returns it that way, or UTC.
            //
            // However, looking at api.rs, start is DateTimeTimeZone which has date_time: String.
            // Let's try to parse it.

            let start_time_str = &event.start.date_time;

            // Microsoft Graph often returns 7 digits of precision for seconds, which chrono might struggle with
            // if not handled, or standard ISO.
            // Let's try standard parsing.

            let start_time =
                match chrono::NaiveDateTime::parse_from_str(start_time_str, "%Y-%m-%dT%H:%M:%S%.f")
                {
                    Ok(t) => t,
                    Err(_) => {
                        // Try without fractional seconds
                        match chrono::NaiveDateTime::parse_from_str(
                            start_time_str,
                            "%Y-%m-%dT%H:%M:%S",
                        ) {
                            Ok(t) => t,
                            Err(e) => {
                                debug!(
                                    "Failed to parse event time for notification: {} - {}",
                                    start_time_str, e
                                );
                                continue;
                            }
                        }
                    }
                };

            // Convert NaiveDateTime to Local DateTime for comparison
            // Ideally we should respect the time zone in event.start.time_zone
            // But for simplicity and since we often get times in UTC or Local,
            // let's assume the comparison logic:

            // If the event is within the window:
            // now < event_start <= threshold_time

            // We need to convert start_time to the same timezone as 'now' (Local) or 'now' to UTC.
            // Let's assume the event time is roughly comparable.
            // A better approach is to rely on the fact that we display these times in the UI.

            // Let's try to be safe: treat start_time as Local for now if it lacks offset,
            // or better, just compare NaiveDateTimes if we assume everything is consistent.

            let now_naive = now.naive_local();
            let threshold_naive = threshold_time.naive_local();

            if start_time > now_naive && start_time <= threshold_naive {
                if !self.notified_events.contains(&event.id) {
                    self.send_notification(&event.subject, start_time_str);
                    self.notified_events.insert(event.id.clone());
                }
            }
        }
    }

    fn send_notification(&self, subject: &str, time_str: &str) {
        info!("Sending notification for event: {}", subject);

        // Format time for display (simple)
        let time_display =
            match chrono::NaiveDateTime::parse_from_str(time_str, "%Y-%m-%dT%H:%M:%S%.f") {
                Ok(t) => t.format("%H:%M").to_string(),
                Err(_) => time_str.to_string(), // Fallback
            };

        let body = format!("Starting at {}", time_display);

        let result = Notification::new()
            .summary(subject)
            .body(&body)
            .appname("365cal-tui")
            .icon("calendar")
            .show();

        if let Err(e) = result {
            error!("Failed to send notification: {}", e);
        }
    }
}
