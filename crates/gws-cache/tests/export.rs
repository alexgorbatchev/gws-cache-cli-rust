use chrono::{TimeZone, Utc};
use gws_cache::{
    Classifier, ExportFormat, ExportOptions, Rules, Store, Window, export_topic,
    model::ThreadDetail,
};
use serde_json::{Value, json};

#[test]
fn exports_chronological_human_timeline_and_json_empty_array() {
    let temp = tempfile::tempdir().unwrap();
    let mut store = Store::open(&temp.path().join("cache.db")).unwrap();
    let rules = Classifier::new(Rules::default()).unwrap();
    let topic = store.create_topic("work", "Work", "").unwrap();
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
    let options = ExportOptions {
        since: Window::Unbounded,
        ..Default::default()
    };
    assert_eq!(
        export_topic(&store, "work", &options, now).unwrap(),
        "### Work Timeline\n\n*(No messages found)*\n"
    );
    let json_options = ExportOptions {
        format: ExportFormat::Json,
        ..options
    };
    assert_eq!(
        export_topic(&store, "work", &json_options, now).unwrap(),
        "[]"
    );
    assert!(export_topic(&store, "missing", &json_options, now).is_err());
    let thread:ThreadDetail=serde_json::from_value(json!({"id":"t","messages":[
        {"id":"new","internalDate":"1770000001000","snippet":"Hello &copy; &amp; world&#10;next\r\nline", "payload":{"headers":[{"name":"From","value":"Alice <alice@example.com>"}]}},
        {"id":"old","internalDate":"1770000000000","snippet":"Earlier", "payload":{"headers":[{"name":"From","value":"bob@example.com"}]}},
        {"id":"auto","internalDate":"1770000002000","snippet":"Automated", "payload":{"headers":[{"name":"From","value":"noreply@example.com"}]}}
    ]})).unwrap();
    store
        .ingest_thread(&thread, Some(topic.id), "1", "", &rules, now)
        .unwrap();
    let markdown = export_topic(
        &store,
        "work",
        &ExportOptions {
            since: Window::Unbounded,
            ..Default::default()
        },
        now,
    )
    .unwrap();
    assert!(markdown.contains("Alice: Hello © & world next  line"));
    assert!(markdown.find("Earlier").unwrap() < markdown.find("Alice").unwrap());
    assert!(!markdown.contains("Automated"));
    let value: Value =
        serde_json::from_str(&export_topic(&store, "work", &json_options, now).unwrap()).unwrap();
    assert_eq!(value.as_array().unwrap().len(), 2);
    assert_eq!(value[0]["id"], "old");
    assert_eq!(value[0]["from"], "bob@example.com");
    assert!(
        value[0]["date_display"]
            .as_str()
            .unwrap()
            .starts_with("Feb")
    );
    assert!(
        !export_topic(&store, "work", &ExportOptions::default(), now)
            .unwrap()
            .contains("Earlier")
    );
}
