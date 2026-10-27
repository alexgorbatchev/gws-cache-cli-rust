use crate::{Error, Result};
use chrono::{DateTime, Utc};
use rusqlite::{
    Connection, OptionalExtension, params,
    types::{FromSql, ValueRef},
};

pub(super) fn initialize(connection: &mut Connection) -> Result<()> {
    let version: u32 = connection.query_row("PRAGMA user_version", [], |r| r.get(0))?;
    if version > 1 {
        return Err(Error::Invalid(format!(
            "unsupported cache schema version {version}"
        )));
    }
    let legacy = version == 0
        && connection
            .query_row(
                "SELECT 1 FROM pragma_table_info('messages') WHERE name='topic_id'",
                [],
                |_| Ok(()),
            )
            .optional()?
            .is_some();
    connection.execute_batch("PRAGMA foreign_keys=OFF;")?;
    let tx = connection.transaction()?;
    if legacy {
        tx.execute_batch("ALTER TABLE messages RENAME TO legacy_messages; ALTER TABLE threads RENAME TO legacy_threads; ALTER TABLE calendar_events RENAME TO legacy_events; ALTER TABLE calendar_sync_state RENAME TO legacy_calendar_state;")?;
    }
    tx.execute_batch(include_str!("schema.sql"))?;
    if legacy {
        tx.execute_batch("INSERT INTO threads SELECT id,history_id,snippet,last_synced_at FROM legacy_threads;
            INSERT INTO thread_topics SELECT id,topic_id FROM legacy_threads WHERE topic_id IS NOT NULL;
            INSERT OR IGNORE INTO thread_topics SELECT thread_id,topic_id FROM legacy_messages WHERE topic_id IS NOT NULL;
            INSERT INTO messages SELECT id,thread_id,internal_date,date_iso,from_address,from_name,to_address,subject,snippet,body_plain,is_automated,classification_reason,created_at FROM legacy_messages;
            INSERT INTO calendar_events(id,calendar_id,topic_id,summary,description,location,organizer_email,start_time,end_time,status,is_deleted,attendees_json,html_link,i_cal_uid,created_at,updated_at)
            SELECT id,calendar_id,topic_id,summary,description,location,organizer_email,start_time,end_time,status,is_deleted,attendees_json,html_link,i_cal_uid,created_at,updated_at FROM legacy_events;
            INSERT INTO calendar_sync_state SELECT * FROM legacy_calendar_state;
            DROP TABLE legacy_messages; DROP TABLE legacy_threads; DROP TABLE legacy_events; DROP TABLE legacy_calendar_state;")?;
        // The Go SQLite driver's default time.Time.String format includes a zone
        // abbreviation and sometimes a monotonic suffix. Normalize persisted
        // timestamps once, before native Chrono decoding and SQL date comparisons.
        for (table, columns) in [
            (
                "topics",
                &["last_synced_at", "created_at", "updated_at"][..],
            ),
            ("threads", &["last_synced_at"]),
            ("messages", &["created_at"]),
            (
                "calendar_events",
                &["start_time", "end_time", "created_at", "updated_at"],
            ),
            (
                "calendar_sync_state",
                &["last_synced_at", "window_start", "window_end"],
            ),
            ("sync_runs", &["started_at", "completed_at"]),
        ] {
            for column in columns {
                let rows = tx
                    .prepare(&format!(
                        "SELECT rowid,{column} FROM {table} WHERE {column} IS NOT NULL"
                    ))?
                    .query_map([], |r| Ok((r.get::<_, i64>(0)?, r.get::<_, String>(1)?)))?
                    .collect::<rusqlite::Result<Vec<_>>>()?;
                for (rowid, raw) in rows {
                    let value = normalize_timestamp(&raw)?;
                    tx.execute(
                        &format!("UPDATE {table} SET {column}=?1 WHERE rowid=?2"),
                        params![value, rowid],
                    )?;
                }
            }
        }
        // Index names belonged to the renamed tables until their deletion.
        tx.execute_batch(include_str!("schema.sql"))?;
    }
    let violation = tx
        .query_row("PRAGMA foreign_key_check", [], |r| r.get::<_, String>(0))
        .optional()?;
    if let Some(table) = violation {
        return Err(Error::Invalid(format!(
            "cache migration encountered invalid foreign keys in {table}"
        )));
    }
    tx.execute_batch("PRAGMA user_version=1;")?;
    tx.commit()?;
    Ok(())
}

fn normalize_timestamp(raw: &str) -> Result<DateTime<Utc>> {
    if let Ok(value) = DateTime::<Utc>::column_result(ValueRef::Text(raw.as_bytes())) {
        return Ok(value);
    }
    let without_monotonic = raw.split(" m=").next().unwrap_or(raw);
    DateTime::parse_from_str(without_monotonic, "%Y-%m-%d %H:%M:%S%.f %z %Z")
        .map(|t| t.with_timezone(&Utc))
        .map_err(|_| Error::Invalid(format!("unrecognized persisted cache timestamp {raw:?}")))
}
