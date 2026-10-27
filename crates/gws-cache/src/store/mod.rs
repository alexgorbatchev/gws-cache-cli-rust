use crate::{
    Classifier, Error, Result, convert_thread,
    model::{Stats, ThreadDetail, Topic},
};
use chrono::{DateTime, Utc};
use rusqlite::{Connection, OptionalExtension, params};
use std::{path::Path, time::Duration};

mod calendar;
mod messages;
mod migration;
pub use calendar::EventFilter;
pub use messages::MessageFilter;

/// One SQLite connection. Mutating multi-row operations use native transactions.
pub struct Store {
    pub(crate) connection: Connection,
}
impl Store {
    pub fn open(path: &Path) -> Result<Self> {
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            let mut builder = std::fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            builder.create(parent)?;
        }
        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        match options.open(path) {
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
        let mut connection = Connection::open(path)?;
        connection.busy_timeout(Duration::from_secs(5))?;
        migration::initialize(&mut connection)?;
        connection.execute_batch("PRAGMA foreign_keys=ON; PRAGMA journal_mode=WAL;")?;
        Ok(Self { connection })
    }
    pub fn topic(&self, slug: &str) -> Result<Topic> {
        self.connection
            .query_row(&format!("{TOPIC_SELECT} WHERE slug=?1"), [slug], read_topic)
            .optional()?
            .ok_or_else(|| Error::TopicNotFound(slug.into()))
    }
    pub fn topics(&self) -> Result<Vec<Topic>> {
        Ok(self
            .connection
            .prepare(&format!("{TOPIC_SELECT} ORDER BY slug"))?
            .query_map([], read_topic)?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn create_topic(&mut self, slug: &str, display_name: &str, query: &str) -> Result<Topic> {
        if slug.trim().is_empty() {
            return Err(Error::Invalid("topic slug must contain text".into()));
        }
        let name = if display_name.is_empty() {
            slug
        } else {
            display_name
        };
        let tx = self.connection.transaction()?;
        tx.execute(
            "INSERT INTO topics(slug,display_name,query) VALUES (?1,?2,?3)",
            params![slug, name, query],
        )?;
        tx.execute(
            "INSERT INTO topic_queries(topic_id,query_string) VALUES (?1,?2)",
            params![
                tx.last_insert_rowid(),
                if query.is_empty() { name } else { query }
            ],
        )?;
        tx.commit()?;
        self.topic(slug)
    }
    pub fn delete_topic(&mut self, slug: &str) -> Result<()> {
        if self
            .connection
            .execute("DELETE FROM topics WHERE slug=?1", [slug])?
            == 0
        {
            return Err(Error::TopicNotFound(slug.into()));
        }
        Ok(())
    }
    pub fn add_query(&mut self, topic_id: i64, query: &str) -> Result<()> {
        if query.trim().is_empty() {
            return Err(Error::Invalid("Gmail query must contain text".into()));
        }
        self.connection.execute(
            "INSERT OR IGNORE INTO topic_queries(topic_id,query_string) VALUES (?1,?2)",
            params![topic_id, query],
        )?;
        Ok(())
    }
    pub fn queries(&self, topic_id: i64) -> Result<Vec<String>> {
        Ok(self
            .connection
            .prepare("SELECT query_string FROM topic_queries WHERE topic_id=?1 ORDER BY id")?
            .query_map([topic_id], |r| r.get(0))?
            .collect::<rusqlite::Result<_>>()?)
    }
    pub fn has_thread_history(&self, id: &str, history: &str) -> Result<bool> {
        if history.is_empty() {
            return Ok(false);
        }
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM threads WHERE id=?1 AND history_id=?2)",
            params![id, history],
            |r| r.get(0),
        )?)
    }
    pub fn attach_thread(&mut self, id: &str, topic_id: i64) -> Result<()> {
        self.connection.execute(
            "INSERT OR IGNORE INTO thread_topics(thread_id,topic_id) VALUES (?1,?2)",
            params![id, topic_id],
        )?;
        Ok(())
    }
    pub fn ingest_thread(
        &mut self,
        detail: &ThreadDetail,
        topic_id: Option<i64>,
        history: &str,
        snippet: &str,
        rules: &Classifier,
        now: DateTime<Utc>,
    ) -> Result<usize> {
        let messages = convert_thread(detail, rules, now)?;
        let tx = self.connection.transaction()?;
        tx.execute("INSERT INTO threads(id,history_id,snippet,last_synced_at) VALUES (?1,?2,?3,?4) ON CONFLICT(id) DO UPDATE SET history_id=excluded.history_id,snippet=excluded.snippet,last_synced_at=excluded.last_synced_at", params![detail.id,history,snippet,now])?;
        if let Some(topic) = topic_id {
            tx.execute(
                "INSERT OR IGNORE INTO thread_topics(thread_id,topic_id) VALUES (?1,?2)",
                params![detail.id, topic],
            )?;
        }
        for m in &messages {
            tx.execute("INSERT INTO messages(id,thread_id,internal_date,date_iso,from_address,from_name,to_address,subject,snippet,body_plain,is_automated,classification_reason,created_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13) ON CONFLICT(id) DO UPDATE SET thread_id=excluded.thread_id,internal_date=excluded.internal_date,date_iso=excluded.date_iso,from_address=excluded.from_address,from_name=excluded.from_name,to_address=excluded.to_address,subject=excluded.subject,snippet=excluded.snippet,body_plain=excluded.body_plain,is_automated=excluded.is_automated,classification_reason=excluded.classification_reason", params![m.id,m.thread_id,m.internal_date,m.date_iso,m.from_address,m.from_name,m.to_address,m.subject,m.snippet,m.body_plain,m.is_automated,m.classification_reason,m.created_at])?;
        }
        tx.commit()?;
        Ok(messages.len())
    }
    pub fn max_message_date(&self, topic_id: i64) -> Result<i64> {
        Ok(self.connection.query_row("SELECT COALESCE(MAX(m.internal_date),0) FROM messages m JOIN thread_topics t ON t.thread_id=m.thread_id WHERE t.topic_id=?1", [topic_id], |r| r.get(0))?)
    }
    pub fn update_topic_state(
        &mut self,
        topic_id: i64,
        history: &str,
        watermark: i64,
        now: DateTime<Utc>,
    ) -> Result<()> {
        self.connection.execute("UPDATE topics SET last_synced_at=?1,last_history_id=CASE WHEN ?2='' THEN last_history_id ELSE ?2 END,last_message_internal_date=MAX(last_message_internal_date,?3),updated_at=?1 WHERE id=?4", params![now,history,watermark,topic_id])?;
        Ok(())
    }
    pub fn start_run(&mut self, topic_id: Option<i64>, now: DateTime<Utc>) -> Result<i64> {
        self.connection.execute(
            "INSERT INTO sync_runs(topic_id,started_at,status) VALUES (?1,?2,'running')",
            params![topic_id, now],
        )?;
        Ok(self.connection.last_insert_rowid())
    }
    pub fn finish_run(
        &mut self,
        id: i64,
        result: &crate::model::MailSyncResult,
        failure: Option<&str>,
        now: DateTime<Utc>,
    ) -> Result<()> {
        let status = if failure.is_some() {
            "failed"
        } else if result.threads_remaining > 0 {
            "partial"
        } else {
            "success"
        };
        let fetched = i64::try_from(result.threads_fetched)
            .map_err(|_| Error::Invalid("thread count exceeds SQLite integer range".into()))?;
        let ingested = i64::try_from(result.messages_ingested)
            .map_err(|_| Error::Invalid("message count exceeds SQLite integer range".into()))?;
        self.connection.execute("UPDATE sync_runs SET completed_at=?1,status=?2,threads_fetched=?3,messages_ingested=?4,new_max_history_id=?5,error_message=?6 WHERE id=?7", params![now,status,fetched,ingested,result.max_history_id,failure,id])?;
        Ok(())
    }
    pub fn stats(&self) -> Result<Stats> {
        Ok(self.connection.query_row("SELECT (SELECT COUNT(*) FROM topics),(SELECT COUNT(*) FROM threads),(SELECT COUNT(*) FROM messages),(SELECT COUNT(*) FROM calendar_events)", [], |r| Ok(Stats { topics: r.get(0)?,threads:r.get(1)?,messages:r.get(2)?,calendar_events:r.get(3)? }))?)
    }
}
const TOPIC_SELECT: &str = "SELECT id,slug,display_name,query,last_synced_at,COALESCE(last_history_id,''),COALESCE(last_message_internal_date,0),created_at,updated_at FROM topics";
fn read_topic(row: &rusqlite::Row<'_>) -> rusqlite::Result<Topic> {
    Ok(Topic {
        id: row.get(0)?,
        slug: row.get(1)?,
        display_name: row.get(2)?,
        query: row.get(3)?,
        last_synced_at: row.get(4)?,
        last_history_id: row.get(5)?,
        last_message_internal_date: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}
