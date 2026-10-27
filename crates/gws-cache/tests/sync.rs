#![cfg(unix)]
use chrono::{TimeZone, Timelike, Utc};
use gws_cache::{
    CalendarOptions, Classifier, Client, EventFilter, MailOptions, MessageFilter, Rules, Store,
    sync_all_topics, sync_calendar, sync_query, sync_topic,
};
use std::{os::unix::fs::PermissionsExt, time::Duration};
use tokio_util::sync::CancellationToken;

static EXECUTABLE_FIXTURES: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

fn program(temp: &tempfile::TempDir, script: &str) -> Client {
    let path = temp.path().join("gws");
    let staged = temp.path().join("gws.next");
    std::fs::write(&staged, format!("#!/bin/sh\n{script}\n")).unwrap();
    std::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o700)).unwrap();
    std::fs::rename(staged, &path).unwrap();
    Client::new(path, Duration::from_secs(3)).unwrap()
}
const MAIL: &str = r#"case "$*" in
*"threads list"*) printf '%s\n' '{"threads":[{"id":"first","historyId":"99"},{"id":"second","historyId":"100"},{"id":"first","historyId":"99"}]}' ;;
*'"id":"first"'*) printf '%s\n' '{"id":"first","historyId":"99","messages":[{"id":"m1","internalDate":"1770000000000","snippet":"Hello"}]}' ;;
*) printf '%s\n' '{"id":"second","historyId":"100","messages":[{"id":"m2","internalDate":"1770000001000"}]}' ;;
esac"#;

#[tokio::test]
async fn mail_budget_drains_and_force_full_really_refetches() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    let client = program(&temp, MAIL);
    let rules = Classifier::new(Rules::default()).unwrap();
    let cancel = CancellationToken::new();
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
    let options = MailOptions {
        max_threads: 1,
        ..Default::default()
    };
    let first = sync_topic(&mut store, &client, &rules, "work", &options, &cancel, now)
        .await
        .unwrap();
    assert_eq!(
        (
            first.threads_listed,
            first.threads_fetched,
            first.threads_remaining
        ),
        (2, 1, 1)
    );
    assert_eq!(store.topic("work").unwrap().last_message_internal_date, 0);
    let second = sync_topic(&mut store, &client, &rules, "work", &options, &cancel, now)
        .await
        .unwrap();
    assert_eq!((second.threads_fetched, second.threads_remaining), (1, 0));
    assert_eq!(
        store.topic("work").unwrap().last_message_internal_date,
        1770000001000
    );
    let third = sync_topic(&mut store, &client, &rules, "work", &options, &cancel, now)
        .await
        .unwrap();
    assert_eq!((third.threads_fetched, third.threads_remaining), (0, 0));
    let force = MailOptions {
        force_full: true,
        max_threads: 3,
        ..Default::default()
    };
    let result = sync_topic(&mut store, &client, &rules, "work", &force, &cancel, now)
        .await
        .unwrap();
    assert_eq!(result.threads_fetched, 2);
    assert_eq!(result.max_history_id, "100");
    sync_topic(&mut store, &client, &rules, "other", &options, &cancel, now)
        .await
        .unwrap();
    assert_eq!(
        store
            .messages(&MessageFilter {
                topic: Some("other".into()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        sync_all_topics(&mut store, &client, &rules, &options, &cancel, now)
            .await
            .unwrap()
            .len(),
        2
    );
    let query = sync_query(
        &mut store,
        &client,
        &rules,
        "is:unread",
        &options,
        &cancel,
        now,
    )
    .await
    .unwrap();
    assert_eq!(query.threads_listed, 2);
    assert_eq!(store.stats().unwrap().topics, 2);
}

#[tokio::test]
async fn failed_get_is_backlog_and_holds_watermark_and_cutoff_uses_seconds() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    let client = program(
        &temp,
        r#"case "$*" in
*"threads list"*) case "$*" in *after:0*) printf '%s\n' '{"threads":[{"id":"bad","historyId":"1"}]}' ;; *) exit 7 ;; esac ;;
*) printf '%s\n' '{"error":{"code":403,"message":"Forbidden"}}'; exit 1 ;;
esac"#,
    );
    let rules = Classifier::new(Rules::default()).unwrap();
    let cancel = CancellationToken::new();
    let now = Utc::now();
    let options = MailOptions {
        since: "1970-01-01".parse().unwrap(),
        since_explicit: true,
        ..Default::default()
    };
    let result = sync_topic(&mut store, &client, &rules, "work", &options, &cancel, now)
        .await
        .unwrap();
    assert_eq!(result.threads_remaining, 1);
    assert_eq!(result.warnings.len(), 1);
    assert_eq!(store.topic("work").unwrap().last_message_internal_date, 0);
    assert!(
        store
            .messages(&MessageFilter::default())
            .unwrap()
            .is_empty()
    );
    let empty = MailOptions {
        max_threads: 0,
        ..Default::default()
    };
    assert!(
        sync_topic(&mut store, &client, &rules, "work", &empty, &cancel, now)
            .await
            .is_err()
    );
    cancel.cancel();
    assert!(
        sync_all_topics(&mut store, &client, &rules, &options, &cancel, now)
            .await
            .is_err()
    );
}

