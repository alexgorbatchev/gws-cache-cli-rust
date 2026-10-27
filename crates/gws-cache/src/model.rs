//! Native Google JSON resources and cached records.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Header {
    pub name: String,
    pub value: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Payload {
    pub headers: Vec<Header>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct MessageDetail {
    pub id: String,
    pub thread_id: String,
    pub history_id: String,
    pub internal_date: String,
    pub snippet: String,
    pub payload: Payload,
}
impl MessageDetail {
    pub fn header(&self, name: &str) -> &str {
        self.payload
            .headers
            .iter()
            .find(|h| h.name.eq_ignore_ascii_case(name))
            .map_or("", |h| h.value.as_str())
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThreadSummary {
    pub id: String,
    pub history_id: String,
    pub snippet: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThreadDetail {
    pub id: String,
    pub history_id: String,
    pub messages: Vec<MessageDetail>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct ThreadPage {
    pub threads: Vec<ThreadSummary>,
    pub next_page_token: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Attendee {
    pub email: String,
    pub display_name: String,
    pub response_status: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Identity {
    pub email: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventTime {
    pub date_time: String,
    pub date: String,
    pub time_zone: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventDetail {
    pub id: String,
    pub status: String,
    pub summary: String,
    pub description: String,
    pub location: String,
    pub organizer: Identity,
    pub start: EventTime,
    pub end: EventTime,
    pub attendees: Vec<Attendee>,
    pub html_link: String,
    #[serde(rename = "iCalUID")]
    pub i_cal_uid: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct EventPage {
    pub items: Vec<EventDetail>,
    pub next_page_token: String,
    pub next_sync_token: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CalendarParams {
    pub calendar_id: String,
    pub single_events: bool,
    pub show_deleted: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_min: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_max: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sync_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_token: Option<String>,
}
impl Default for CalendarParams {
    fn default() -> Self {
        Self {
            calendar_id: "primary".into(),
            single_events: true,
            show_deleted: true,
            time_min: None,
            time_max: None,
            sync_token: None,
            page_token: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Topic {
    pub id: i64,
    pub slug: String,
    pub display_name: String,
    pub query: String,
    pub last_synced_at: Option<DateTime<Utc>>,
    pub last_history_id: String,
    pub last_message_internal_date: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
    pub id: String,
    pub thread_id: String,
    pub topic_ids: Vec<i64>,
    pub internal_date: i64,
    pub date_iso: String,
    pub from_address: String,
    pub from_name: String,
    pub to_address: String,
    pub subject: String,
    pub snippet: String,
    pub body_plain: String,
    pub is_automated: bool,
    pub classification_reason: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalendarEvent {
    pub id: String,
    pub calendar_id: String,
    pub topic_id: Option<i64>,
    pub summary: String,
    pub description: String,
    pub location: String,
    pub organizer_email: String,
    pub start_time: Option<DateTime<Utc>>,
    pub end_time: Option<DateTime<Utc>>,
    pub all_day: bool,
    pub status: String,
    pub is_deleted: bool,
    pub attendees_json: String,
    pub html_link: String,
    pub i_cal_uid: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug)]
pub struct CalendarState {
    pub calendar_id: String,
    pub sync_token: String,
    pub last_synced_at: DateTime<Utc>,
    pub window_start: Option<DateTime<Utc>>,
    pub window_end: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
pub struct Stats {
    pub topics: i64,
    pub threads: i64,
    pub messages: i64,
    pub calendar_events: i64,
}

#[derive(Debug, Default, Serialize)]
pub struct MailSyncResult {
    pub target: String,
    pub threads_listed: usize,
    pub threads_fetched: usize,
    pub threads_remaining: usize,
    pub messages_ingested: usize,
    pub max_history_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
}
