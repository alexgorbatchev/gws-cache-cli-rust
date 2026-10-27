use chrono::{TimeZone, Utc};
use gws_cache::{Classifier, Rules, Window, convert_thread, model::ThreadDetail};
use serde_json::json;

#[test]
fn windows_preserve_epoch_bounds_and_reject_overflow() {
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
    for (expression, seconds) in [
        ("2h", 7200),
        ("1d", 86400),
        ("4w", 2419200),
        ("1m", 2592000),
        ("1y", 31536000),
        ("0d", 0),
    ] {
        let window: Window = expression.parse().unwrap();
        assert_eq!(
            (now - window.start(now).unwrap().unwrap()).num_seconds(),
            seconds
        );
        assert_eq!(
            (window.end(now).unwrap().unwrap() - now).num_seconds(),
            seconds
        );
    }
    assert_eq!(
        "1969-12-31"
            .parse::<Window>()
            .unwrap()
            .start(now)
            .unwrap()
            .unwrap()
            .timestamp(),
        -86400
    );
    assert_eq!(
        "1970-01-01"
            .parse::<Window>()
            .unwrap()
            .start(now)
            .unwrap()
            .unwrap()
            .timestamp(),
        0
    );
    for expression in ["", " all ", "ALL"] {
        assert!(
            expression
                .parse::<Window>()
                .unwrap()
                .start(now)
                .unwrap()
                .is_none()
        );
    }
    for expression in ["-1d", "3x", "9223372036854775807y", "2026-02-30", "3", "é"] {
        assert!(expression.parse::<Window>().is_err(), "{expression}");
    }
    let bound = Window::Relative(chrono::TimeDelta::seconds(1));
    assert!(bound.end(chrono::DateTime::<Utc>::MAX_UTC).is_err());
    assert!(bound.start(chrono::DateTime::<Utc>::MIN_UTC).is_err());
}

#[test]
fn classification_and_rfc_mail_conversion_use_native_parsers() {
    let rules = Classifier::new(Rules::default()).unwrap();
    let now = Utc.with_ymd_and_hms(2026, 10, 14, 12, 0, 0).unwrap();
    let detail: ThreadDetail = serde_json::from_value(json!({"id":"t", "historyId":"100", "messages":[
        {"id":"a","threadId":"t","internalDate":"1000","snippet":"hello", "payload":{"headers":[
            {"name":"from","value":"=?UTF-8?B?RG9lLCBKb2hu?= <john@example.com>"},
            {"name":"auto-submitted","value":" no ; x-test=yes "},
            {"name":"subject","value":"A meeting"}, {"name":"To","value":"you@example.com"}]}},
        {"id":"b","snippet":"auto", "payload":{"headers":[{"name":"FROM","value":"Notification@host.example"}]}},
        {"id":"c","payload":{"headers":[{"name":"Date","value":"Thu, 1 Jan 1970 01:00:00 +0100"}, {"name":"From","value":"bad header"}]}},
        {"id":"d","payload":{"headers":[{"name":"List-Unsubscribe","value":""}]}}
    ]})).unwrap();
    let converted = convert_thread(&detail, &rules, now).unwrap();
    assert_eq!(converted[0].from_name, "Doe, John");
    assert_eq!(converted[0].from_address, "john@example.com");
    assert_eq!(converted[0].date_iso, "1970-01-01T00:00:01Z");
    assert!(!converted[0].is_automated);
    assert!(converted[1].is_automated);
    assert_eq!(converted[1].body_plain, "auto");
    assert_eq!(converted[2].from_address, "bad header");
    assert_eq!(converted[2].date_iso, "1970-01-01T00:00:00Z");
    assert!(converted[3].is_automated);
    let mut broken = detail.clone();
    broken.messages[1].internal_date = "broken".into();
    assert!(convert_thread(&broken, &rules, now).is_err());
    broken = detail.clone();
    broken.id.clear();
    assert!(convert_thread(&broken, &rules, now).is_err());
    for (id, thread_id, internal_date) in [
        ("", "t", "1"),
        ("m", "wrong", "1"),
        ("m", "t", "9223372036854775807"),
    ] {
        let mut invalid = detail.clone();
        invalid.messages[0].id = id.into();
        invalid.messages[0].thread_id = thread_id.into();
        invalid.messages[0].internal_date = internal_date.into();
        assert!(convert_thread(&invalid, &rules, now).is_err());
    }
    for (timestamp, expected) in [
        ("0", "1970-01-01T00:00:00Z"),
        ("-1000", "1969-12-31T23:59:59Z"),
    ] {
        let mut valid = detail.clone();
        valid.messages[0].internal_date = timestamp.into();
        assert_eq!(
            convert_thread(&valid, &rules, now).unwrap()[0].date_iso,
            expected
        );
    }
}

#[test]
fn additional_rules_are_loaded_without_silencing_configuration_errors() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("rules.json");
    let missing = Classifier::load(&path).unwrap();
    assert!(
        missing
            .classify("no-reply@example.com", "", &[])
            .is_automated
    );
    std::fs::write(
        &path,
        br#"{"sender_patterns":["(?i)robot@"],"snippet_patterns":["Your receipt"]}"#,
    )
    .unwrap();
    let classifier = Classifier::load(&path).unwrap();
    assert!(
        classifier
            .classify("robot@example.com", "", &[])
            .is_automated
    );
    assert!(
        classifier
            .classify("someone@example.com", "Your receipt", &[])
            .reason
            .starts_with("snippet:")
    );
    assert!(
        !classifier
            .classify("human@example.com", "Hello", &[])
            .is_automated
    );
    std::fs::write(&path, "{").unwrap();
    assert!(Classifier::load(&path).is_err());
    std::fs::write(&path, br#"{"sender_patterns":["("]}"#).unwrap();
    assert!(Classifier::load(&path).is_err());
    assert!(Classifier::load(temp.path()).is_err());
}
