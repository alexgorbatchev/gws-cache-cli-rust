use crate::{MessageFilter, Result, Store, Window};
use chrono::{DateTime, TimeDelta, Utc};
use serde::Serialize;
use std::fmt::Write;

#[derive(Default)]
pub enum ExportFormat {
    #[default]
    Markdown,
    Json,
}
pub struct ExportOptions {
    pub human_only: bool,
    pub format: ExportFormat,
    pub since: Window,
}
impl Default for ExportOptions {
    fn default() -> Self {
        Self {
            human_only: true,
            format: ExportFormat::Markdown,
            since: Window::Relative(TimeDelta::weeks(4)),
        }
    }
}
#[derive(Serialize)]
struct FormattedMessage<'a> {
    id: &'a str,
    thread_id: &'a str,
    date_iso: &'a str,
    date_display: String,
    from: &'a str,
    subject: &'a str,
    snippet: &'a str,
    is_automated: bool,
}
pub fn export_topic(
    store: &Store,
    slug: &str,
    options: &ExportOptions,
    now: DateTime<Utc>,
) -> Result<String> {
    let topic = store.topic(slug)?;
    let messages = store.messages(&MessageFilter {
        topic: Some(slug.into()),
        human_only: options.human_only,
        since: options.since.start(now)?,
        ascending: true,
        ..Default::default()
    })?;
    let formatted: Vec<_> = messages
        .iter()
        .map(|m| FormattedMessage {
            id: &m.id,
            thread_id: &m.thread_id,
            date_iso: &m.date_iso,
            date_display: DateTime::parse_from_rfc3339(&m.date_iso).map_or_else(
                |_| m.date_iso.clone(),
                |d| d.format("%b %d, %Y").to_string(),
            ),
            from: if m.from_name.is_empty() {
                &m.from_address
            } else {
                &m.from_name
            },
            subject: &m.subject,
            snippet: &m.snippet,
            is_automated: m.is_automated,
        })
        .collect();
    if matches!(options.format, ExportFormat::Json) {
        return Ok(serde_json::to_string_pretty(&formatted)?);
    }
    let mut output = format!("### {} Timeline\n\n", topic.display_name);
    if formatted.is_empty() {
        output.push_str("*(No messages found)*\n");
    }
    for m in formatted {
        let clean = html_escape::decode_html_entities(m.snippet).replace(['\n', '\r'], " ");
        writeln!(
            output,
            "* **{}** — {}: {}",
            m.date_display,
            m.from,
            clean.trim()
        )
        .expect("String writes are infallible");
    }
    Ok(output)
}