const EVENT: &str = r#"printf '%s\n' '{"items":[{"id":"same","summary":"Project Alpha review","status":"confirmed","start":{"dateTime":"2026-10-14T10:00:00-07:00"},"end":{"dateTime":"2026-10-14T11:00:00-07:00"},"attendees":[{"email":"a@example.com"}]}],"nextSyncToken":"next"}'"#;

#[tokio::test]
async fn calendar_composite_identity_and_sparse_cancellation_preserve_details() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    store.create_topic("alpha", "Project Alpha", "").unwrap();
    store.create_topic("project", "Project", "").unwrap();
    let cancel = CancellationToken::new();
    let client = program(&temp, EVENT);
    let now = Utc::now();
    let options = CalendarOptions::default();
    assert_eq!(
        sync_calendar(&mut store, &client, &options, &cancel, now)
            .await
            .unwrap()
            .events_ingested,
        1
    );
    let other = CalendarOptions {
        calendar_id: "other".into(),
        ..Default::default()
    };
    sync_calendar(&mut store, &client, &other, &cancel, now)
        .await
        .unwrap();
    assert_eq!(store.events(&EventFilter::default()).unwrap().len(), 2);
    let before = store.event("primary", "same").unwrap().unwrap();
    assert_eq!(before.topic_id, Some(store.topic("alpha").unwrap().id));
    program(
        &temp,
        r#"case "$*" in *'"syncToken":"next"'*) printf '%s\n' '{"items":[{"id":"same","status":"cancelled"}],"nextSyncToken":"cancelled"}' ;; *) exit 9 ;; esac"#,
    );
    let result = sync_calendar(&mut store, &client, &options, &cancel, now)
        .await
        .unwrap();
    assert_eq!(result.events_deleted, 1);
    let after = store.event("primary", "same").unwrap().unwrap();
    assert!(after.is_deleted);
    assert_eq!(after.summary, before.summary);
    assert_eq!(after.start_time, before.start_time);
    assert!(!store.event("other", "same").unwrap().unwrap().is_deleted);
    assert!(
        !serde_json::to_string(&result)
            .unwrap()
            .contains("cancelled")
    );
}

