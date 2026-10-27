use chrono::{TimeZone, Utc};
use gws_cache::{Classifier, MessageFilter, Rules, Store, model::ThreadDetail};
use serde_json::json;

fn thread(id: &str, history: &str, date: &str) -> ThreadDetail {
    serde_json::from_value(json!({"id":id,"historyId":history,"messages":[{"id":format!("{id}-m"),"internalDate":date,"snippet":"100% of a_test", "payload":{"headers":[{"name":"Subject","value":"Meeting"},{"name":"From","value":"Human <human@example.com>"}]}}]})).unwrap()
}

#[test]
fn atomic_thread_ingest_and_overlapping_topic_membership() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("private/cache.db");
    let mut store = Store::open(&path).unwrap();
    let rules = Classifier::new(Rules::default()).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
    let first = store.create_topic("first", "First", "label:one").unwrap();
    let second = store.create_topic("second", "", "").unwrap();
    assert_eq!(second.display_name, "second");
    store.add_query(first.id, "label:two").unwrap();
    store.add_query(first.id, "label:two").unwrap();
    assert_eq!(
        store.queries(first.id).unwrap(),
        vec!["label:one", "label:two"]
    );
    assert!(store.create_topic("first", "", "").is_err());
    assert!(store.create_topic(" ", "", "").is_err());
    let detail = thread("t", "99", "0");
    store
        .ingest_thread(&detail, Some(first.id), "99", "preview", &rules, now)
        .unwrap();
    store.attach_thread("t", second.id).unwrap();
    assert_eq!(
        store
            .messages(&MessageFilter {
                topic: Some("second".into()),
                ..Default::default()
            })
            .unwrap()[0]
            .topic_ids,
        vec![first.id, second.id]
    );
    assert!(store.has_thread_history("t", "99").unwrap());
    assert!(!store.has_thread_history("t", "").unwrap());
    let mut bad = thread("t", "100", "1000");
    bad.messages
        .push(thread("bad", "100", "broken").messages.remove(0));
    assert!(
        store
            .ingest_thread(&bad, Some(first.id), "100", "changed", &rules, now)
            .is_err()
    );
    assert!(store.has_thread_history("t", "99").unwrap());
    assert!(!store.has_thread_history("t", "100").unwrap());
    assert_eq!(store.stats().unwrap().messages, 1);
    assert!(
        store
            .ingest_thread(&detail, Some(9999), "100", "", &rules, now)
            .is_err()
    );
    assert!(store.has_thread_history("t", "99").unwrap());
    assert!(
        store
            .messages(&MessageFilter {
                since: Some(now),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .messages(&MessageFilter {
                since: Some(Utc.timestamp_opt(0, 0).unwrap()),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    assert!(
        store
            .messages(&MessageFilter {
                keyword: "missing%".into(),
                ..Default::default()
            })
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        store
            .messages(&MessageFilter {
                keyword: "100%".into(),
                ..Default::default()
            })
            .unwrap()
            .len(),
        1
    );
    store.delete_topic("first").unwrap();
    assert_eq!(
        store.messages(&MessageFilter::default()).unwrap()[0].topic_ids,
        vec![second.id]
    );
    assert!(store.delete_topic("first").is_err());
    assert_eq!(store.topics().unwrap().len(), 1);
    assert_eq!(store.stats().unwrap().threads, 1);
    drop(store);
    assert_eq!(Store::open(&path).unwrap().stats().unwrap().messages, 1);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(
            std::fs::metadata(path.parent().unwrap())
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o700
        );
    }
}
