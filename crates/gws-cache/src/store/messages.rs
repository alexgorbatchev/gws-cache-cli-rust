use super::Store;
use crate::{Result, model::Message};
use chrono::{DateTime, Utc};
use rusqlite::params;

#[derive(Default)]
pub struct MessageFilter {
    pub keyword: String,
    pub topic: Option<String>,
    pub human_only: bool,
    pub since: Option<DateTime<Utc>>,
    pub ascending: bool,
}
impl Store {
    pub fn messages(&self, filter: &MessageFilter) -> Result<Vec<Message>> {
        let query = format!(
            "SELECT m.id,m.thread_id,m.internal_date,m.date_iso,m.from_address,COALESCE(m.from_name,''),m.to_address,m.subject,m.snippet,COALESCE(m.body_plain,''),m.is_automated,COALESCE(m.classification_reason,''),m.created_at FROM messages m WHERE (?1 IS NULL OR EXISTS(SELECT 1 FROM thread_topics tt JOIN topics t ON t.id=tt.topic_id WHERE tt.thread_id=m.thread_id AND t.slug=?1)) AND (?2=0 OR m.is_automated=0) AND (?3 IS NULL OR m.internal_date>=?3) AND (?4='' OR m.subject LIKE ?5 ESCAPE '\\' OR m.snippet LIKE ?5 ESCAPE '\\' OR m.from_address LIKE ?5 ESCAPE '\\' OR m.from_name LIKE ?5 ESCAPE '\\') ORDER BY m.internal_date {},m.id",
            if filter.ascending { "ASC" } else { "DESC" }
        );
        let mut rows = self.connection.prepare(&query)?;
        let messages = rows
            .query_map(
                params![
                    filter.topic,
                    filter.human_only,
                    filter.since.map(|s| s.timestamp_millis()),
                    filter.keyword,
                    like_literal(&filter.keyword)
                ],
                |r| {
                    Ok(Message {
                        id: r.get(0)?,
                        thread_id: r.get(1)?,
                        topic_ids: Vec::new(),
                        internal_date: r.get(2)?,
                        date_iso: r.get(3)?,
                        from_address: r.get(4)?,
                        from_name: r.get(5)?,
                        to_address: r.get(6)?,
                        subject: r.get(7)?,
                        snippet: r.get(8)?,
                        body_plain: r.get(9)?,
                        is_automated: r.get(10)?,
                        classification_reason: r.get(11)?,
                        created_at: r.get(12)?,
                    })
                },
            )?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        let mut membership = self
            .connection
            .prepare("SELECT topic_id FROM thread_topics WHERE thread_id=?1 ORDER BY topic_id")?;
        messages
            .into_iter()
            .map(|mut m| {
                m.topic_ids = membership
                    .query_map([&m.thread_id], |r| r.get(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok(m)
            })
            .collect()
    }
}
pub(super) fn like_literal(value: &str) -> String {
    format!(
        "%{}%",
        value
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}
