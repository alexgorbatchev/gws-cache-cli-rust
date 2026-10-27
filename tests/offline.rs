use cli_command_kit::Mode;
use std::{ffi::OsString, path::Path};
use tokio_util::sync::CancellationToken;

async fn run(path: &Path, args: &[&str], mode: Mode) -> (u8, String, String) {
    let mut argv = vec![
        OsString::from("gws-cache"),
        OsString::from("--db"),
        path.as_os_str().to_owned(),
    ];
    argv.extend(args.iter().map(OsString::from));
    let mut out = Vec::new();
    let mut err = Vec::new();
    let status =
        gws_cache_cli::entry(argv, &mut out, &mut err, mode, &CancellationToken::new()).await;
    (
        status,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

#[tokio::test]
async fn offline_topics_cache_search_and_export_round_trip() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cache.db");
    let result = run(
        &path,
        &[
            "topic",
            "add",
            "work",
            "--name",
            "Work",
            "--query",
            "label:work",
        ],
        Mode::Agent,
    )
    .await;
    assert_eq!(result.0, 0, "{}", result.2);
    assert!(result.1.starts_with("OK:"));
    let result = run(&path, &["topic", "list"], Mode::Agent).await;
    assert!(result.1.contains("slug:work\tname:Work"));
    let result = run(&path, &["cache", "status"], Mode::Agent).await;
    assert!(result.1.contains("topics:1"));
    for subject in ["message", "event"] {
        let result = run(
            &path,
            &[subject, "search", "--format", "json", "--since", "all"],
            Mode::Agent,
        )
        .await;
        assert_eq!(result.0, 0, "{}", result.2);
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(&result.1).unwrap(),
            serde_json::json!([])
        );
    }
    let result = run(
        &path,
        &[
            "message", "export", "work", "--format", "json", "--since", "all",
        ],
        Mode::Human,
    )
    .await;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&result.1).unwrap(),
        serde_json::json!([])
    );
    let result = run(&path, &["topic", "remove", "work"], Mode::Human).await;
    assert_eq!(result.0, 0, "{}", result.2);
    assert!(
        run(&path, &["topic", "list"], Mode::Human)
            .await
            .1
            .contains("No topics")
    );
}

#[tokio::test]
async fn invalid_arguments_fail_without_creating_cache() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("must-not-exist.db");
    for args in [
        vec!["message", "sync"],
        vec!["message", "sync", "work", "--all"],
        vec!["message", "sync", "work", "--max-threads", "0"],
        vec!["message", "search", "--since", "3x"],
        vec!["event", "search", "--format", "xml"],
        vec!["cache", "status", "--timeout", "nonsense"],
        vec!["topic", "add", " "],
        vec!["skill", "extra"],
    ] {
        let result = run(&path, &args, Mode::Agent).await;
        assert_ne!(result.0, 0, "{args:?}");
        assert!(result.1.is_empty(), "{args:?}");
        assert!(!result.2.is_empty(), "{args:?}");
        assert!(!path.exists(), "{args:?}");
    }
}

#[tokio::test]
async fn terminal_output_neutralizes_controls_without_changing_stored_topic_data() {
    let temp = tempfile::tempdir().unwrap();
    let path = temp.path().join("cache.db");
    let name = "Work\x1b[2J\nName\tDone\x07";
    let added = run(
        &path,
        &["topic", "add", "work", "--name", name],
        Mode::Agent,
    )
    .await;
    assert_eq!(added.0, 0, "{}", added.2);
    for mode in [Mode::Agent, Mode::Human] {
        let result = run(&path, &["topic", "list"], mode).await;
        assert_eq!(result.0, 0, "{}", result.2);
        assert!(!result.1.contains(['\x1b', '\x07']), "{:?}", result.1);
        assert!(result.1.contains("Work[2J Name Done"), "{}", result.1);
    }
    let store = gws_cache::Store::open(&path).unwrap();
    assert_eq!(store.topic("work").unwrap().display_name, name);
}
