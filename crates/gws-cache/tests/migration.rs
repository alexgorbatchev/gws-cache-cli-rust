use gws_cache::{EventFilter, MessageFilter, Store};
use rusqlite::{Connection, params};

fn legacy(path: &std::path::Path) {
    let connection = Connection::open(path).unwrap();
    connection
        .execute_batch(include_str!("fixtures/go-schema.sql"))
        .unwrap();
    connection.execute_batch("INSERT INTO topics(id,slug,display_name) VALUES (1,'first','First'),(2,'second','Second');
        INSERT INTO topic_queries(topic_id,query_string) VALUES (1,'label:one');
        INSERT INTO threads(id,topic_id,history_id,snippet,last_synced_at) VALUES ('t',1,'99','snippet','2026-10-14 12:00:00 +0000 UTC m=+0.001');
        INSERT INTO messages(id,thread_id,topic_id,internal_date,date_iso,from_address,to_address,subject,snippet) VALUES ('m','t',2,0,'1970-01-01T00:00:00Z','a@example.com','','Message','cached');
        INSERT INTO calendar_events(id,calendar_id,summary,start_time,end_time,status,attendees_json) VALUES ('e','primary','Old event','2026-10-14 12:00:00 -0700 PDT','2026-10-14 13:00:00 -0700 PDT','confirmed','[]');
        INSERT INTO calendar_sync_state VALUES ('primary','cursor','2026-10-14 12:00:00 +0000 UTC','2026-10-01 00:00:00 +0000 UTC','2026-11-01 00:00:00 +0000 UTC');
        INSERT INTO sync_runs(topic_id,started_at,status) VALUES (1,'2026-10-14 12:00:00 +0000 UTC','running');").unwrap();
}

#[test]
fn migrates_actual_go_schema_and_timestamp_encodings_without_losing_membership() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cache.db");
    legacy(&path);
    let store = Store::open(&path).unwrap();
    let messages = store.messages(&MessageFilter::default()).unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].topic_ids, vec![1, 2]);
    assert_eq!(store.queries(1).unwrap(), vec!["label:one"]);
    let event = store.events(&EventFilter::default()).unwrap().remove(0);
    assert_eq!(
        event.start_time.unwrap().to_rfc3339(),
        "2026-10-14T19:00:00+00:00"
    );
    assert_eq!(
        store.calendar_state("primary").unwrap().unwrap().sync_token,
        "cursor"
    );
    drop(store);
    assert_eq!(Store::open(&path).unwrap().stats().unwrap().messages, 1);
    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row(
                "SELECT COUNT(*) FROM pragma_table_info('messages') WHERE name='topic_id'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
    assert_eq!(
        connection
            .query_row("SELECT COUNT(*) FROM sync_runs", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        1
    );
}

#[test]
fn failed_migration_rolls_back_and_future_schemas_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cache.db");
    legacy(&path);
    {
        let connection = Connection::open(&path).unwrap();
        connection
            .execute(
                "UPDATE calendar_events SET start_time=?1",
                ["not a timestamp"],
            )
            .unwrap();
    }
    assert!(Store::open(&path).is_err());
    let connection = Connection::open(&path).unwrap();
    assert_eq!(
        connection
            .query_row("SELECT topic_id FROM messages", [], |r| r.get::<_, i64>(0))
            .unwrap(),
        2
    );
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |r| r.get::<_, u32>(0))
            .unwrap(),
        0
    );
    connection
        .execute(
            "UPDATE calendar_events SET start_time=?1",
            ["2026-10-14 12:00:00"],
        )
        .unwrap();
    connection
        .execute_batch("PRAGMA foreign_keys=OFF;")
        .unwrap();
    connection
        .execute("UPDATE threads SET topic_id=?1", params![9999])
        .unwrap();
    drop(connection);
    assert!(Store::open(&path).is_err());
    let future = temp.path().join("future.db");
    Connection::open(&future)
        .unwrap()
        .execute_batch("PRAGMA user_version=99;")
        .unwrap();
    assert!(
        matches!(Store::open(&future),Err(gws_cache::Error::Invalid(message)) if message.contains("99"))
    );
}
