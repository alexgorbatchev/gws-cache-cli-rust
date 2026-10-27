use super::{Store, messages::like_literal};
use crate::{
    Error, Result,
    model::{CalendarEvent, CalendarState},
};
use chrono::{DateTime, Utc};
use rusqlite::{OptionalExtension, params};

#[derive(Default)]
pub struct EventFilter {
    pub keyword: String,
    pub calendar: Option<String>,
    pub topic: Option<String>,
    pub since: Option<DateTime<Utc>>,
}
impl Store {
    pub fn clear_calendar(&mut self, calendar: &str) -> Result<()> {
        let tx = self.connection.transaction()?;
        tx.execute(
            "DELETE FROM calendar_events WHERE calendar_id=?1",
            [calendar],
        )?;
        tx.execute(
            "DELETE FROM calendar_sync_state WHERE calendar_id=?1",
            [calendar],
        )?;
        tx.commit()?;
        Ok(())
    }
    pub fn calendar_state(&self, calendar: &str) -> Result<Option<CalendarState>> {
        Ok(self.connection.query_row("SELECT calendar_id,sync_token,last_synced_at,window_start,window_end FROM calendar_sync_state WHERE calendar_id=?1", [calendar], |r| Ok(CalendarState { calendar_id:r.get(0)?,sync_token:r.get(1)?,last_synced_at:r.get(2)?,window_start:r.get(3)?,window_end:r.get(4)? })).optional()?)
    }
    /// Persist a complete fetched calendar and its cursor together. A reset
    /// replaces that calendar's previous scope after a rejected cursor/full sync.
    pub fn commit_calendar(
        &mut self,
        events: &[CalendarEvent],
        state: &CalendarState,
        reset: bool,
    ) -> Result<()> {
        if events
            .iter()
            .any(|event| event.calendar_id != state.calendar_id)
        {
            return Err(Error::Invalid(
                "calendar records and cursor must belong to the same calendar".into(),
            ));
        }
        let tx = self.connection.transaction()?;
        if reset {
            tx.execute(
                "DELETE FROM calendar_events WHERE calendar_id=?1",
                [&state.calendar_id],
            )?;
        }
        for e in events {
            tx.execute("INSERT INTO calendar_events(id,calendar_id,topic_id,summary,description,location,organizer_email,start_time,end_time,status,is_deleted,attendees_json,html_link,i_cal_uid,all_day,created_at,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17) ON CONFLICT(calendar_id,id) DO UPDATE SET topic_id=excluded.topic_id,summary=excluded.summary,description=excluded.description,location=excluded.location,organizer_email=excluded.organizer_email,start_time=excluded.start_time,end_time=excluded.end_time,status=excluded.status,is_deleted=excluded.is_deleted,attendees_json=excluded.attendees_json,html_link=excluded.html_link,i_cal_uid=excluded.i_cal_uid,all_day=excluded.all_day,updated_at=excluded.updated_at", params![e.id,e.calendar_id,e.topic_id,e.summary,e.description,e.location,e.organizer_email,e.start_time,e.end_time,e.status,e.is_deleted,e.attendees_json,e.html_link,e.i_cal_uid,e.all_day,e.created_at,e.updated_at])?;
        }
        tx.execute("INSERT INTO calendar_sync_state(calendar_id,sync_token,last_synced_at,window_start,window_end) VALUES (?1,?2,?3,?4,?5) ON CONFLICT(calendar_id) DO UPDATE SET sync_token=excluded.sync_token,last_synced_at=excluded.last_synced_at,window_start=excluded.window_start,window_end=excluded.window_end", params![state.calendar_id,state.sync_token,state.last_synced_at,state.window_start,state.window_end])?;
        tx.commit()?;
        Ok(())
    }
    pub fn event(&self, calendar: &str, id: &str) -> Result<Option<CalendarEvent>> {
        Ok(self
            .connection
            .query_row(
                &format!("{EVENT_SELECT} WHERE e.calendar_id=?1 AND e.id=?2"),
                params![calendar, id],
                read_event,
            )
            .optional()?)
    }
    pub fn events(&self, filter: &EventFilter) -> Result<Vec<CalendarEvent>> {
        let query = format!(
            "{EVENT_SELECT} WHERE (?1 IS NULL OR e.calendar_id=?1) AND (?2 IS NULL OR e.topic_id IN (SELECT id FROM topics WHERE slug=?2)) AND (?3 IS NULL OR julianday(e.start_time)>=julianday(?3)) AND (?4='' OR e.summary LIKE ?5 ESCAPE '\\' OR e.description LIKE ?5 ESCAPE '\\' OR e.location LIKE ?5 ESCAPE '\\' OR e.organizer_email LIKE ?5 ESCAPE '\\') ORDER BY julianday(e.start_time),e.calendar_id,e.id"
        );
        Ok(self
            .connection
            .prepare(&query)?
            .query_map(
                params![
                    filter.calendar,
                    filter.topic,
                    filter.since,
                    filter.keyword,
                    like_literal(&filter.keyword)
                ],
                read_event,
            )?
            .collect::<rusqlite::Result<_>>()?)
    }
}
const EVENT_SELECT: &str = "SELECT e.id,e.calendar_id,e.topic_id,e.summary,COALESCE(e.description,''),COALESCE(e.location,''),COALESCE(e.organizer_email,''),e.start_time,e.end_time,e.status,e.is_deleted,COALESCE(e.attendees_json,'[]'),COALESCE(e.html_link,''),COALESCE(e.i_cal_uid,''),e.all_day,e.created_at,e.updated_at FROM calendar_events e";
fn read_event(r: &rusqlite::Row<'_>) -> rusqlite::Result<CalendarEvent> {
    Ok(CalendarEvent {
        id: r.get(0)?,
        calendar_id: r.get(1)?,
        topic_id: r.get(2)?,
        summary: r.get(3)?,
        description: r.get(4)?,
        location: r.get(5)?,
        organizer_email: r.get(6)?,
        start_time: r.get(7)?,
        end_time: r.get(8)?,
        status: r.get(9)?,
        is_deleted: r.get(10)?,
        attendees_json: r.get(11)?,
        html_link: r.get(12)?,
        i_cal_uid: r.get(13)?,
        all_day: r.get(14)?,
        created_at: r.get(15)?,
        updated_at: r.get(16)?,
    })
}
