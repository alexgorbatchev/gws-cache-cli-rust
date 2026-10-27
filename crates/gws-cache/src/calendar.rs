use crate::{
    Client, Error, Result, Store, Window,
    model::{CalendarEvent, CalendarParams, CalendarState, EventDetail, EventTime, Topic},
};
use chrono::{DateTime, NaiveDate, NaiveDateTime, TimeDelta, TimeZone, Utc};
use regex::Regex;
use serde::Serialize;
use tokio_util::sync::CancellationToken;

pub struct CalendarOptions {
    pub calendar_id: String,
    pub past: Window,
    pub future: Window,
    pub force_full: bool,
    pub window_explicit: bool,
}
impl Default for CalendarOptions {
    fn default() -> Self {
        Self {
            calendar_id: "primary".into(),
            past: Window::Relative(TimeDelta::weeks(4)),
            future: Window::Relative(TimeDelta::weeks(4)),
            force_full: false,
            window_explicit: false,
        }
    }
}
#[derive(Debug, Serialize)]
pub struct CalendarSyncResult {
    pub calendar_id: String,
    pub events_fetched: usize,
    pub events_ingested: usize,
    pub events_deleted: usize,
}

pub async fn sync_calendar(
    store: &mut Store,
    client: &Client,
    options: &CalendarOptions,
    cancel: &CancellationToken,
    now: DateTime<Utc>,
) -> Result<CalendarSyncResult> {
    let calendar = if options.calendar_id.is_empty() {
        "primary"
    } else {
        &options.calendar_id
    };
    let start = options.past.start(now)?;
    let end = options.future.end(now)?;
    if start.zip(end).is_some_and(|(s, e)| e <= s) {
        return Err(Error::Invalid(
            "calendar window end must be later than its start".into(),
        ));
    }
    let previous = store.calendar_state(calendar)?;
    let mut delta = !options.force_full
        && !options.window_explicit
        && previous.as_ref().is_some_and(|s| !s.sync_token.is_empty());
    let bounded = CalendarParams {
        calendar_id: calendar.into(),
        time_min: start.map(|t| t.to_rfc3339()),
        time_max: end.map(|t| t.to_rfc3339()),
        ..Default::default()
    };
    let page = if delta {
        match client
            .calendar_events(
                CalendarParams {
                    calendar_id: calendar.into(),
                    sync_token: previous.as_ref().map(|s| s.sync_token.clone()),
                    ..Default::default()
                },
                cancel,
            )
            .await
        {
            Ok(page) => page,
            Err(Error::Workspace {
                code: Some(410), ..
            }) => {
                store.clear_calendar(calendar)?;
                delta = false;
                client.calendar_events(bounded, cancel).await?
            }
            Err(error) => return Err(error),
        }
    } else {
        client.calendar_events(bounded, cancel).await?
    };
    if page.next_sync_token.is_empty() {
        return Err(Error::Invalid(
            "Calendar's final page did not provide a sync token; no fetched records were committed"
                .into(),
        ));
    }
    let matcher = Matcher::new(store.topics()?)?;
    let mut records = Vec::new();
    for detail in &page.items {
        if cancel.is_cancelled() {
            return Err(Error::Cancelled);
        }
        let existing = store.event(calendar, &detail.id)?;
        records.push(convert_event(detail, calendar, existing, &matcher, now)?);
    }
    let result = CalendarSyncResult {
        calendar_id: calendar.into(),
        events_fetched: records.len(),
        events_ingested: records.len(),
        events_deleted: records.iter().filter(|e| e.is_deleted).count(),
    };
    let (window_start, window_end) = if delta {
        let state = previous.as_ref().expect("delta has stored state");
        (state.window_start, state.window_end)
    } else {
        (start, end)
    };
    if cancel.is_cancelled() {
        return Err(Error::Cancelled);
    }
    store.commit_calendar(
        &records,
        &CalendarState {
            calendar_id: calendar.into(),
            sync_token: page.next_sync_token,
            last_synced_at: now,
            window_start,
            window_end,
        },
        !delta,
    )?;
    Ok(result)
}