#[tokio::test]
async fn calendar_only_410_resets_and_invalid_payload_never_commits_cursor() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    let client = program(&temp, EVENT);
    let cancel = CancellationToken::new();
    let now = Utc::now();
    let options = CalendarOptions::default();
    sync_calendar(&mut store, &client, &options, &cancel, now)
        .await
        .unwrap();
    program(
        &temp,
        "printf '%s\\n' '{\"error\":{\"code\":403,\"message\":\"Forbidden\"}}'; exit 1",
    );
    assert!(
        sync_calendar(&mut store, &client, &options, &cancel, now)
            .await
            .is_err()
    );
    assert_eq!(
        store.calendar_state("primary").unwrap().unwrap().sync_token,
        "next"
    );
    program(
        &temp,
        r#"case "$*" in *syncToken*) printf '%s\n' '{"error":{"code":410,"message":"Expired"}}'; exit 1 ;; *) printf '%s\n' '{"items":[],"nextSyncToken":"fresh"}' ;; esac"#,
    );
    sync_calendar(&mut store, &client, &options, &cancel, now)
        .await
        .unwrap();
    assert!(store.events(&EventFilter::default()).unwrap().is_empty());
    assert_eq!(
        store.calendar_state("primary").unwrap().unwrap().sync_token,
        "fresh"
    );
    program(
        &temp,
        r#"printf '%s\n' '{"items":[{"id":"bad","status":"confirmed","start":{"date":"nonsense"}}],"nextSyncToken":"invalid"}'"#,
    );
    assert!(
        sync_calendar(&mut store, &client, &options, &cancel, now)
            .await
            .is_err()
    );
    assert_eq!(
        store.calendar_state("primary").unwrap().unwrap().sync_token,
        "fresh"
    );
    program(&temp, r#"printf '%s\n' '{"items":[]}'"#);
    assert!(
        sync_calendar(&mut store, &client, &options, &cancel, now)
            .await
            .is_err()
    );
    assert_eq!(
        store.calendar_state("primary").unwrap().unwrap().sync_token,
        "fresh"
    );
}

