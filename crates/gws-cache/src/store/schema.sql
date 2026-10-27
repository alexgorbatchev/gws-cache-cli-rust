CREATE TABLE IF NOT EXISTS topics (
 id INTEGER PRIMARY KEY AUTOINCREMENT, slug TEXT NOT NULL UNIQUE, display_name TEXT NOT NULL,
 query TEXT NOT NULL DEFAULT '', last_synced_at DATETIME, last_history_id TEXT DEFAULT '',
 last_message_internal_date INTEGER DEFAULT 0, created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
 updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS topic_queries (
 id INTEGER PRIMARY KEY AUTOINCREMENT, topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE,
 query_string TEXT NOT NULL, created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, UNIQUE(topic_id,query_string)
);
CREATE TABLE IF NOT EXISTS threads (
 id TEXT PRIMARY KEY, history_id TEXT NOT NULL, snippet TEXT, last_synced_at DATETIME NOT NULL
);
CREATE TABLE IF NOT EXISTS thread_topics (
 thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
 topic_id INTEGER NOT NULL REFERENCES topics(id) ON DELETE CASCADE, PRIMARY KEY(thread_id,topic_id)
);
CREATE TABLE IF NOT EXISTS messages (
 id TEXT PRIMARY KEY, thread_id TEXT NOT NULL REFERENCES threads(id) ON DELETE CASCADE,
 internal_date INTEGER NOT NULL, date_iso TEXT NOT NULL, from_address TEXT NOT NULL, from_name TEXT,
 to_address TEXT NOT NULL, subject TEXT NOT NULL, snippet TEXT NOT NULL, body_plain TEXT,
 is_automated BOOLEAN NOT NULL DEFAULT 0, classification_reason TEXT, created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE TABLE IF NOT EXISTS calendar_events (
 id TEXT NOT NULL, calendar_id TEXT NOT NULL, topic_id INTEGER REFERENCES topics(id) ON DELETE SET NULL,
 summary TEXT NOT NULL, description TEXT, location TEXT, organizer_email TEXT, start_time DATETIME, end_time DATETIME,
 status TEXT NOT NULL, is_deleted BOOLEAN NOT NULL DEFAULT 0, attendees_json TEXT, html_link TEXT, i_cal_uid TEXT,
 all_day BOOLEAN NOT NULL DEFAULT 0, created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
 updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP, PRIMARY KEY(calendar_id,id)
);
CREATE TABLE IF NOT EXISTS calendar_sync_state (
 calendar_id TEXT PRIMARY KEY, sync_token TEXT NOT NULL, last_synced_at DATETIME NOT NULL,
 window_start DATETIME, window_end DATETIME
);
CREATE TABLE IF NOT EXISTS sync_runs (
 id INTEGER PRIMARY KEY AUTOINCREMENT, topic_id INTEGER REFERENCES topics(id) ON DELETE SET NULL,
 started_at DATETIME NOT NULL, completed_at DATETIME, status TEXT NOT NULL,
 threads_fetched INTEGER NOT NULL DEFAULT 0, messages_ingested INTEGER NOT NULL DEFAULT 0,
 new_max_history_id TEXT, error_message TEXT
);
CREATE INDEX IF NOT EXISTS idx_thread_topics_topic ON thread_topics(topic_id);
CREATE INDEX IF NOT EXISTS idx_messages_date ON messages(internal_date);
CREATE INDEX IF NOT EXISTS idx_calendar_events_window ON calendar_events(start_time,end_time);
CREATE INDEX IF NOT EXISTS idx_calendar_events_topic ON calendar_events(topic_id);