struct Matcher(Vec<(Regex, i64)>);
impl Matcher {
    fn new(topics: Vec<Topic>) -> Result<Self> {
        let mut terms = Vec::new();
        for topic in topics {
            if !topic.display_name.eq_ignore_ascii_case(&topic.slug)
                && !topic.display_name.is_empty()
            {
                terms.push((topic.display_name, topic.id));
            }
            terms.push((topic.slug, topic.id));
        }
        terms.sort_by(|a, b| {
            b.0.len()
                .cmp(&a.0.len())
                .then(a.0.cmp(&b.0))
                .then(a.1.cmp(&b.1))
        });
        Ok(Self(
            terms
                .into_iter()
                .map(|(term, id)| {
                    Ok((
                        Regex::new(&format!(r"(?i)\b{}\b", regex::escape(&term)))?,
                        id,
                    ))
                })
                .collect::<Result<_>>()?,
        ))
    }
    fn topic(&self, summary: &str, description: &str) -> Option<i64> {
        self.0
            .iter()
            .find(|(pattern, _)| pattern.is_match(summary) || pattern.is_match(description))
            .map(|(_, id)| *id)
    }
}
fn convert_event(
    detail: &EventDetail,
    calendar: &str,
    existing: Option<CalendarEvent>,
    matcher: &Matcher,
    now: DateTime<Utc>,
) -> Result<CalendarEvent> {
    if detail.id.is_empty() {
        return Err(Error::Invalid("Calendar event has no ID".into()));
    }
    let deleted = detail.status == "cancelled";
    if deleted && let Some(mut record) = existing {
        record.is_deleted = true;
        record.status = detail.status.clone();
        record.updated_at = now;
        return Ok(record);
    }
    let start = parse_time(&detail.start)?;
    let end = parse_time(&detail.end)?;
    if !deleted && (start.is_none() || end.is_none()) {
        return Err(Error::Invalid(format!(
            "Calendar event {} has no valid start/end",
            detail.id
        )));
    }
    if start.zip(end).is_some_and(|(s, e)| e <= s) {
        return Err(Error::Invalid(format!(
            "Calendar event {} has a nonpositive duration",
            detail.id
        )));
    }
    Ok(CalendarEvent {
        id: detail.id.clone(),
        calendar_id: calendar.into(),
        topic_id: matcher.topic(&detail.summary, &detail.description),
        summary: detail.summary.clone(),
        description: detail.description.clone(),
        location: detail.location.clone(),
        organizer_email: detail.organizer.email.clone(),
        start_time: start,
        end_time: end,
        all_day: !detail.start.date.is_empty(),
        status: detail.status.clone(),
        is_deleted: deleted,
        attendees_json: serde_json::to_string(&detail.attendees)?,
        html_link: detail.html_link.clone(),
        i_cal_uid: detail.i_cal_uid.clone(),
        created_at: now,
        updated_at: now,
    })
}
fn parse_time(time: &EventTime) -> Result<Option<DateTime<Utc>>> {
    if !time.date_time.is_empty() {
        if let Ok(value) = DateTime::parse_from_rfc3339(&time.date_time) {
            return Ok(Some(value.with_timezone(&Utc)));
        }
        let zone = time.time_zone.parse::<chrono_tz::Tz>().map_err(|_| {
            Error::Invalid(format!(
                "invalid Calendar timestamp/timezone {:?}/{:?}",
                time.date_time, time.time_zone
            ))
        })?;
        let naive = NaiveDateTime::parse_from_str(&time.date_time, "%Y-%m-%dT%H:%M:%S%.f")
            .map_err(|_| {
                Error::Invalid(format!("invalid Calendar timestamp {:?}", time.date_time))
            })?;
        return zone
            .from_local_datetime(&naive)
            .single()
            .map(|t| Some(t.with_timezone(&Utc)))
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "ambiguous or nonexistent Calendar local time {:?}",
                    time.date_time
                ))
            });
    }
    if !time.date.is_empty() {
        return NaiveDate::parse_from_str(&time.date, "%Y-%m-%d")
            .map(|d| Some(d.and_hms_opt(0, 0, 0).expect("midnight").and_utc()))
            .map_err(|_| Error::Invalid(format!("invalid Calendar date {:?}", time.date)));
    }
    Ok(None)
}