#[tokio::test]
async fn calendar_civil_dates_timezones_filters_and_atomic_rollback() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    use chrono::{TimeDelta, TimeZone};
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cache.db");
    let mut store = Store::open(&path).unwrap();
    let topic = store.create_topic("alpha", "Project Alpha", "").unwrap();
    let client = program(
        &temp,
        r#"printf '%s\n' '{"items":[{"id":"day","summary":"Project Alpha 100%_ complete","status":"confirmed","start":{"date":"2026-10-14"},"end":{"date":"2026-10-15"}},{"id":"local","status":"tentative","start":{"dateTime":"2026-10-14T10:00:00","timeZone":"America/Los_Angeles"},"end":{"dateTime":"2026-10-14T11:00:00","timeZone":"America/Los_Angeles"}},{"id":"gone","status":"cancelled"}],"nextSyncToken":"civil"}'"#,
    );
    let cancel = CancellationToken::new();
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
    let options = CalendarOptions::default();
    sync_calendar(&mut store, &client, &options, &cancel, now)
        .await
        .unwrap();
    let day = store.event("primary", "day").unwrap().unwrap();
    assert!(day.all_day);
    assert_eq!(
        day.start_time.unwrap(),
        Utc.with_ymd_and_hms(2026, 10, 14, 0, 0, 0).unwrap()
    );
    assert_eq!(
        store
            .event("primary", "local")
            .unwrap()
            .unwrap()
            .start_time
            .unwrap()
            .hour(),
        17
    );
    assert!(
        store
            .event("primary", "gone")
            .unwrap()
            .unwrap()
            .start_time
            .is_none()
    );
    let filter = EventFilter {
        calendar: Some("primary".into()),
        topic: Some("alpha".into()),
        keyword: "100%_".into(),
        since: Some(now - TimeDelta::days(1)),
    };
    assert_eq!(store.events(&filter).unwrap().len(), 1);
    assert!(
        store
            .events(&EventFilter {
                keyword: "100x".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    assert!(
        store
            .events(&EventFilter {
                calendar: Some("other".into()),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    let state = store.calendar_state("primary").unwrap().unwrap();
    let mut wrong_calendar = day.clone();
    wrong_calendar.calendar_id = "other".into();
    assert!(
        store
            .commit_calendar(&[wrong_calendar], &state, true)
            .is_err()
    );
    assert!(store.event("other", "day").unwrap().is_none());
    assert!(store.event("primary", "day").unwrap().is_some());
    let mut invalid = day.clone();
    invalid.topic_id = Some(topic.id + 1000);
    invalid.summary = "must roll back".into();
    assert!(store.commit_calendar(&[invalid], &state, true).is_err());
    assert_eq!(
        store.event("primary", "day").unwrap().unwrap().summary,
        day.summary
    );
    store.delete_topic("alpha").unwrap();
    assert!(
        store
            .event("primary", "day")
            .unwrap()
            .unwrap()
            .topic_id
            .is_none()
    );
    for (id, start, end) in [
        (
            "ambiguous",
            r#"{"dateTime":"2026-11-01T01:30:00","timeZone":"America/Los_Angeles"}"#,
            r#"{"dateTime":"2026-11-01T02:30:00","timeZone":"America/Los_Angeles"}"#,
        ),
        ("missing", r#"{}"#, r#"{}"#),
        (
            "backward",
            r#"{"date":"2026-10-15"}"#,
            r#"{"date":"2026-10-14"}"#,
        ),
        (
            "bad-zone",
            r#"{"dateTime":"2026-10-14T10:00:00","timeZone":"Invalid/Zone"}"#,
            r#"{"date":"2026-10-15"}"#,
        ),
    ] {
        program(
            &temp,
            &format!(
                "printf '%s\\n' '{{\"items\":[{{\"id\":\"{id}\",\"status\":\"confirmed\",\"start\":{start},\"end\":{end}}}],\"nextSyncToken\":\"bad\"}}'"
            ),
        );
        assert!(
            sync_calendar(&mut store, &client, &options, &cancel, now)
                .await
                .is_err(),
            "{id}"
        );
        assert_eq!(
            store.calendar_state("primary").unwrap().unwrap().sync_token,
            "civil"
        );
    }
}

#[tokio::test]
async fn mail_run_failures_are_recorded_and_all_topics_continue() {
    let _fixture_lock = EXECUTABLE_FIXTURES.lock().await;
    use gws_cache::Error;
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cache.db");
    let mut store = Store::open(&path).unwrap();
    store.create_topic("broken", "Broken", "broken").unwrap();
    store.create_topic("good", "Good", "good").unwrap();
    let client = program(
        &temp,
        r#"case "$*" in *broken*) printf '%s\n' '{"error":{"code":403,"message":"Forbidden"}}'; exit 1 ;; *) printf '%s\n' '{"threads":[]}' ;; esac"#,
    );
    let rules = Classifier::new(Rules::default()).unwrap();
    let options = MailOptions::default();
    let cancel = CancellationToken::new();
    let results = sync_all_topics(&mut store, &client, &rules, &options, &cancel, Utc::now())
        .await
        .unwrap();
    assert!(results[0].error.as_ref().unwrap().contains("Forbidden"));
    assert!(results[1].error.is_none());
    let connection = rusqlite::Connection::open(&path).unwrap();
    let statuses = connection
        .prepare("SELECT status FROM sync_runs ORDER BY id")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<rusqlite::Result<Vec<_>>>()
        .unwrap();
    assert_eq!(statuses, vec!["failed", "success"]);
    assert!(matches!(
        sync_query(
            &mut store,
            &client,
            &rules,
            " ",
            &options,
            &cancel,
            Utc::now()
        )
        .await,
        Err(Error::Invalid(_))
    ));
    connection.execute_batch("CREATE TRIGGER reject_finish BEFORE UPDATE ON sync_runs BEGIN SELECT RAISE(FAIL,'journal unavailable'); END;").unwrap();
    assert!(matches!(
        sync_topic(
            &mut store,
            &client,
            &rules,
            "broken",
            &options,
            &cancel,
            Utc::now()
        )
        .await,
        Err(Error::Recording { .. })
    ));
    assert!(matches!(
        sync_topic(
            &mut store,
            &client,
            &rules,
            "good",
            &options,
            &cancel,
            Utc::now()
        )
        .await,
        Err(Error::Sqlite(_))
    ));
}
