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

        let now_utc = chrono::Utc::now();
        let threshold_time_utc = now_utc + chrono::Duration::minutes(self.minutes_before as i64);

        for event in events {
            let start_time_str = &event.start.date_time;

            // Parse as NaiveDateTime first
            let start_naive =
                match chrono::NaiveDateTime::parse_from_str(start_time_str, "%Y-%m-%dT%H:%M:%S%.f")
                {
                    Ok(t) => t,
                    Err(_) => {
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

            // Assume the API returns UTC times (which is standard for Graph API)
            let start_time_utc = chrono::DateTime::<chrono::Utc>::from_naive_utc_and_offset(
                start_naive,
                chrono::Utc,
            );

            if start_time_utc > now_utc && start_time_utc <= threshold_time_utc {
                if !self.notified_events.contains(&event.id) {
                    self.send_notification(&event.subject, start_time_utc);
                    self.notified_events.insert(event.id.clone());
                }
            }
        }
    }

    fn send_notification(&self, subject: &str, start_time_utc: chrono::DateTime<chrono::Utc>) {
        info!("Sending notification for event: {}", subject);

        // Convert to Local time for display
        let local_time = start_time_utc.with_timezone(&Local);
        let time_display = local_time.format("%H:%M").to_string();

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
