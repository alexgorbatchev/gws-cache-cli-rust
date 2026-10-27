#![cfg(unix)]
use chrono::Utc;
use gws_cache::{
    CalendarOptions, Classifier, Client, Error, MailOptions, Rules, Store, model::CalendarParams,
    sync_all_topics, sync_topic,
};
use std::time::Duration;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn invalid_or_cancelled_sync_does_not_register_topics() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    let client = Client::new(temp.path().join("missing"), Duration::from_secs(1)).unwrap();
    let rules = Classifier::new(Rules::default()).unwrap();
    let cancel = CancellationToken::new();
    let invalid = MailOptions {
        max_threads: 0,
        ..Default::default()
    };
    assert!(matches!(
        sync_topic(
            &mut store,
            &client,
            &rules,
            "new",
            &invalid,
            &cancel,
            Utc::now()
        )
        .await,
        Err(Error::Invalid(_))
    ));
    assert!(store.topics().unwrap().is_empty());
    cancel.cancel();
    assert!(matches!(
        sync_topic(
            &mut store,
            &client,
            &rules,
            "new",
            &MailOptions::default(),
            &cancel,
            Utc::now()
        )
        .await,
        Err(Error::Cancelled)
    ));
    assert!(store.topics().unwrap().is_empty());
    assert!(matches!(
        sync_all_topics(
            &mut store,
            &client,
            &rules,
            &MailOptions::default(),
            &cancel,
            Utc::now()
        )
        .await,
        Err(Error::Cancelled)
    ));
}

#[tokio::test]
async fn incompatible_calendar_cursor_bounds_fail_before_spawning() {
    let temp = tempfile::tempdir().unwrap();
    let client = Client::new(temp.path().join("missing"), Duration::from_secs(1)).unwrap();
    for (time_min, time_max) in [
        (Some("2026-10-01T00:00:00Z".into()), None),
        (None, Some("2026-11-01T00:00:00Z".into())),
    ] {
        let params = CalendarParams {
            sync_token: Some("cursor".into()),
            time_min,
            time_max,
            ..Default::default()
        };
        assert!(matches!(
            client
                .calendar_events(params, &CancellationToken::new())
                .await,
            Err(Error::Invalid(_))
        ));
    }
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    let options = CalendarOptions {
        past: "2026-11-01".parse().unwrap(),
        future: "2026-10-01".parse().unwrap(),
        ..Default::default()
    };
    assert!(matches!(
        gws_cache::sync_calendar(
            &mut store,
            &client,
            &options,
            &CancellationToken::new(),
            Utc::now()
        )
        .await,
        Err(Error::Invalid(_))
    ));
}
